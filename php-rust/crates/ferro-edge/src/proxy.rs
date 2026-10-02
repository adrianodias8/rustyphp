//! The request path: lookup, coalesced fetch, stale-while-revalidate,
//! stale-if-error, pass, BAN/PURGE.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http::header::{self, HeaderMap, HeaderValue};
use http::{Method, Request, Response, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full};
use hyper::body::Incoming;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use regex::Regex;
use tokio::sync::watch;

use crate::cache::{Entry, Lookup, Store};
use crate::config::Config;
use crate::{ban, policy};

pub type Body = BoxBody<Bytes, hyper::Error>;

fn full(b: Bytes) -> Body {
    Full::new(b).map_err(|never: Infallible| match never {}).boxed()
}
fn empty() -> Body {
    Empty::<Bytes>::new().map_err(|never: Infallible| match never {}).boxed()
}
fn text(status: StatusCode, msg: &str) -> Response<Body> {
    let mut r = Response::new(full(Bytes::copy_from_slice(msg.as_bytes())));
    *r.status_mut() = status;
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));
    r
}

#[derive(Default)]
pub struct Stats {
    pub hit: AtomicU64,
    pub stale: AtomicU64,
    pub miss: AtomicU64,
    /// Requests that waited on another request's fetch instead of their own.
    pub coalesced: AtomicU64,
    pub pass: AtomicU64,
    pub upstream: AtomicU64,
    pub errors: AtomicU64,
    pub bans: AtomicU64,
    pub banned: AtomicU64,
}

/// What one origin fetch produced, shared by every request that waited on it.
#[derive(Clone)]
enum Outcome {
    Entry(Arc<Entry>),
    /// The origin failed; a retained stale object stands in.
    Stale(Arc<Entry>),
    /// Not cacheable: the first taker streams the response, the others fetch
    /// their own.
    Pass(Arc<Mutex<Option<Response<Incoming>>>>),
    Error,
}

/// What a coalesced fetch sends upstream.
struct Head {
    pq: String,
    headers: HeaderMap,
}

pub struct Edge {
    pub cfg: Config,
    store: Mutex<Store>,
    inflight: Mutex<HashMap<Arc<str>, watch::Receiver<Option<Outcome>>>>,
    client: Client<HttpConnector, Body>,
    pub stats: Stats,
}

impl Edge {
    pub fn new(cfg: Config) -> Arc<Edge> {
        let mut conn = HttpConnector::new();
        conn.set_nodelay(true);
        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(1024)
            .build(conn);
        Arc::new(Edge {
            store: Mutex::new(Store::new(cfg.max_bytes)),
            inflight: Mutex::new(HashMap::new()),
            client,
            stats: Stats::default(),
            cfg,
        })
    }

    pub fn stats_line(&self) -> String {
        let s = &self.stats;
        let st = self.store.lock().unwrap();
        format!(
            "hit {} stale {} miss {} coalesced {} pass {} upstream {} errors {} bans {} banned {} objects {} bytes {}",
            s.hit.load(Relaxed),
            s.stale.load(Relaxed),
            s.miss.load(Relaxed),
            s.coalesced.load(Relaxed),
            s.pass.load(Relaxed),
            s.upstream.load(Relaxed),
            s.errors.load(Relaxed),
            s.bans.load(Relaxed),
            s.banned.load(Relaxed),
            st.len(),
            st.bytes()
        )
    }

