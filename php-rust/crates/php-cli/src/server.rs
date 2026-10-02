//! `ferro -S host:port [-t docroot] [router.php]` — a work-alike of PHP's
//! built-in development web server (the cli-server SAPI).
//!
//! Faithful to `php -S` 8.5.7 (oracle-pinned by the WP-4 sapi-probe battery):
//! sequential request handling on one thread, `Connection: close` on every
//! response, the request Host echoed back, PHP script responses without
//! Content-Length, static files with the cli-server mime map and
//! `Content-Length`, the exact 404 template, and the asctime-stamped stderr
//! log (`Accepted` / `[code]: METHOD URI` / `Closing`, PHP diagnostics
//! interleaved).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::ffi::OsStrExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use php_types::sapi::WebRequest;

use crate::mime::MIME_TYPE_MAP;

/// The 404 page of the cli-server, byte-identical (URI interpolated).
const ERROR_PAGE_HEAD: &str = "<!doctype html><html><head><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>404 Not Found</title><style>\nbody { background-color: #fcfcfc; color: #333333; margin: 0; padding:0; }\nh1 { font-size: 1.5em; font-weight: normal; background-color: #9999cc; min-height:2em; line-height:2em; border-bottom: 1px inset black; margin: 0; }\nh1, p { padding-left: 10px; }\ncode.url { background-color: #eeeeee; font-family:monospace; padding:0 2px;}\n</style>\n</head><body><h1>Not Found</h1><p>The requested resource <code class=\"url\">";
const ERROR_PAGE_TAIL: &str = "</code> was not found on this server.</p></body></html>";

/// PHP's reason-phrase table (main/http_status_codes.h).
fn status_reason(code: i64) -> &'static str {
    match code {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Request Entity Too Large",
        414 => "Request-URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Requested Range Not Satisfiable",
        417 => "Expectation Failed",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        511 => "Network Authentication Required",
        _ => "Unknown Status Code",
    }
}

/// The mime type for a file extension (lowercased lookup, sorted table).
fn mime_for_ext(ext: &[u8]) -> Option<&'static str> {
    let ext = String::from_utf8_lossy(&ext.to_ascii_lowercase()).into_owned();
    MIME_TYPE_MAP
        .binary_search_by(|(e, _)| (*e).cmp(ext.as_str()))
        .ok()
        .map(|i| MIME_TYPE_MAP[i].1)
}

/// Local time as asctime ("Tue Jul 14 17:24:55 2026" — day space-padded),
/// the cli-server log timestamp.
fn asctime_local() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        let t = now as libc::time_t;
        libc::localtime_r(&t, &mut tm);
    }
    const WDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{} {} {:2} {:02}:{:02}:{:02} {}",
        WDAYS[tm.tm_wday.clamp(0, 6) as usize],
        MONS[tm.tm_mon.clamp(0, 11) as usize],
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
        tm.tm_year + 1900
    )
}

fn log_line(msg: &str) {
    eprintln!("[{}] {}", asctime_local(), msg);
}

/// Percent-decode a URL *path* (no `+` → space — that is query-only).
fn percent_decode_path(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'%' && i + 2 < s.len() {
            let hi = (s[i + 1] as char).to_digit(16);
            let lo = (s[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(s[i]);
        i += 1;
    }
    out
}

/// Normalize a decoded path: resolve `.`/`..` segments, clamping at the root
/// (the oracle serves `GET /../x` as `/x`). Preserves a trailing slash.
fn normalize_path(path: &[u8]) -> Vec<u8> {
    let trailing = path.ends_with(b"/");
    let mut segs: Vec<&[u8]> = Vec::new();
    for seg in path.split(|&b| b == b'/') {
        match seg {
            b"" | b"." => {}
            b".." => {
                segs.pop();
            }
            s => segs.push(s),
        }
    }
    let mut out = Vec::new();
    for s in &segs {
        out.push(b'/');
        out.extend_from_slice(s);
    }
    if out.is_empty() {
        out.push(b'/');
    } else if trailing {
        out.push(b'/');
    }
    out
}

/// One parsed HTTP request off the socket.
struct HttpRequest {
    method: Vec<u8>,
    target: Vec<u8>,
    protocol: (u8, u8),
    headers: Vec<(Vec<u8>, Vec<u8>)>,
    body: Vec<u8>,
}

fn header_value<'a>(headers: &'a [(Vec<u8>, Vec<u8>)], name: &[u8]) -> Option<&'a [u8]> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_slice())
}

