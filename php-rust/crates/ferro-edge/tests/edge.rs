//! End-to-end behaviour against a scripted origin: the query string tells the
//! origin what to answer (`ttl`, `swr`, `sie`, `tags`, `delay`, `private`,
//! `cookie`, `vary`, `etag`), and it counts the requests it served per path.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use ferro_edge::{Config, Edge};
use http::{HeaderMap, Method, Request, Response, StatusCode};
use http_body_util::{BodyExt, Empty, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;

#[derive(Default)]
struct Origin {
    hits: Mutex<HashMap<String, usize>>,
    /// Every request seen: (path, Cookie header, If-None-Match header).
    seen: Mutex<Vec<(String, Option<String>, Option<String>)>>,
    fail: AtomicBool,
}

impl Origin {
    fn hits(&self, path: &str) -> usize {
        self.hits.lock().unwrap().get(path).copied().unwrap_or(0)
    }
}

async fn origin_handle(o: Arc<Origin>, req: Request<hyper::body::Incoming>) -> Response<Full<Bytes>> {
    let path = req.uri().path().to_string();
    let q: HashMap<String, String> = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.replace('+', " "))))
        .collect();
    let hdr = |n: &str| req.headers().get(n).map(|v| v.to_str().unwrap().to_string());
    o.seen.lock().unwrap().push((path.clone(), hdr("cookie"), hdr("if-none-match")));
    let n = {
        let mut h = o.hits.lock().unwrap();
        let c = h.entry(path.clone()).or_default();
        *c += 1;
        *c
    };
    if let Some(ms) = q.get("delay") {
        tokio::time::sleep(Duration::from_millis(ms.parse().unwrap())).await;
    }
    let mut b = Response::builder();
    if o.fail.load(Ordering::SeqCst) {
        return b.status(503).body(Full::new(Bytes::from_static(b"down"))).unwrap();
    }
    let mut cc = match q.get("ttl") {
        Some(t) if q.contains_key("private") => format!("private, max-age={t}"),
        Some(t) => format!("public, max-age={t}"),
        None => "no-cache, private".to_string(),
    };
    if let Some(s) = q.get("swr") {
        cc += &format!(", stale-while-revalidate={s}");
    }
    if let Some(s) = q.get("sie") {
        cc += &format!(", stale-if-error={s}");
    }
    b = b.header("cache-control", cc);
    if let Some(t) = q.get("tags") {
        b = b.header("cache-tags", t.as_str());
    }
    if q.contains_key("cookie") {
        b = b.header("set-cookie", "SESSx=1");
    }
    if let Some(v) = q.get("vary") {
        b = b.header("vary", v.as_str());
    }
    if let Some(e) = q.get("etag") {
        let tag = format!("\"{e}\"");
        if hdr("if-none-match").as_deref() == Some(tag.as_str()) {
            return b.status(304).header("etag", tag).body(Full::new(Bytes::new())).unwrap();
        }
        b = b.header("etag", tag);
    }
    let lang = hdr("accept-language").unwrap_or_default();
    b.body(Full::new(Bytes::from(format!("{path} #{n} {lang}")))).unwrap()
}

async fn serve_on(listener: TcpListener, o: Arc<Origin>) {
    loop {
        let (s, _) = listener.accept().await.unwrap();
        let o = o.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |r| {
                let o = o.clone();
                async move { Ok::<_, Infallible>(origin_handle(o, r).await) }
            });
            let _ = http1::Builder::new().serve_connection(TokioIo::new(s), svc).await;
        });
    }
}

struct Harness {
    edge: SocketAddr,
    origin: Arc<Origin>,
    client: Client<hyper_util::client::legacy::connect::HttpConnector, Empty<Bytes>>,
}

