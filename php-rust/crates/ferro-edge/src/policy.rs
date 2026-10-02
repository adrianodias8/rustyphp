//! What may be cached, for how long, and what crosses the proxy.

use std::net::SocketAddr;
use std::time::Duration;

use http::header::{self, HeaderMap, HeaderName, HeaderValue};
use http::StatusCode;

use crate::config::Config;

/// Response headers a cache-tag list is read from (space-separated): the Purge
/// module's (`purge_purger_http_tagsheader`), varnish_purger's, Drupal core's
/// debug header, and the Fastly-style surrogate key.
pub const TAG_HEADERS: [&str; 4] = ["purge-cache-tags", "cache-tags", "x-drupal-cache-tags", "surrogate-key"];

/// Lifetimes of a cacheable response.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lifetime {
    pub ttl: Duration,
    /// Serve stale while one background request revalidates.
    pub grace: Duration,
    /// Serve stale when the origin fails, counted past the TTL.
    pub stale_if_error: Duration,
}

#[derive(Default, Debug)]
pub struct CacheControl {
    pub no_store: bool,
    pub no_cache: bool,
    pub private: bool,
    pub max_age: Option<u64>,
    pub s_maxage: Option<u64>,
    pub stale_while_revalidate: Option<u64>,
    pub stale_if_error: Option<u64>,
}

impl CacheControl {
    pub fn parse(h: &HeaderMap) -> CacheControl {
        let mut cc = CacheControl::default();
        for v in h.get_all(header::CACHE_CONTROL) {
            for d in v.to_str().unwrap_or("").split(',') {
                let (name, arg) = match d.split_once('=') {
                    Some((n, a)) => (n.trim(), a.trim().trim_matches('"').parse::<u64>().ok()),
                    None => (d.trim(), None),
                };
                match name.to_ascii_lowercase().as_str() {
                    "no-store" => cc.no_store = true,
                    "no-cache" => cc.no_cache = true,
                    "private" => cc.private = true,
                    "max-age" => cc.max_age = arg,
                    "s-maxage" => cc.s_maxage = arg,
                    "stale-while-revalidate" => cc.stale_while_revalidate = arg,
                    "stale-if-error" => cc.stale_if_error = arg,
                    _ => {}
                }
            }
        }
        cc
    }
}

/// `Some` when a response to a GET may be stored: a heuristically cacheable
/// status, no `Set-Cookie`, no `Vary: *`, not private/no-store/no-cache, and a
/// positive `s-maxage`/`max-age` (or the configured default TTL).
pub fn cacheable(status: StatusCode, h: &HeaderMap, cfg: &Config) -> Option<Lifetime> {
    if !matches!(status.as_u16(), 200 | 203 | 204 | 300 | 301 | 308 | 404 | 405 | 410 | 414 | 501) {
        return None;
    }
    if h.contains_key(header::SET_COOKIE) || vary_names(h).is_none() {
        return None;
    }
    let cc = CacheControl::parse(h);
    if cc.no_store || cc.no_cache || cc.private {
        return None;
    }
    let ttl = cc.s_maxage.or(cc.max_age).unwrap_or(cfg.default_ttl);
    (ttl > 0).then(|| Lifetime {
        ttl: Duration::from_secs(ttl),
        grace: Duration::from_secs(cc.stale_while_revalidate.unwrap_or(cfg.grace)),
        stale_if_error: Duration::from_secs(cc.stale_if_error.unwrap_or(cfg.stale_if_error)),
    })
}

/// The request headers a response varies on; `None` for `Vary: *`.
pub fn vary_names(h: &HeaderMap) -> Option<Vec<HeaderName>> {
    let mut names = Vec::new();
    for v in h.get_all(header::VARY) {
        for n in v.to_str().ok()?.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            if n == "*" {
                return None;
            }
            let name = HeaderName::from_bytes(n.as_bytes()).ok()?;
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    Some(names)
}

/// The cache tags a response carries, from every [`TAG_HEADERS`] header.
pub fn tags(h: &HeaderMap) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for name in TAG_HEADERS {
        for v in h.get_all(name) {
            for t in v.to_str().unwrap_or("").split_ascii_whitespace() {
                if !tags.iter().any(|x| x == t) {
                    tags.push(t.to_string());
                }
            }
        }
    }
    tags
}

/// A request that must not be answered from the shared cache: credentials or a
/// session cookie (Drupal's `SESS…`/`SSESS…`).
pub fn private_request(h: &HeaderMap, cfg: &Config) -> bool {
    if h.contains_key(header::AUTHORIZATION) {
        return true;
    }
    h.get_all(header::COOKIE).iter().any(|v| {
        v.to_str().unwrap_or("").split(';').any(|c| {
            let name = c.trim().split('=').next().unwrap_or("");
            cfg.session_cookies.iter().any(|p| name.starts_with(p.as_str()))
        })
    })
}