/// Read and parse one request (headers + Content-Length body).
fn read_request(stream: &mut TcpStream) -> Option<HttpRequest> {
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut tmp = [0u8; 8192];
    let head_end;
    loop {
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            head_end = p;
            break;
        }
        if buf.len() > 1024 * 1024 {
            return None;
        }
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let head = buf[..head_end].to_vec();
    let mut rest = buf[head_end + 4..].to_vec();
    let mut lines = head.split(|&b| b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l));
    let request_line = lines.next()?;
    let mut parts = request_line.split(|&b| b == b' ');
    let method = parts.next()?.to_vec();
    let target = parts.next()?.to_vec();
    let proto = parts.next().unwrap_or(b"HTTP/1.1");
    let protocol = if proto.starts_with(b"HTTP/") && proto.len() >= 8 {
        (
            proto[5].wrapping_sub(b'0').min(9),
            proto[7].wrapping_sub(b'0').min(9),
        )
    } else {
        (1, 1)
    };
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some(colon) = line.iter().position(|&b| b == b':') else { continue };
        let name = line[..colon].to_vec();
        let mut value = &line[colon + 1..];
        while value.first() == Some(&b' ') || value.first() == Some(&b'\t') {
            value = &value[1..];
        }
        headers.push((name, value.to_vec()));
    }
    // Body: `Transfer-Encoding: chunked` is decoded like PHP's parser
    // (php://input carries the DE-chunked bytes; CONTENT_LENGTH stays unset
    // because the request has no Content-Length header); otherwise
    // Content-Length bytes.
    let te_chunked = header_value(&headers, b"transfer-encoding")
        .map(|v| v.to_ascii_lowercase())
        .is_some_and(|v| v.windows(7).any(|w| w == b"chunked"));
    if te_chunked {
        rest = read_chunked_body(stream, rest)?;
    } else {
        let clen: usize = header_value(&headers, b"content-length")
            .and_then(|v| String::from_utf8_lossy(v).trim().parse().ok())
            .unwrap_or(0);
        let clen = clen.min(512 * 1024 * 1024);
        while rest.len() < clen {
            let n = stream.read(&mut tmp).ok()?;
            if n == 0 {
                break;
            }
            rest.extend_from_slice(&tmp[..n]);
        }
        rest.truncate(clen);
    }
    Some(HttpRequest {
        method,
        target,
        protocol,
        headers,
        body: rest,
    })
}

/// Decode a `Transfer-Encoding: chunked` request body: `already` holds the
/// bytes read past the header block; more are pulled from the stream as
/// needed. Returns the de-chunked payload (trailers are consumed and dropped).
fn read_chunked_body(stream: &mut TcpStream, already: Vec<u8>) -> Option<Vec<u8>> {
    let mut raw = already;
    let mut pos = 0usize;
    let mut out = Vec::new();
    let mut tmp = [0u8; 8192];
    // Ensure at least `need` bytes exist past `pos`, reading as necessary.
    macro_rules! ensure {
        ($need:expr) => {
            while raw.len() - pos < $need {
                let n = stream.read(&mut tmp).ok()?;
                if n == 0 {
                    return None;
                }
                raw.extend_from_slice(&tmp[..n]);
            }
        };
    }
    loop {
        // Chunk-size line: hex[;extensions]\r\n.
        let line_end = loop {
            if let Some(p) = raw[pos..].windows(2).position(|w| w == b"\r\n") {
                break pos + p;
            }
            ensure!(raw.len() - pos + 1);
        };
        let line = &raw[pos..line_end];
        let hex_part = line.split(|&b| b == b';').next().unwrap_or(line);
        let size = usize::from_str_radix(String::from_utf8_lossy(hex_part).trim(), 16).ok()?;
        pos = line_end + 2;
        if size == 0 {
            // Trailer section ends at the first empty line; a bare CRLF right
            // here is the common no-trailer case.
            loop {
                ensure!(2);
                if let Some(p) = raw[pos..].windows(2).position(|w| w == b"\r\n") {
                    let blank = p == 0;
                    pos += p + 2;
                    if blank {
                        return Some(out);
                    }
                } else {
                    ensure!(raw.len() - pos + 1);
                }
            }
        }
        if size > 512 * 1024 * 1024 {
            return None;
        }
        ensure!(size + 2);
        out.extend_from_slice(&raw[pos..pos + size]);
        pos += size;
        if &raw[pos..pos + 2] == b"\r\n" {
            pos += 2;
        }
    }
}

/// What the docroot walk resolved the request path to.
enum Resolved {
    /// A PHP script: absolute file, its vpath, and any PATH_INFO.
    Script(PathBuf, Vec<u8>, Option<Vec<u8>>),
    /// A static file on disk.
    Static(PathBuf),
    NotFound,
}

/// Translate a decoded, normalized path against the docroot; an unresolved
/// path falls back to the DOCROOT index.php with the whole decoded path as
/// PATH_INFO (oracle-pinned: SCRIPT_NAME=/index.php, PHP_SELF=
/// /index.php/robots.txt) — this is what serves WordPress' virtual routes
/// (/robots.txt, /wp-json/) without a router script.
fn translate(docroot: &Path, path: &[u8]) -> Resolved {
    match translate_walk(docroot, path) {
        Resolved::NotFound => {
            let root_index = docroot.join("index.php");
            if root_index.is_file() {
                return Resolved::Script(root_index, b"/index.php".to_vec(), Some(path.to_vec()));
            }
            Resolved::NotFound
        }
        hit => hit,
    }
}