struct Got {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Got {
    fn cache(&self) -> &str {
        self.headers.get("x-cache").map_or("", |v| v.to_str().unwrap())
    }
}

impl Harness {
    async fn start(tune: impl FnOnce(&mut Config)) -> Harness {
        let origin = Arc::new(Origin::default());
        let ol = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let oaddr = ol.local_addr().unwrap();
        tokio::spawn(serve_on(ol, origin.clone()));
        let mut cfg = Config { upstream: oaddr.to_string(), ..Config::default() };
        tune(&mut cfg);
        let el = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let edge = el.local_addr().unwrap();
        tokio::spawn(ferro_edge::run(Edge::new(cfg), el));
        let client = Client::builder(TokioExecutor::new()).build_http();
        Harness { edge, origin, client }
    }

    async fn req(&self, method: &str, path: &str, headers: &[(&str, &str)]) -> Got {
        let mut r = Request::builder().method(Method::from_bytes(method.as_bytes()).unwrap()).uri(format!("http://{}{path}", self.edge));
        for (k, v) in headers {
            r = r.header(*k, *v);
        }
        let resp = self.client.request(r.body(Empty::new()).unwrap()).await.unwrap();
        let (parts, body) = resp.into_parts();
        let body = String::from_utf8(body.collect().await.unwrap().to_bytes().to_vec()).unwrap();
        Got { status: parts.status, headers: parts.headers, body }
    }

    async fn get(&self, path: &str) -> Got {
        self.req("GET", path, &[]).await
    }
}

#[tokio::test]
async fn miss_then_hit() {
    let h = Harness::start(|_| {}).await;
    let a = h.get("/a?ttl=60").await;
    let b = h.get("/a?ttl=60").await;
    assert_eq!((a.cache(), b.cache()), ("MISS", "HIT"));
    assert_eq!(a.body, b.body);
    assert_eq!(h.origin.hits("/a"), 1);
    assert!(b.headers.contains_key("age"));
    let head = h.req("HEAD", "/a?ttl=60", &[]).await;
    assert_eq!((head.cache(), head.body.as_str()), ("HIT", ""));
    assert_eq!(head.headers["content-length"], a.body.len().to_string().as_str());
}

#[tokio::test]
async fn coalesces_concurrent_misses() {
    let h = Arc::new(Harness::start(|_| {}).await);
    let mut tasks = Vec::new();
    for _ in 0..20 {
        let h = h.clone();
        tasks.push(tokio::spawn(async move { h.get("/slow?ttl=60&delay=300").await.body }));
    }
    let bodies: Vec<String> = futures_join(tasks).await;
    assert!(bodies.iter().all(|b| b == &bodies[0]));
    assert_eq!(h.origin.hits("/slow"), 1);
}

async fn futures_join<T: Send + 'static>(tasks: Vec<tokio::task::JoinHandle<T>>) -> Vec<T> {
    let mut out = Vec::new();
    for t in tasks {
        out.push(t.await.unwrap());
    }
    out
}

#[tokio::test]
async fn uncacheable_responses_pass() {
    let h = Harness::start(|_| {}).await;
    for _ in 0..3 {
        assert_eq!(h.get("/p?ttl=60&private=1").await.cache(), "PASS");
        assert_eq!(h.get("/c?ttl=60&cookie=1").await.cache(), "PASS");
        assert_eq!(h.get("/n").await.cache(), "PASS");
    }
    assert_eq!((h.origin.hits("/p"), h.origin.hits("/c"), h.origin.hits("/n")), (3, 3, 3));
    // concurrent requests to a hit-for-pass URL are not serialized behind each other
    let h = Arc::new(h);
    let t0 = Instant::now();
    let tasks = (0..5).map(|_| {
        let h = h.clone();
        tokio::spawn(async move { h.get("/p?ttl=60&private=1&delay=200").await })
    });
    futures_join(tasks.collect()).await;
    assert!(t0.elapsed() < Duration::from_millis(900), "{:?}", t0.elapsed());
}