    pub async fn handle(self: Arc<Self>, req: Request<Incoming>, peer: SocketAddr) -> Response<Body> {
        match req.method().as_str() {
            "BAN" | "PURGE" => return self.ban(&req, peer),
            "STATS" if self.cfg.ban_allowed(peer.ip()) => return text(StatusCode::OK, &(self.stats_line() + "\n")),
            "GET" | "HEAD" => {}
            _ => return self.pass(req, peer).await,
        }
        if policy::private_request(req.headers(), &self.cfg) {
            return self.pass(req, peer).await;
        }
        let (mut parts, _) = req.into_parts();
        if !self.cfg.keep_cookies {
            parts.headers.remove(header::COOKIE);
        }
        let key = cache_key(&parts.uri, &parts.headers);
        let head = parts.method == Method::HEAD;
        for _ in 0..3 {
            let now = Instant::now();
            let found = self.store.lock().unwrap().lookup(&key, &parts.headers, now);
            let fallback = match found {
                Lookup::Fresh(e) => {
                    self.stats.hit.fetch_add(1, Relaxed);
                    return serve(&e, head, &parts.headers, "HIT", now);
                }
                Lookup::Stale(e) => {
                    self.stats.stale.fetch_add(1, Relaxed);
                    self.join_or_lead(&key, &parts, peer, Some(e.clone()));
                    return serve(&e, head, &parts.headers, "STALE", now);
                }
                Lookup::Miss(fallback) => fallback,
            };
            if self.store.lock().unwrap().is_pass(&key, now) {
                break;
            }
            let (mut rx, leader) = self.join_or_lead(&key, &parts, peer, fallback);
            let outcome = match rx.wait_for(Option::is_some).await {
                Ok(o) => o.clone().unwrap(),
                Err(_) => Outcome::Error,
            };
            self.stats.miss.fetch_add(leader as u64, Relaxed);
            self.stats.coalesced.fetch_add(!leader as u64, Relaxed);
            match outcome {
                // A waiter whose Vary values differ from the fetcher's looks again.
                Outcome::Entry(e) if !e.matches(&parts.headers) => continue,
                Outcome::Entry(e) => return serve(&e, head, &parts.headers, "MISS", Instant::now()),
                Outcome::Stale(e) => return serve(&e, head, &parts.headers, "STALE", Instant::now()),
                Outcome::Pass(slot) => match slot.lock().unwrap().take() {
                    Some(resp) => {
                        self.stats.pass.fetch_add(1, Relaxed);
                        return self.client_response(resp, "PASS");
                    }
                    None => break,
                },
                Outcome::Error => return text(StatusCode::BAD_GATEWAY, "Bad Gateway\n"),
            }
        }
        let mut req = Request::new(empty());
        *req.method_mut() = parts.method;
        *req.uri_mut() = parts.uri;
        *req.headers_mut() = parts.headers;
        self.forward(req, peer).await
    }

    /// Joins the fetch in flight for `key`, or starts one (as a task, so a
    /// client that goes away does not cancel it for the others).
    fn join_or_lead(
        self: &Arc<Self>,
        key: &Arc<str>,
        parts: &http::request::Parts,
        peer: SocketAddr,
        stale: Option<Arc<Entry>>,
    ) -> (watch::Receiver<Option<Outcome>>, bool) {
        let mut inflight = self.inflight.lock().unwrap();
        if let Some(rx) = inflight.get(key) {
            return (rx.clone(), false);
        }
        let (tx, rx) = watch::channel(None);
        inflight.insert(key.clone(), rx.clone());
        drop(inflight);
        let mut headers = parts.headers.clone();
        for h in [header::IF_NONE_MATCH, header::IF_MODIFIED_SINCE, header::RANGE, header::IF_RANGE] {
            headers.remove(h);
        }
        policy::upstream_request_headers(&mut headers, peer);
        let pq = parts.uri.path_and_query().map_or("/", |p| p.as_str()).to_string();
        let (me, key) = (self.clone(), key.clone());
        tokio::spawn(async move {
            let outcome = me.fetch(&key, Head { pq, headers }, stale).await;
            me.inflight.lock().unwrap().remove(&key);
            let _ = tx.send(Some(outcome));
        });
        (rx, true)
    }