/// The docroot walk: longest existing file prefix wins (the remainder is
/// PATH_INFO for scripts), a directory tries `index.php` then `index.html`
/// (cli-server order).
fn translate_walk(docroot: &Path, path: &[u8]) -> Resolved {
    let rel = &path[1.min(path.len())..];
    let mut acc = docroot.to_path_buf();
    let mut vpath: Vec<u8> = Vec::new();
    let segs: Vec<&[u8]> = if rel.is_empty() {
        Vec::new()
    } else {
        rel.split(|&b| b == b'/').collect()
    };
    for (i, seg) in segs.iter().enumerate() {
        if seg.is_empty() {
            continue;
        }
        acc.push(std::ffi::OsStr::from_bytes(seg));
        vpath.push(b'/');
        vpath.extend_from_slice(seg);
        let Ok(meta) = std::fs::metadata(&acc) else { return Resolved::NotFound };
        if meta.is_file() {
            let rest: Vec<u8> = segs[i + 1..]
                .iter()
                .flat_map(|s| {
                    let mut v = vec![b'/'];
                    v.extend_from_slice(s);
                    v
                })
                .collect();
            let path_info = (!rest.is_empty()).then_some(rest);
            if vpath.to_ascii_lowercase().ends_with(b".php") {
                return Resolved::Script(acc, vpath, path_info);
            }
            return if path_info.is_none() {
                Resolved::Static(acc)
            } else {
                Resolved::NotFound
            };
        }
    }
    // Landed on a directory: try the index files.
    for idx in [&b"index.php"[..], &b"index.html"[..]] {
        let cand = acc.join(std::ffi::OsStr::from_bytes(idx));
        if cand.is_file() {
            vpath.push(b'/');
            vpath.extend_from_slice(idx);
            return if idx.ends_with(b".php") {
                Resolved::Script(cand, vpath, None)
            } else {
                Resolved::Static(cand)
            };
        }
    }
    Resolved::NotFound
}

/// Whether the script sent header `name` (lowercase): PHP's cli-server then
/// sends the script's `Date` instead of its own (Symfony's Response sets one).
fn script_sets_header(headers: &[Vec<u8>], name: &[u8]) -> bool {
    headers.iter().any(|l| {
        l.iter().position(|&b| b == b':').is_some_and(|p| l[..p].trim_ascii().eq_ignore_ascii_case(name))
    })
}

/// Remove the head line starting with `prefix` (e.g. the server's `Date: `).
fn drop_head_line(out: &mut Vec<u8>, prefix: &[u8]) {
    if let Some(start) = out.windows(prefix.len()).position(|w| w == prefix) {
        if let Some(len) = out[start..].windows(2).position(|w| w == b"\r\n") {
            out.drain(start..start + len + 2);
        }
    }
}

/// The response head shared by every kind of response.
fn response_head(
    protocol: (u8, u8),
    code: i64,
    reason: &str,
    host: Option<&[u8]>,
) -> Vec<u8> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(
        format!("HTTP/{}.{} {} {}\r\n", protocol.0, protocol.1, code, reason).as_bytes(),
    );
    if let Some(h) = host {
        out.extend_from_slice(b"Host: ");
        out.extend_from_slice(h);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(
        format!("Date: {}\r\n", php_types::sapi::http_date(now)).as_bytes(),
    );
    out.extend_from_slice(b"Connection: close\r\n");
    out
}

struct ServerConfig {
    host: String,
    port: u16,
    docroot: PathBuf,
    router: Option<PathBuf>,
}