#[tokio::test]
async fn session_cookie_bypasses_and_other_cookies_are_stripped() {
    let h = Harness::start(|_| {}).await;
    let s = h.req("GET", "/s?ttl=60", &[("cookie", "has_js=1; SESSabc=x")]).await;
    assert_eq!(s.cache(), "PASS");
    assert_eq!(h.get("/s?ttl=60").await.cache(), "MISS");
    assert_eq!(h.req("GET", "/s?ttl=60", &[("cookie", "has_js=1")]).await.cache(), "HIT");
    assert_eq!(h.req("GET", "/s?ttl=60", &[("authorization", "Basic eDp5")]).await.cache(), "PASS");
    let seen = h.origin.seen.lock().unwrap().clone();
    assert_eq!(seen[0].1.as_deref(), Some("has_js=1; SESSabc=x"));
    assert_eq!(seen[1].1, None);
}

#[tokio::test]
async fn ban_by_cache_tags_varnish_semantics() {
    let h = Harness::start(|_| {}).await;
    let t1 = h.get("/t1?ttl=60&tags=node:1+node_list").await;
    h.get("/t2?ttl=60&tags=node:10").await;
    h.get("/t3?ttl=60&tags=user:1").await;
    assert!(!t1.headers.contains_key("cache-tags"), "tag header leaked to the client");
    // anchored: exactly node:1
    let b = h.req("BAN", "/", &[("purge-cache-tags", r"(^|\s)node:1(\s|$)")]).await;
    assert_eq!((b.status, b.headers["x-edge-banned"].to_str().unwrap()), (StatusCode::OK, "1"));
    assert_eq!(h.get("/t2?ttl=60&tags=node:10").await.cache(), "HIT");
    // unanchored, as Varnish: node:1 also matches node:10
    let b = h.req("BAN", "/", &[("cache-tags", "node:1")]).await;
    assert_eq!(b.headers["x-edge-banned"], "1");
    assert_eq!(h.get("/t1?ttl=60&tags=node:1+node_list").await.cache(), "MISS");
    assert_eq!(h.get("/t2?ttl=60&tags=node:10").await.cache(), "MISS");
    assert_eq!(h.get("/t3?ttl=60&tags=user:1").await.cache(), "HIT");
    // exact keys
    let b = h.req("BAN", "/", &[("surrogate-key", "user:1 nothing")]).await;
    assert_eq!(b.headers["x-edge-banned"], "1");
    assert_eq!(h.get("/t3?ttl=60&tags=user:1").await.cache(), "MISS");
    // everything
    let b = h.req("BAN", "/", &[("cache-tags", ".*")]).await;
    assert_eq!(b.headers["x-edge-banned"], "3");
}

