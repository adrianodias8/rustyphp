//! Zygote mode (`ferro -S host:port --worker boot.php --zygote [--workers N]`):
//! N single-threaded zygote processes, each running `boot.php` once — the
//! application boots up to its handle boundary, then calls
//! `ferro_handle_request($handler)`. In the zygote, that call never returns:
//! it accepts a connection, reads the request and `fork()`s; the child gets
//! the request back, runs `$handler` on the inherited post-boot state (unit
//! caches, declared classes, the booted kernel — copy-on-write), writes the
//! response and exits. Nothing a request does survives it (php-fpm's
//! isolation), and nothing boot did is repeated (a worker's boot cost).
//! One request is in flight per zygote, like a php-fpm child.
//!
//! `FERRO_ZYGOTE_STATS=1` logs the fork cost every 100 requests: the
//! `fork()` call in the zygote, and fork → child running (page-table copy
//! and the first faults).

use super::*;
use std::cell::RefCell;
use std::io::Read;
use std::os::unix::net::UnixStream;

thread_local! {
    /// In a request child: the connection and what its response needs.
    /// The request child's connection, its pending response metadata, and its
    /// end of the done-pipe (one byte once the response is out).
    static CHILD: RefCell<Option<(TcpStream, PendingRequest, UnixStream)>> = const { RefCell::new(None) };
}

/// Children served by a zygote's first generation before it re-boots on
/// settled application caches (with `--max-requests`).
const FIRST_GENERATION: u64 = 32;

/// The supervisor: fork the zygotes, restart any that dies.
/// `max_requests` (0 = never) re-boots a zygote after that many children: a
/// zygote is a snapshot, and state it captured at boot that the application
/// later invalidates (Drupal's caches) would otherwise be recomputed by every
/// child for good. Zygote `i` recycles after `max_requests * (n + i) / n`
/// children, so the zygotes do not all re-boot at once; its first generation
/// after only `FIRST_GENERATION` children, since a fresh server's first
/// requests are the ones that fill the application's caches.
pub(super) fn serve_zygote(listener: TcpListener, cfg: ServerConfig, script: PathBuf, n: usize, max_requests: u64) -> u8 {
    let source = match std::fs::read(&script) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Could not open worker script {}: {e}", script.display());
            return 1;
        }
    };
    log_line(&format!(
        "ferro zygote server (http://{}:{}) started: {n} zygotes booting {}{}",
        cfg.host,
        cfg.port,
        script.display(),
        if max_requests > 0 { format!(", re-booted every ~{max_requests} requests") } else { String::new() }
    ));
    let limit = |i: usize, generation: u32| match max_requests {
        0 => 0,
        m if generation == 0 => m.min(FIRST_GENERATION),
        m => m * (n + i) as u64 / n as u64,
    };
    let mut generation = vec![0u32; n];
    // pid -> (zygote index, when it started); a zygote that dies within a
    // few seconds of starting (a failing boot script) is restarted with an
    // exponential backoff, so a broken boot cannot spin.
    let mut zygotes: std::collections::HashMap<i32, (usize, std::time::Instant)> = std::collections::HashMap::new();
    let mut backoff = vec![std::time::Duration::from_millis(200); n];
    for i in 0..n {
        if let Some(pid) = spawn_zygote(i, &listener, &cfg, &script, &source, limit(i, 0)) {
            zygotes.insert(pid, (i, std::time::Instant::now()));
        }
    }
    loop {
        let mut status = 0;
        // SAFETY: waitpid on any child with a valid out-pointer; the
        // supervisor is single-threaded and owns its children.
        let pid = unsafe { libc::waitpid(-1, &mut status, 0) };
        if pid <= 0 {
            return 1;
        }
        if let Some((i, started)) = zygotes.remove(&pid) {
            let planned = libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0;
            if planned || started.elapsed() > std::time::Duration::from_secs(5) {
                backoff[i] = std::time::Duration::from_millis(200);
            }
            generation[i] += 1;
            if planned {
                // a planned recycle (--max-requests): re-boot at once
                log_line(&format!("[zygote {i}] recycled; re-booting"));
            } else {
                log_line(&format!("[zygote {i}] exited (status {status}); restarting in {:?}", backoff[i]));
                std::thread::sleep(backoff[i]);
                backoff[i] = (backoff[i] * 2).min(std::time::Duration::from_secs(10));
            }
            if let Some(pid) = spawn_zygote(i, &listener, &cfg, &script, &source, limit(i, generation[i])) {
                zygotes.insert(pid, (i, std::time::Instant::now()));
            }
        }
    }
}

fn spawn_zygote(i: usize, listener: &TcpListener, cfg: &ServerConfig, script: &PathBuf, source: &[u8], max_requests: u64) -> Option<i32> {
    // SAFETY: the supervisor has not started any thread (it only binds,
    // forks and waits), so the child is a complete single-threaded copy.
    match unsafe { libc::fork() } {
        0 => {
            zygote_main(i, listener, cfg, script, source, max_requests);
            // Status 0 only for a planned recycle; a boot script that ended
            // on its own (a fatal, a loop that stopped) is a failure.
            let code = if RECYCLING.with(|r| r.get()) { 0 } else { 1 };
            // SAFETY: leave without running the supervisor's atexit state.
            unsafe { libc::_exit(code) }
        }
        -1 => {
            log_line(&format!("[zygote {i}] fork failed"));
            None
        }
        pid => Some(pid),
    }
}