/// One request/response cycle on an accepted connection.
fn handle_client(
    cfg: &ServerConfig,
    registry: &php_runtime::Registry,
    stream: &mut TcpStream,
    peer: (String, u16),
) {
    let Some(req) = read_request(stream) else { return };
    let head = req.method == b"HEAD";
    let host = header_value(&req.headers, b"host").map(|v| v.to_vec());
    // A bare trailing `?` registers no QUERY_STRING (oracle-pinned).
    let (path_part, query) = match req.target.iter().position(|&b| b == b'?') {
        Some(p) if p + 1 < req.target.len() => {
            (&req.target[..p], Some(req.target[p + 1..].to_vec()))
        }
        Some(p) => (&req.target[..p], None),
        None => (&req.target[..], None),
    };
    let decoded = normalize_path(&percent_decode_path(path_part));
    let uri_label = format!(
        "{} {}",
        String::from_utf8_lossy(&req.method),
        String::from_utf8_lossy(&req.target)
    );

    // S-80.0.3 (A-BG20/A-AH20/KL-81-3, Council WP-81): idle-window probe on
    // the cli arm too — the arm that makes A-BB1 judgeable had NO idle probe,
    // so its idle drift was undeclared. Accept-side, produces NO census-cli
    // line (out of the counted channel, A-BG19), same line shape as the axum
    // dispatcher probe. Anchored ends_with (S-78.1 lesson: magic paths are
    // anchored at the END, never by equality — the docroot prefix varies).
    #[cfg(feature = "census-instrumentation")]
    if decoded.ends_with(b"/__census_global") {
        let (calls, bytes) = php_runtime::alloc_census::SNAPSHOT_FN
            .get()
            .map(|f| f())
            .unwrap_or((0, 0));
        // A-DL14 (Council WP-82): gross-churn tag in-band, appended LAST
        // (the idle parser reads fields $3/$5 positionally).
        eprintln!("census-global: calls={calls} bytes={bytes} gross=1");
        let body: &[u8] = b"census-global\n";
        let mut out = response_head(req.protocol, 200, "OK", host.as_deref());
        out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
        if !head {
            out.extend_from_slice(body);
        }
        let _ = stream.write_all(&out);
        log_request(&peer, 200, &uri_label, None);
        return;
    }

    // Router script first: any return value other than boolean false ends the
    // request; false falls through to the normal docroot resolution. Anything
    // the router ECHOED before returning false is NOT discarded (oracle-pinned
    // WP-10): for a PHP target it lands at the front of the script's body (the
    // oracle runs both in one request sharing the output stream); for a static
    // file / 404 page it is flushed RAW on the socket ahead of the response
    // head (yes: the oracle emits a malformed response there).
    let mut router_prefix: Vec<u8> = Vec::new();
    if let Some(router) = &cfg.router {
        let outcome = run_php(
            cfg, registry, &req, &peer, router, decoded.clone(), None, query.clone(),
        );
        match outcome {
            Some((outcome, routed)) if routed => {
                let detail = fatal_detail(&outcome);
                let code =
                    write_php_response(stream, &req, host.as_deref(), &outcome, head, b"");
                log_request(&peer, code, &uri_label, detail.as_deref());
                return;
            }
            None => {
                let code = write_bare_error(stream, &req, host.as_deref(), 500, head);
                log_request(&peer, code, &uri_label, None);
                return;
            }
            Some((outcome, _)) => {
                // returned false — fall through, keeping its output.
                router_prefix = outcome.rendered.clone();
            }
        }
    }

    match translate(&cfg.docroot, &decoded) {
        Resolved::Script(file, vpath, path_info) => {
            match run_php(cfg, registry, &req, &peer, &file, vpath, path_info, query) {
                Some((outcome, _)) => {
                    let detail = fatal_detail(&outcome);
                    let code = write_php_response(
                        stream,
                        &req,
                        host.as_deref(),
                        &outcome,
                        head,
                        &router_prefix,
                    );
                    log_request(&peer, code, &uri_label, detail.as_deref());
                }
                None => {
                    let code = write_bare_error(stream, &req, host.as_deref(), 500, head);
                    log_request(&peer, code, &uri_label, None);
                }
            }
        }
        Resolved::Static(file) => {
            let body = std::fs::read(&file).unwrap_or_default();
            if !router_prefix.is_empty() {
                let _ = stream.write_all(&router_prefix);
            }
            let mut out = response_head(req.protocol, 200, "OK", host.as_deref());
            let ext = file
                .extension()
                .map(|e| e.as_bytes().to_vec())
                .unwrap_or_default();
            if let Some(mime) = mime_for_ext(&ext) {
                out.extend_from_slice(b"Content-Type: ");
                out.extend_from_slice(mime.as_bytes());
                if mime.starts_with("text/") {
                    out.extend_from_slice(b"; charset=UTF-8");
                }
                out.extend_from_slice(b"\r\n");
            }
            out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
            if !head {
                out.extend_from_slice(&body);
            }
            let _ = stream.write_all(&out);
            log_request(&peer, 200, &uri_label, None);
        }
        Resolved::NotFound => {
            let mut page = ERROR_PAGE_HEAD.as_bytes().to_vec();
            page.extend_from_slice(&decoded);
            page.extend_from_slice(ERROR_PAGE_TAIL.as_bytes());
            if !router_prefix.is_empty() {
                let _ = stream.write_all(&router_prefix);
            }
            let mut out = response_head(req.protocol, 404, "Not Found", host.as_deref());
            out.extend_from_slice(b"X-Powered-By: PHP/8.5.7\r\n");
            out.extend_from_slice(b"Content-Type: text/html; charset=UTF-8\r\n");
            out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", page.len()).as_bytes());
            if !head {
                out.extend_from_slice(&page);
            }
            let _ = stream.write_all(&out);
            log_request(&peer, 404, &uri_label, Some("No such file or directory"));
        }
    }
}

/// The `- detail` of the request log line when the script died with a fatal:
/// the whole (multiline) fatal message, prefix stripped — oracle-pinned.
fn fatal_detail(outcome: &php_runtime::Outcome) -> Option<String> {
    if outcome.fatal.is_none() {
        return None;
    }
    let last = outcome.error_log.last()?;
    let s = String::from_utf8_lossy(last);
    Some(s.strip_prefix("PHP Fatal error:  ").unwrap_or(&s).to_string())
}

fn log_request(peer: &(String, u16), code: i64, uri: &str, detail: Option<&str>) {
    match detail {
        Some(d) => log_line(&format!("{}:{} [{}]: {} - {}", peer.0, peer.1, code, uri, d)),
        None => log_line(&format!("{}:{} [{}]: {}", peer.0, peer.1, code, uri)),
    }
}