#[tokio::test]
async fn ban_by_url_and_purge() {
    let h = Harness::start(|_| {}).await;
    h.get("/u/1?ttl=60").await;
    h.get("/u/2?ttl=60").await;
    h.get("/other?ttl=60").await;
    let b = h.req("BAN", "/", &[("x-url", "^/u/")]).await;
    assert_eq!(b.headers["x-edge-banned"], "2");
    assert_eq!(h.get("/other?ttl=60").await.cache(), "HIT");
    let p = h.req("PURGE", "/other?ttl=60", &[]).await;
    assert_eq!((p.body.as_str(), p.headers["x-edge-banned"].to_str().unwrap()), ("Purged.\n", "1"));
    assert_eq!(h.get("/other?ttl=60").await.cache(), "MISS");
    assert_eq!(h.req("BAN", "/", &[]).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(h.req("BAN", "/", &[("cache-tags", "(")]).await.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn ban_needs_an_allowed_address() {
    let h = Harness::start(|c| c.ban_allow = vec![]).await;
    assert_eq!(h.req("BAN", "/", &[("cache-tags", ".*")]).await.status, StatusCode::FORBIDDEN);
    assert_eq!(h.req("PURGE", "/", &[]).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn stale_while_revalidate() {
    let h = Harness::start(|_| {}).await;
    let first = h.get("/w?ttl=1&swr=30&delay=100").await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let t0 = Instant::now();
    let stale = h.get("/w?ttl=1&swr=30&delay=100").await;
    assert!(t0.elapsed() < Duration::from_millis(80), "stale was not served at once");
    assert_eq!((stale.cache(), stale.body.as_str()), ("STALE", first.body.as_str()));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let fresh = h.get("/w?ttl=1&swr=30&delay=100").await;
    assert_eq!(fresh.cache(), "HIT");
    assert!(fresh.body.contains("#2"), "{}", fresh.body);
    assert_eq!(h.origin.hits("/w"), 2);
}

#[tokio::test]
async fn revalidation_uses_the_etag() {
    let h = Harness::start(|_| {}).await;
    let first = h.get("/e?ttl=1&swr=30&etag=v1").await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(h.get("/e?ttl=1&swr=30&etag=v1").await.cache(), "STALE");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after = h.get("/e?ttl=1&swr=30&etag=v1").await;
    assert_eq!((after.cache(), after.body.as_str()), ("HIT", first.body.as_str()));
    let seen = h.origin.seen.lock().unwrap().clone();
    assert_eq!(seen[1].2.as_deref(), Some("\"v1\""));
    // and a client's own conditional request is answered from the cache
    let nm = h.req("GET", "/e?ttl=1&swr=30&etag=v1", &[("if-none-match", "\"v1\"")]).await;
    assert_eq!((nm.status, nm.body.as_str()), (StatusCode::NOT_MODIFIED, ""));
}

#[tokio::test]
async fn stale_if_error() {
    let h = Harness::start(|c| c.grace = 0).await;
    let first = h.get("/f?ttl=1&sie=60").await;
    h.origin.fail.store(true, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let s = h.get("/f?ttl=1&sie=60").await;
    assert_eq!((s.status, s.cache(), s.body.as_str()), (StatusCode::OK, "STALE", first.body.as_str()));
    // without stale-if-error the failure goes through
    h.origin.fail.store(false, Ordering::SeqCst);
    h.get("/g?ttl=1").await;
    h.origin.fail.store(true, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(h.get("/g?ttl=1").await.status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn vary_keeps_variants_apart() {
    let h = Harness::start(|_| {}).await;
    let q = "/v?ttl=60&vary=Accept-Language";
    let en = h.req("GET", q, &[("accept-language", "en")]).await;
    let de = h.req("GET", q, &[("accept-language", "de")]).await;
    assert_eq!((en.cache(), de.cache()), ("MISS", "MISS"));
    let en2 = h.req("GET", q, &[("accept-language", "en")]).await;
    assert_eq!((en2.cache(), en2.body.as_str()), ("HIT", en.body.as_str()));
    assert!(de.body.ends_with(" de"));
}

#[tokio::test]
async fn a_ban_during_a_fetch_keeps_its_object_out() {
    let h = Arc::new(Harness::start(|_| {}).await);
    let h2 = h.clone();
    let slow = tokio::spawn(async move { h2.get("/b?ttl=60&delay=300&tags=x").await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    h.req("BAN", "/", &[("cache-tags", "x")]).await;
    assert_eq!(slow.await.unwrap().cache(), "MISS");
    assert_eq!(h.get("/b?ttl=60&delay=300&tags=x").await.cache(), "MISS");
    assert_eq!(h.origin.hits("/b"), 2);
}

#[tokio::test]
async fn other_methods_pass_and_stats_answer() {
    let h = Harness::start(|_| {}).await;
    assert_eq!(h.req("POST", "/x?ttl=60", &[]).await.cache(), "PASS");
    assert_eq!(h.req("POST", "/x?ttl=60", &[]).await.cache(), "PASS");
    assert_eq!(h.origin.hits("/x"), 2);
    let s = h.req("STATS", "/", &[]).await;
    assert!(s.body.starts_with("hit 0 stale 0 miss 0 coalesced 0 pass 2 upstream 2"), "{}", s.body);
}