const HOP_BY_HOP: [HeaderName; 8] = [
    header::CONNECTION,
    HeaderName::from_static("keep-alive"),
    HeaderName::from_static("proxy-connection"),
    header::TRANSFER_ENCODING,
    header::TE,
    header::TRAILER,
    header::UPGRADE,
    header::PROXY_AUTHORIZATION,
];

/// Drops hop-by-hop headers, including those named in `Connection`.
pub fn strip_hop_by_hop(h: &mut HeaderMap) {
    let named: Vec<HeaderName> = h
        .get_all(header::CONNECTION)
        .iter()
        .flat_map(|v| v.to_str().unwrap_or("").split(',').map(str::trim).map(str::to_string).collect::<Vec<_>>())
        .filter_map(|n| HeaderName::from_bytes(n.as_bytes()).ok())
        .collect();
    for n in HOP_BY_HOP.iter().chain(&named) {
        h.remove(n);
    }
}

/// Request headers as sent to the origin: hop-by-hop dropped, the client's
/// address appended to `X-Forwarded-For`.
pub fn upstream_request_headers(h: &mut HeaderMap, peer: SocketAddr) {
    strip_hop_by_hop(h);
    let ip = peer.ip().to_canonical().to_string();
    let xff = match h.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        Some(prev) => format!("{prev}, {ip}"),
        None => ip,
    };
    if let Ok(v) = HeaderValue::from_str(&xff) {
        h.insert("x-forwarded-for", v);
    }
}

/// Response headers as sent to the client (and stored): hop-by-hop dropped,
/// cache-tag headers dropped unless exposed.
pub fn client_response_headers(h: &mut HeaderMap, cfg: &Config) {
    strip_hop_by_hop(h);
    if !cfg.expose_tags {
        for n in TAG_HEADERS {
            h.remove(n);
        }
    }
}

/// `If-None-Match` against a stored `ETag` (weak comparison, RFC 9110 13.1.2).
pub fn etag_matches(req: &HeaderMap, etag: Option<&HeaderValue>) -> bool {
    let (Some(inm), Some(etag)) = (req.get(header::IF_NONE_MATCH), etag) else { return false };
    let weak = |s: &str| s.trim().trim_start_matches("W/").to_string();
    let etag = weak(etag.to_str().unwrap_or(""));
    inm.to_str().unwrap_or("").split(',').any(|t| t.trim() == "*" || weak(t) == etag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.append(*k, HeaderValue::from_static(v));
        }
        m
    }

    #[test]
    fn drupal_headers() {
        let cfg = Config::default();
        let anon = h(&[("cache-control", "max-age=3600, public")]);
        let l = cacheable(StatusCode::OK, &anon, &cfg).unwrap();
        assert_eq!((l.ttl.as_secs(), l.grace.as_secs()), (3600, 10));
        // page max-age 0 (Drupal's default)
        assert!(cacheable(StatusCode::OK, &h(&[("cache-control", "must-revalidate, no-cache, private")]), &cfg).is_none());
        assert!(cacheable(StatusCode::OK, &h(&[("cache-control", "max-age=60"), ("set-cookie", "a=b")]), &cfg).is_none());
        assert!(cacheable(StatusCode::OK, &h(&[("cache-control", "max-age=60"), ("vary", "*")]), &cfg).is_none());
        assert!(cacheable(StatusCode::INTERNAL_SERVER_ERROR, &anon, &cfg).is_none());
        let s = h(&[("cache-control", "public, max-age=10, s-maxage=99, stale-while-revalidate=5, stale-if-error=7")]);
        let l = cacheable(StatusCode::OK, &s, &cfg).unwrap();
        assert_eq!((l.ttl.as_secs(), l.grace.as_secs(), l.stale_if_error.as_secs()), (99, 5, 7));
    }

    #[test]
    fn session_cookies_and_tags() {
        let cfg = Config::default();
        assert!(private_request(&h(&[("cookie", "has_js=1; SESSabc=x")]), &cfg));
        assert!(!private_request(&h(&[("cookie", "has_js=1")]), &cfg));
        let t = tags(&h(&[("x-drupal-cache-tags", "node:1 node_list"), ("cache-tags", "node_list http_response")]));
        assert_eq!(t, ["node_list", "http_response", "node:1"]); // TAG_HEADERS order, deduplicated
    }

    #[test]
    fn etags() {
        let e = HeaderValue::from_static("W/\"abc\"");
        assert!(etag_matches(&h(&[("if-none-match", "\"x\", \"abc\"")]), Some(&e)));
        assert!(!etag_matches(&h(&[("if-none-match", "\"x\"")]), Some(&e)));
    }
}