/// Run one PHP script for this request. Returns the outcome plus whether the
/// run "handled" the request (always true for a non-router script; a router
/// returning boolean false does not). `None` = the engine failed hard
/// (lowering error or panic) — the caller sends a bare 500.
#[allow(clippy::too_many_arguments)]
fn run_php(
    cfg: &ServerConfig,
    registry: &php_runtime::Registry,
    req: &HttpRequest,
    peer: &(String, u16),
    file: &Path,
    vpath: Vec<u8>,
    path_info: Option<Vec<u8>>,
    query: Option<Vec<u8>>,
) -> Option<(php_runtime::Outcome, bool)> {
    let source = std::fs::read(file).ok()?;
    let request_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let web = WebRequest {
        method: req.method.clone(),
        protocol: req.protocol,
        request_uri: req.target.clone(),
        vpath,
        path_info,
        query_string: query,
        headers: req.headers.clone(),
        body: req.body.clone(),
        remote_addr: peer.0.clone(),
        remote_port: peer.1,
        server_host: cfg.host.clone(),
        server_port: cfg.port,
        doc_root: cfg.docroot.as_os_str().as_bytes().to_vec(),
        script_filename: file.as_os_str().as_bytes().to_vec(),
        request_time,
    };
    php_types::sapi::set_web_request(Rc::new(web));
    let name = file.as_os_str().as_bytes().to_vec();
    // S-79.0.6 (A-BG16, Council WP-80): per-request census line on the CLI
    // arm — the engine window only (lower+compile+run+teardown), comparable
    // to the axum arm's a+b+c; fs read and HTTP plumbing sit outside, like
    // the axum resid channel. Line name "census-cli:" is probe API (WP-64).
    #[cfg(feature = "census-instrumentation")]
    let census_s0 = php_runtime::alloc_census::SNAPSHOT_FN
        .get()
        .map(|f| f())
        .unwrap_or((0, 0));
    // A-BB6/A-TH14: this SAPI is long-lived — the main probe is ON (the one
    // parameter at the SAPI boundary; the one-shot CLI keeps probe off via
    // run_source_with_ini). This arm stays the honest A/B twin of the axum
    // worker: same acquire, same park, same put-after-link_fatal_check.
    let result = catch_unwind(AssertUnwindSafe(|| {
        php_runtime::run_source_probed(&name, &source, registry, &[], None, true)
    }));
    #[cfg(feature = "census-instrumentation")]
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static CLI_REQS: AtomicU64 = AtomicU64::new(0);
        let census_s1 = php_runtime::alloc_census::SNAPSHOT_FN
            .get()
            .map(|f| f())
            .unwrap_or((0, 0));
        let ((a1_calls, a1_bytes), (a3_calls, a3_bytes)) =
            php_runtime::alloc_census::take_split();
        let req = CLI_REQS.fetch_add(1, Ordering::Relaxed) + 1;
        // gross=1 (A-DL10): gross churn, upper bound. a3_trip (KS-MS-81-2):
        // split-integrity tripwire — the driver VOIDs the run on any nonzero.
        eprintln!(
            "census-cli: req={req} gross=1 total_calls={} total_bytes={} \
             a1_calls={a1_calls} a1_bytes={a1_bytes} a3_calls={a3_calls} a3_bytes={a3_bytes} \
             a3_trip={}",
            census_s1.0 - census_s0.0,
            census_s1.1 - census_s0.1,
            php_runtime::alloc_census::trip_count(),
        );
    }
    php_types::sapi::clear_web_request();
    // Unclaimed upload tmp files die with the request (PHP request shutdown).
    for tmp in php_types::sapi::take_uploaded_files() {
        let _ = std::fs::remove_file(std::ffi::OsStr::from_bytes(&tmp));
    }
    let outcome = match result {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            log_line(&format!("PHP Parse error: {e}"));
            return None;
        }
        Err(_) => {
            log_line("[phpr] internal error: the runtime panicked serving this request");
            return None;
        }
    };
    // Stderr log: the script's diagnostics ("PHP Warning:  …"), each stamped
    // like the oracle (continuation lines of a fatal stay unstamped).
    for entry in &outcome.error_log {
        log_line(&String::from_utf8_lossy(entry));
    }
    let routed = !matches!(outcome.return_value, php_types::Zval::Bool(false));
    Some((outcome, routed))
}

/// Write a PHP outcome as the HTTP response; returns the status code sent.
fn write_php_response(
    stream: &mut TcpStream,
    req: &HttpRequest,
    host: Option<&[u8]>,
    outcome: &php_runtime::Outcome,
    head: bool,
    body_prefix: &[u8],
) -> i64 {
    let code = outcome.response_code.unwrap_or(200);
    let reason_owned;
    let reason = match &outcome.response_reason {
        Some(r) => {
            reason_owned = String::from_utf8_lossy(r).into_owned();
            reason_owned.as_str()
        }
        None => status_reason(code),
    };
    let mut out = response_head(req.protocol, code, reason, host);
    if script_sets_header(&outcome.headers, b"date") {
        drop_head_line(&mut out, b"Date: ");
    }
    let mut have_ctype = false;
    for line in &outcome.headers {
        if let Some(p) = line.iter().position(|&b| b == b':') {
            if line[..p].eq_ignore_ascii_case(b"content-type") {
                have_ctype = true;
            }
        }
        out.extend_from_slice(line);
        out.extend_from_slice(b"\r\n");
    }
    if !have_ctype {
        out.extend_from_slice(b"Content-type: text/html; charset=UTF-8\r\n");
    }
    out.extend_from_slice(b"\r\n");
    if !head {
        out.extend_from_slice(body_prefix);
        out.extend_from_slice(&outcome.rendered);
    }
    let _ = stream.write_all(&out);
    code
}

/// A headers-only error response for engine-level failures (bare 500).
fn write_bare_error(
    stream: &mut TcpStream,
    req: &HttpRequest,
    host: Option<&[u8]>,
    code: i64,
    head: bool,
) -> i64 {
    let mut out = response_head(req.protocol, code, status_reason(code), host);
    out.extend_from_slice(b"Content-Type: text/html; charset=UTF-8\r\nContent-Length: 0\r\n\r\n");
    let _ = stream.write_all(&out);
    let _ = head;
    code
}