    async fn fetch(&self, key: &Arc<str>, head: Head, stale: Option<Arc<Entry>>) -> Outcome {
        self.stats.upstream.fetch_add(1, Relaxed);
        let gen = self.store.lock().unwrap().ban_gen;
        let mut req = Request::new(empty());
        *req.uri_mut() = match self.upstream_uri(&head.pq) {
            Some(u) => u,
            None => return Outcome::Error,
        };
        *req.headers_mut() = head.headers.clone();
        if let Some(etag) = stale.as_ref().and_then(|s| s.headers.get(header::ETAG)) {
            req.headers_mut().insert(header::IF_NONE_MATCH, etag.clone());
        }
        let resp = match self.client.request(req).await {
            Ok(r) => r,
            Err(_) => return self.on_error(stale),
        };
        let now = Instant::now();
        let status = resp.status();
        if status.is_server_error() && stale.as_ref().is_some_and(|s| s.usable_on_error(now)) {
            return self.on_error(stale);
        }
        if status == StatusCode::NOT_MODIFIED {
            if let Some(s) = stale {
                // Revalidated: the stored object, re-timed by the 304's headers.
                let life = policy::cacheable(StatusCode::OK, resp.headers(), &self.cfg).unwrap_or(s.life);
                let e = Arc::new(Entry { stored: now, life, headers: s.headers.clone(), body: s.body.clone(), tags: s.tags.clone(), tag_line: s.tag_line.clone(), host: s.host.clone(), url: s.url.clone(), vary: s.vary.clone(), status: s.status });
                return self.store(key, e, gen);
            }
        }
        let life = policy::cacheable(status, resp.headers(), &self.cfg);
        let too_big = resp
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok()?.parse::<usize>().ok())
            .is_some_and(|n| n > self.cfg.max_object);
        let Some(life) = life.filter(|_| !too_big) else {
            let mut st = self.store.lock().unwrap();
            st.purge_key(key);
            st.set_pass(key, now + Duration::from_secs(self.cfg.hit_for_pass));
            return Outcome::Pass(Arc::new(Mutex::new(Some(resp))));
        };
        let (mut parts, body) = resp.into_parts();
        let body = match body.collect().await {
            Ok(c) => c.to_bytes(),
            Err(_) => return self.on_error(stale),
        };
        let tags = policy::tags(&parts.headers);
        let vary = policy::vary_names(&parts.headers)
            .unwrap_or_default()
            .into_iter()
            .map(|n| {
                let v = head.headers.get(&n).cloned();
                (n, v)
            })
            .collect();
        policy::client_response_headers(&mut parts.headers, &self.cfg);
        parts.headers.remove(header::CONTENT_LENGTH);
        parts.headers.remove(header::AGE);
        let (host, url) = key.split_once('\n').map_or((String::new(), String::new()), |(h, u)| (h.into(), u.into()));
        let e = Arc::new(Entry {
            status,
            headers: parts.headers,
            tag_line: tags.join(" "),
            tags,
            host,
            url,
            vary,
            stored: Instant::now(),
            life,
            body,
        });
        if e.body.len() > self.cfg.max_object {
            return Outcome::Entry(e);
        }
        self.store(key, e, gen)
    }

    /// Stores unless a ban ran while the fetch was in flight (its object may
    /// predate the ban); the waiters get it either way.
    fn store(&self, key: &Arc<str>, e: Arc<Entry>, gen: u64) -> Outcome {
        let mut st = self.store.lock().unwrap();
        if st.ban_gen == gen {
            st.insert(key, e.clone());
        }
        Outcome::Entry(e)
    }

    fn on_error(&self, stale: Option<Arc<Entry>>) -> Outcome {
        self.stats.errors.fetch_add(1, Relaxed);
        match stale {
            Some(s) if s.usable_on_error(Instant::now()) => Outcome::Stale(s),
            _ => Outcome::Error,
        }
    }

    fn upstream_uri(&self, pq: &str) -> Option<http::Uri> {
        format!("http://{}{}", self.cfg.upstream, pq).parse().ok()
    }

    async fn pass(&self, req: Request<Incoming>, peer: SocketAddr) -> Response<Body> {
        self.forward(req.map(|b| b.boxed()), peer).await
    }

    /// Sends a request to the origin as is (streaming both ways).
    async fn forward(&self, mut req: Request<Body>, peer: SocketAddr) -> Response<Body> {
        self.stats.pass.fetch_add(1, Relaxed);
        self.stats.upstream.fetch_add(1, Relaxed);
        let pq = req.uri().path_and_query().map_or("/", |p| p.as_str()).to_string();
        let Some(uri) = self.upstream_uri(&pq) else { return text(StatusCode::BAD_REQUEST, "Bad Request\n") };
        if !req.headers().contains_key(header::HOST) {
            if let Some(h) = req.uri().authority().and_then(|a| HeaderValue::from_str(a.as_str()).ok()) {
                req.headers_mut().insert(header::HOST, h);
            }
        }
        *req.uri_mut() = uri;
        *req.version_mut() = http::Version::HTTP_11;
        policy::upstream_request_headers(req.headers_mut(), peer);
        match self.client.request(req).await {
            Ok(resp) => self.client_response(resp, "PASS"),
            Err(_) => {
                self.stats.errors.fetch_add(1, Relaxed);
                text(StatusCode::BAD_GATEWAY, "Bad Gateway\n")
            }
        }
    }

    fn client_response(&self, resp: Response<Incoming>, label: &'static str) -> Response<Body> {
        let (mut parts, body) = resp.into_parts();
        policy::client_response_headers(&mut parts.headers, &self.cfg);
        parts.headers.insert("x-cache", HeaderValue::from_static(label));
        Response::from_parts(parts, body.boxed())
    }

    fn ban(&self, req: &Request<Incoming>, peer: SocketAddr) -> Response<Body> {
        if !self.cfg.ban_allowed(peer.ip()) {
            return text(StatusCode::FORBIDDEN, "Forbidden.\n");
        }
        self.stats.bans.fetch_add(1, Relaxed);
        let h = req.headers();
        let mut st = self.store.lock().unwrap();
        let mut n = 0;
        if req.method().as_str() == "PURGE" {
            n = st.purge_key(&cache_key(req.uri(), h));
        } else {
            let mut any = false;
            let regex = |v: &HeaderValue| v.to_str().ok().and_then(|s| Regex::new(s).ok().map(|r| (r, s.to_string())));
            for name in ["purge-cache-tags", "cache-tags"] {
                if let Some(v) = h.get(name) {
                    let Some((re, src)) = regex(v) else { return text(StatusCode::BAD_REQUEST, "Bad tag regex.\n") };
                    n += st.ban_tag_regex(&re, ban::per_tag(&src));
                    any = true;
                }
            }
            if let Some(v) = h.get("surrogate-key") {
                // Space-separated (Fastly), or `|` / `,` (what Purge's
                // `[invalidations:separated_pipe|comma]` tokens produce).
                let keys: Vec<&str> = v
                    .to_str()
                    .unwrap_or("")
                    .split(|c: char| c.is_ascii_whitespace() || c == '|' || c == ',')
                    .filter(|k| !k.is_empty())
                    .collect();
                n += st.ban_tags(&keys);
                any = true;
            }
            if let Some(v) = h.get("x-url").or_else(|| h.get("purge-url")) {
                let Some((url, _)) = regex(v) else { return text(StatusCode::BAD_REQUEST, "Bad URL regex.\n") };
                let host = match h.get("x-host") {
                    Some(v) => match regex(v) {
                        Some((r, _)) => Some(r),
                        None => return text(StatusCode::BAD_REQUEST, "Bad host regex.\n"),
                    },
                    None => None,
                };
                n += st.ban_where(|e| url.is_match(&e.url) && host.as_ref().is_none_or(|r| r.is_match(&e.host)));
                any = true;
            }
            if !any {
                return text(StatusCode::BAD_REQUEST, "BAN needs Purge-Cache-Tags, Cache-Tags, Surrogate-Key or X-Url.\n");
            }
        }
        self.stats.banned.fetch_add(n as u64, Relaxed);
        if self.cfg.log_bans {
            let shown: Vec<String> = ["purge-cache-tags", "cache-tags", "surrogate-key", "x-url", "x-host"]
                .iter()
                .filter_map(|k| h.get(*k).map(|v| format!("{k}: {}", String::from_utf8_lossy(v.as_bytes()))))
                .collect();
            eprintln!("ferro-edge: {} {} [{}] -> {n} objects", req.method(), req.uri(), shown.join("; "));
        }
        let mut r = text(StatusCode::OK, if req.method().as_str() == "PURGE" { "Purged.\n" } else { "Ban added.\n" });
        r.headers_mut().insert("x-edge-banned", HeaderValue::from(n));
        r
    }
}

/// `host` (lower-cased) and path+query, one line each.
fn cache_key(uri: &http::Uri, h: &HeaderMap) -> Arc<str> {
    let host = h
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .or_else(|| uri.authority().map(|a| a.as_str()))
        .unwrap_or("")
        .to_ascii_lowercase();
    let pq = uri.path_and_query().map_or("/", |p| p.as_str());
    format!("{host}\n{pq}").into()
}

fn serve(e: &Entry, head: bool, req: &HeaderMap, label: &'static str, now: Instant) -> Response<Body> {
    let not_modified = policy::etag_matches(req, e.headers.get(header::ETAG));
    let mut r = Response::new(if head || not_modified { empty() } else { full(e.body.clone()) });
    *r.status_mut() = if not_modified { StatusCode::NOT_MODIFIED } else { e.status };
    *r.headers_mut() = e.headers.clone();
    let h = r.headers_mut();
    if !not_modified {
        h.insert(header::CONTENT_LENGTH, HeaderValue::from(e.body.len()));
    }
    h.insert(header::AGE, HeaderValue::from(e.age(now).as_secs()));
    h.insert("x-cache", HeaderValue::from_static(label));
    r
}