/// One zygote: boot once, then serve each request in a forked child.
fn zygote_main(i: usize, listener: &TcpListener, cfg: &ServerConfig, script: &PathBuf, source: &[u8], max_requests: u64) {
    php_types::sapi::set_sapi_name("cli-server");
    let registry = php_builtins::registry();
    let listener = match listener.try_clone() {
        Ok(l) => l,
        Err(_) => return,
    };
    let cfg = ServerConfig { host: cfg.host.clone(), port: cfg.port, docroot: cfg.docroot.clone(), router: None, router_arg: None };
    let script_path = script.clone();
    let stats = std::env::var_os("FERRO_ZYGOTE_STATS").is_some();
    let mut served: u64 = 0;
    let (mut fork_ns, mut start_ns, mut faults): (u128, u128, u64) = (0, 0, 0);
    let next_request = Box::new(move || -> Option<WebRequest> {
        // The zygote never returns from here but to end: each request returns
        // in its forked child; the zygote loops.
        loop {
            if max_requests > 0 && served >= max_requests {
                RECYCLING.with(|r| r.set(true));
                return None; // ends the boot script's loop; the supervisor re-boots
            }
            let Ok((mut stream, addr)) = listener.accept() else { continue };
            let peer = (addr.ip().to_string(), addr.port());
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(60)));
            let _ = stream.set_nodelay(true);
            let Some(req) = read_request(&mut stream) else { continue };
            let (web, mut meta) = worker_web_request(req, &peer, &cfg, &script_path);
            meta.keep_alive = false; // one request per connection, like `php -S`
            // The child says "response sent" on this pipe; the zygote then serves the
            // next request while the child's exit (unmapping its copy of the whole
            // address space) proceeds in parallel, instead of on the critical path.
            let Ok((done_tx, mut done_rx)) = UnixStream::pair() else { continue };
            let t0 = std::time::Instant::now();
            let t0_wall = monotonic_ns();
            // SAFETY: the zygote is single-threaded (the VM runs on this
            // thread and starts none), so fork() copies a consistent process.
            match unsafe { libc::fork() } {
                0 => {
                    if stats {
                        // The child reports when it starts running.
                        let _ = CHILD_START.with(|c| c.set(monotonic_ns().saturating_sub(t0_wall)));
                    }
                    drop(done_rx);
                    CHILD.with(|c| *c.borrow_mut() = Some((stream, meta, done_tx)));
                    return Some(web);
                }
                -1 => {
                    let _ = stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
                pid => {
                    fork_ns += t0.elapsed().as_nanos();
                    drop(stream);
                    drop(done_tx);
                    // One byte when the response is out; EOF if the child died first.
                    let _ = done_rx.read(&mut [0u8; 1]);
                    reap_children();
                    served += 1;
                    if stats {
                        let (st, flt) = read_child_start(pid);
                        start_ns += st;
                        faults += flt;
                        if served % 100 == 0 {
                            log_line(&format!(
                                "[zygote {i}] {served} requests: fork() {:.1} us, fork->child running {:.1} us, child minor faults {:.0} (mean)",
                                fork_ns as f64 / served as f64 / 1e3,
                                start_ns as f64 / served as f64 / 1e3,
                                faults as f64 / served as f64
                            ));
                        }
                    }
                }
            }
        }
    }) as Box<dyn FnMut() -> Option<WebRequest>>;
    let send_response = Box::new(move |resp: php_types::sapi::WorkerResponse| {
        for entry in &resp.error_log {
            log_line(&String::from_utf8_lossy(entry));
        }
        let mut child = CHILD.with(|c| c.borrow_mut().take());
        if let Some((stream, meta, _)) = &mut child {
            let out = worker_response_bytes(meta, &resp);
            let _ = stream.write_all(&out);
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
        if stats {
            write_child_start();
        }
        if let Some((_, _, mut done)) = child {
            let _ = done.write_all(b"d");
        }
        // SAFETY: the request child is done; leave without unwinding the VM
        // (its state dies with the process, like a php-fpm request).
        unsafe { libc::_exit(0) }
    }) as Box<dyn FnMut(php_types::sapi::WorkerResponse)>;
    php_types::sapi::set_worker_hooks(php_types::sapi::WorkerHooks { next_request, send_response });
    let name = script.as_os_str().as_bytes().to_vec();
    match php_runtime::run_source_with_ini(&name, source, &registry, &[]) {
        Ok(outcome) => {
            if let Some(f) = &outcome.fatal {
                log_line(&format!("[zygote {i}] boot script ended with a fatal: {}", f.message()));
                let _ = std::io::stderr().write_all(&outcome.rendered);
            }
        }
        Err(e) => log_line(&format!("[zygote {i}] PHP Parse error: {e}")),
    }
}

/// Collect every request child that has exited (they finish tearing down
/// after signalling, so the zygote never blocks on them).
fn reap_children() {
    let mut status = 0;
    // SAFETY: non-blocking waitpid on our own children, with a valid out-pointer.
    while unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) } > 0 {}
}

thread_local! {
    /// Set when the zygote ends its loop for a planned recycle.
    static RECYCLING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CHILD_START: std::cell::Cell<u128> = const { std::cell::Cell::new(0) };
}

fn monotonic_ns() -> u128 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: clock_gettime with a valid out-pointer.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u128 * 1_000_000_000 + ts.tv_nsec as u128
}

/// Stats only: the child leaves its fork->running delay in a per-pid file
/// the zygote reads once the child has signalled (before it exits).
fn write_child_start() {
    let ns = CHILD_START.with(|c| c.get());
    // SAFETY: getrusage with a valid out-pointer.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    let _ = std::fs::write(
        format!("/tmp/ferro-zygote-{}", std::process::id()),
        format!("{ns} {}", ru.ru_minflt),
    );
}

/// (fork -> running ns, the child's minor page faults) of a finished child.
fn read_child_start(pid: i32) -> (u128, u64) {
    let path = format!("/tmp/ferro-zygote-{pid}");
    let s = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    let mut it = s.split_whitespace();
    (
        it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
        it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
    )
}