/// Entry point for `ferro -S`. Parses the residual arguments (already past
/// `-S host:port`), binds, and serves forever. Only returns on a bind error.
pub fn serve(addr: &str, mut rest: std::iter::Peekable<impl Iterator<Item = std::ffi::OsString>>) -> u8 {
    // The SAPI name must be installed before ANYTHING is lowered (PHP_SAPI is
    // folded at compile time, prelude included).
    php_types::sapi::set_sapi_name("cli-server");

    let (host, port_s) = match addr.rsplit_once(':') {
        Some(hp) => hp,
        None => {
            eprintln!("Invalid address: {addr}");
            return 1;
        }
    };
    let Ok(port) = port_s.parse::<u16>() else {
        eprintln!("Invalid address: {addr}");
        return 1;
    };
    let mut docroot: Option<PathBuf> = None;
    let mut router: Option<PathBuf> = None;
    let mut worker: Option<PathBuf> = None;
    let mut workers: usize = 0;
    let mut max_requests: u64 = 0;
    let mut reuse_port = false;
    while let Some(arg) = rest.next() {
        let bytes = arg.as_os_str().as_bytes();
        if bytes == b"-t" {
            docroot = rest.next().map(PathBuf::from);
        } else if bytes == b"--worker" {
            worker = rest.next().map(PathBuf::from);
        } else if bytes == b"--workers" {
            workers = rest
                .next()
                .and_then(|n| n.to_string_lossy().parse().ok())
                .unwrap_or(0);
        } else if bytes == b"--reuse-port" {
            // SO_REUSEPORT: several ferro processes serve one port, the
            // kernel spreading connections (php-fpm-style process pools).
            reuse_port = true;
        } else if bytes == b"--max-requests" {
            max_requests = rest
                .next()
                .and_then(|n| n.to_string_lossy().parse().ok())
                .unwrap_or(0);
        } else if router.is_none() {
            router = Some(PathBuf::from(arg));
        }
    }
    let docroot = docroot.unwrap_or_else(|| PathBuf::from("."));
    let Ok(docroot) = std::fs::canonicalize(&docroot) else {
        eprintln!("Directory {} does not exist.", docroot.display());
        return 1;
    };
    let router = router.map(|r| std::fs::canonicalize(&r).unwrap_or(r));
    // The cli-server chdirs to the docroot (oracle-pinned: getcwd() there,
    // relative fopen resolves against it).
    let _ = std::env::set_current_dir(&docroot);

    let registry = php_builtins::registry();
    let cfg = ServerConfig {
        host: host.to_string(),
        port,
        docroot,
        router,
    };
    let listener = match bind_listener(host, port, reuse_port) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to listen on {host}:{port} (reason: {e})");
            return 1;
        }
    };
    if let Some(script) = worker {
        let script = std::fs::canonicalize(&script).unwrap_or(script);
        let n = if workers == 0 {
            std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
        } else {
            workers
        };
        return serve_workers(listener, cfg, script, n, max_requests);
    }
    if workers > 0 {
        return serve_classic_pool(listener, cfg, workers);
    }
    log_line(&format!(
        "PHP 8.5.7 Development Server (http://{host}:{port}) started"
    ));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        serve_connection(&cfg, &registry, stream);
    }
    0
}

/// One accepted connection in one-shot mode: one request, a fresh `Vm`.
fn serve_connection(cfg: &ServerConfig, registry: &php_runtime::Registry, mut stream: TcpStream) {
    let peer = stream
        .peer_addr()
        .map(|a| (a.ip().to_string(), a.port()))
        .unwrap_or_else(|_| ("127.0.0.1".to_string(), 0));
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(60)));
    log_line(&format!("{}:{} Accepted", peer.0, peer.1));
    handle_client(cfg, registry, &mut stream, peer.clone());
    let _ = stream.flush();
    log_line(&format!("{}:{} Closing", peer.0, peer.1));
}

// ---------------------------------------------------------------------------
// Classic pool (`ferro -S host:port [router.php] --workers N`, no `--worker`):
// php-fpm's shape — N threads accept on the shared socket and serve each
// request exactly like the one-shot server (fresh `Vm`, nothing kept between
// requests but the compile caches: units, deferred declarations, regexes,
// realpaths, which are per thread like opcache's are per process).
// ---------------------------------------------------------------------------

fn serve_classic_pool(listener: TcpListener, cfg: ServerConfig, n: usize) -> u8 {
    log_line(&format!(
        "PHP 8.5.7 Development Server (http://{}:{}) started: {n} threads, one request per Vm",
        cfg.host, cfg.port
    ));
    let cfg = std::sync::Arc::new(cfg);
    let mut threads = Vec::with_capacity(n);
    for i in 0..n {
        let Ok(listener) = listener.try_clone() else {
            eprintln!("Failed to share the listening socket");
            return 1;
        };
        let cfg = cfg.clone();
        // The one-shot server runs on the main thread: same stack here.
        let t = std::thread::Builder::new()
            .name(format!("ferro-classic-{i}"))
            .stack_size(8 << 20)
            .spawn(move || {
                let registry = php_builtins::registry();
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { continue };
                    serve_connection(&cfg, &registry, stream);
                }
            });
        match t {
            Ok(t) => threads.push(t),
            Err(e) => {
                eprintln!("Failed to start a server thread: {e}");
                return 1;
            }
        }
    }
    for t in threads {
        let _ = t.join();
    }
    0
}

// ---------------------------------------------------------------------------
// Worker mode (fork, DECISION_KERNEL.md §5): `ferro -S host:port --worker
// worker.php [--workers N]`. N OS threads, one `Vm` each, each running
// `worker.php` once; the script boots the application and then loops on
// `ferro_handle_request(callable)`, which takes the next request off a shared
// queue, runs the callable and sends the response back. Connections are
// handled by one lightweight thread each (accept → parse → queue → write,
// keep-alive served request after request), so connections outnumber
// workers freely and no connection can starve another — the shape of
// php-fpm behind nginx or of FrankenPHP's Go front end, without the extra
// process. Every request goes to the worker script — no static files, no
// router (PHP only). A worker whose script returns (or dies) is restarted.
// `--max-requests N` (0 = never) recycles a worker after N requests: its
// N+1-th `ferro_handle_request()` returns false, the script returns, and the
// thread starts over on a fresh `Vm` (FrankenPHP's and php-fpm's
// `max_requests`, a bound on whatever a request leaks into the worker).
// ---------------------------------------------------------------------------

/// One request queued for a worker, with the channel its response goes to.
struct WorkerJob {
    web: WebRequest,
    reply: std::sync::mpsc::SyncSender<php_types::sapi::WorkerResponse>,
}

/// What the connection thread needs to write the response.
#[derive(Clone)]
struct PendingRequest {
    protocol: (u8, u8),
    head: bool,
    keep_alive: bool,
    host: Option<Vec<u8>>,
}

type JobQueue = std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<WorkerJob>>>;

fn serve_workers(
    listener: TcpListener,
    cfg: ServerConfig,
    script: PathBuf,
    n: usize,
    max_requests: u64,
) -> u8 {
    let source = match std::fs::read(&script) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Could not open worker script {}: {e}", script.display());
            return 1;
        }
    };
    let cfg = std::sync::Arc::new(cfg);
    let script = std::sync::Arc::new(script);
    let source = std::sync::Arc::new(source);
    log_line(&format!(
        "ferro worker server (http://{}:{}) started: {n} workers running {}{}",
        cfg.host,
        cfg.port,
        script.display(),
        if max_requests > 0 { format!(", recycled every {max_requests} requests") } else { String::new() }
    ));
    let (tx, rx) = std::sync::mpsc::sync_channel::<WorkerJob>(1024);
    let rx: JobQueue = std::sync::Arc::new(std::sync::Mutex::new(rx));
    for i in 0..n {
        let (rx, script, source) = (rx.clone(), script.clone(), source.clone());
        std::thread::spawn(move || worker_thread(i, rx, script, source, max_requests));
    }
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let (tx, cfg, script) = (tx.clone(), cfg.clone(), script.clone());
        std::thread::spawn(move || connection_thread(stream, tx, cfg, script));
    }
    0
}

/// One client connection: parse each request, queue it, write the response;
/// keep-alive until the peer closes or asks to.
fn connection_thread(
    mut stream: TcpStream,
    tx: std::sync::mpsc::SyncSender<WorkerJob>,
    cfg: std::sync::Arc<ServerConfig>,
    script: std::sync::Arc<PathBuf>,
) {
    let peer = stream
        .peer_addr()
        .map(|a| (a.ip().to_string(), a.port()))
        .unwrap_or_else(|_| ("127.0.0.1".to_string(), 0));
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(60)));
    let _ = stream.set_nodelay(true);
    while let Some(req) = read_request(&mut stream) {
        let head = req.method == b"HEAD";
        let host = header_value(&req.headers, b"host").map(|v| v.to_vec());
        let conn_hdr = header_value(&req.headers, b"connection").map(|v| v.to_ascii_lowercase());
        let keep_alive = match conn_hdr.as_deref() {
            Some(b"close") => false,
            Some(b"keep-alive") => true,
            _ => req.protocol >= (1, 1),
        };
        let meta = PendingRequest { protocol: req.protocol, head, keep_alive, host };
        let query = match req.target.iter().position(|&b| b == b'?') {
            Some(p) if p + 1 < req.target.len() => Some(req.target[p + 1..].to_vec()),
            _ => None,
        };
        let request_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        // The worker script stands in for /index.php (FrankenPHP's
        // convention): SCRIPT_NAME is the front controller, the request
        // path travels in REQUEST_URI.
        let web = WebRequest {
            method: req.method,
            protocol: req.protocol,
            request_uri: req.target,
            vpath: b"/index.php".to_vec(),
            path_info: None,
            query_string: query,
            headers: req.headers,
            body: req.body,
            remote_addr: peer.0.clone(),
            remote_port: peer.1,
            server_host: cfg.host.clone(),
            server_port: cfg.port,
            doc_root: cfg.docroot.as_os_str().as_bytes().to_vec(),
            script_filename: script.as_os_str().as_bytes().to_vec(),
            request_time,
        };
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        if tx.send(WorkerJob { web, reply: reply_tx }).is_err() {
            return;
        }
        let Ok(resp) = reply_rx.recv() else { return };
        let out = worker_response_bytes(&meta, &resp);
        if stream.write_all(&out).is_err() || stream.flush().is_err() || !keep_alive {
            return;
        }
    }
}

/// Serialise a worker response: status line, `Date`, keep-alive, the
/// script's headers, `Content-Length`, body (omitted for HEAD).
fn worker_response_bytes(p: &PendingRequest, resp: &php_types::sapi::WorkerResponse) -> Vec<u8> {
    let reason_owned;
    let reason = match &resp.reason {
        Some(r) => {
            reason_owned = String::from_utf8_lossy(r).into_owned();
            reason_owned.as_str()
        }
        None => status_reason(resp.status),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut out = Vec::with_capacity(256 + resp.body.len());
    out.extend_from_slice(
        format!("HTTP/{}.{} {} {}\r\n", p.protocol.0, p.protocol.1, resp.status, reason).as_bytes(),
    );
    if let Some(h) = &p.host {
        out.extend_from_slice(b"Host: ");
        out.extend_from_slice(h);
        out.extend_from_slice(b"\r\n");
    }
    if !script_sets_header(&resp.headers, b"date") {
        out.extend_from_slice(format!("Date: {}\r\n", php_types::sapi::http_date(now)).as_bytes());
    }
    out.extend_from_slice(if p.keep_alive {
        b"Connection: keep-alive\r\n"
    } else {
        b"Connection: close\r\n"
    });
    let mut have_ctype = false;
    for line in &resp.headers {
        if let Some(c) = line.iter().position(|&b| b == b':') {
            if line[..c].eq_ignore_ascii_case(b"content-type") {
                have_ctype = true;
            }
        }
        out.extend_from_slice(line);
        out.extend_from_slice(b"\r\n");
    }
    if !have_ctype {
        out.extend_from_slice(b"Content-type: text/html; charset=UTF-8\r\n");
    }
    // An application-sent Content-Length (Symfony's Response::prepare)
    // stands, like on the one-shot path; ours only fills its absence.
    if !script_sets_header(&resp.headers, b"content-length") {
        out.extend_from_slice(format!("Content-Length: {}\r\n", resp.body.len()).as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    if !p.head {
        out.extend_from_slice(&resp.body);
    }
    out
}

fn worker_thread(
    index: usize,
    rx: JobQueue,
    script: std::sync::Arc<PathBuf>,
    source: std::sync::Arc<Vec<u8>>,
    max_requests: u64,
) {
    php_types::sapi::set_sapi_name("cli-server");
    let registry = php_builtins::registry();
    let name = script.as_os_str().as_bytes().to_vec();
    loop {
        // The reply channel of the request in hand, shared by the two hooks.
        let reply: Rc<std::cell::RefCell<Option<std::sync::mpsc::SyncSender<php_types::sapi::WorkerResponse>>>> =
            Rc::new(std::cell::RefCell::new(None));
        let next_request = {
            let (rx, reply) = (rx.clone(), reply.clone());
            let mut served: u64 = 0;
            Box::new(move || -> Option<WebRequest> {
                if max_requests > 0 && served >= max_requests {
                    return None;
                }
                served += 1;
                let job = rx.lock().ok()?.recv().ok()?;
                *reply.borrow_mut() = Some(job.reply);
                Some(job.web)
            }) as Box<dyn FnMut() -> Option<WebRequest>>
        };
        let send_response = {
            let reply = reply.clone();
            Box::new(move |resp: php_types::sapi::WorkerResponse| {
                for entry in &resp.error_log {
                    log_line(&String::from_utf8_lossy(entry));
                }
                if let Some(r) = reply.borrow_mut().take() {
                    let _ = r.send(resp);
                }
            }) as Box<dyn FnMut(php_types::sapi::WorkerResponse)>
        };
        php_types::sapi::set_worker_hooks(php_types::sapi::WorkerHooks { next_request, send_response });
        let result = catch_unwind(AssertUnwindSafe(|| {
            php_runtime::run_source_with_ini(&name, &source, &registry, &[])
        }));
        match result {
            Ok(Ok(outcome)) => {
                if let Some(f) = &outcome.fatal {
                    log_line(&format!("[worker {index}] script ended with a fatal: {}", f.message()));
                    let _ = std::io::stderr().write_all(&outcome.rendered);
                } else {
                    log_line(&format!("[worker {index}] script returned; restarting"));
                }
            }
            Ok(Err(e)) => {
                log_line(&format!("[worker {index}] PHP Parse error: {e}"));
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            Err(_) => {
                log_line(&format!("[worker {index}] the runtime panicked; restarting"));
            }
        }
        // A request in hand dies with the script run: answer it with a 500.
        let orphan = reply.borrow_mut().take();
        if let Some(r) = orphan {
            let _ = r.send(php_types::sapi::WorkerResponse {
                status: 500,
                reason: None,
                headers: Vec::new(),
                body: Vec::new(),
                error_log: Vec::new(),
            });
        }
    }
}

/// Bind the listening socket; with `reuse_port`, SO_REUSEPORT lets several
/// ferro processes bind the same address (`--reuse-port`).
fn bind_listener(host: &str, port: u16, reuse_port: bool) -> std::io::Result<TcpListener> {
    if !reuse_port {
        return TcpListener::bind((host, port));
    }
    use std::net::ToSocketAddrs;
    let addr = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no address"))?;
    let sock = socket2::Socket::new(socket2::Domain::for_address(addr), socket2::Type::STREAM, None)?;
    sock.set_reuse_address(true)?;
    sock.set_reuse_port(true)?;
    sock.bind(&addr.into())?;
    sock.listen(1024)?;
    Ok(sock.into())
}
