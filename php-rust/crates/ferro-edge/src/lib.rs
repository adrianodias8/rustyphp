//! ferro-edge: a cache-tag aware reverse proxy for PHP applications.
//!
//! It sits in front of any HTTP origin (nginx + php-fpm, `ferro -S`) and
//! caches what the origin marks public, indexed by the cache tags the origin
//! sends (Drupal's `X-Drupal-Cache-Tags`, the Purge module's
//! `Purge-Cache-Tags`, `Cache-Tags`, `Surrogate-Key`). Invalidation speaks the
//! Varnish dialect Drupal's Purge purgers already use (see [`ban`]). A miss is
//! fetched once however many clients ask (request coalescing); an expired
//! object is served while one background request revalidates it
//! (stale-while-revalidate), and while the origin fails if allowed
//! (stale-if-error). Requests with a session cookie or credentials go straight
//! to the origin.

pub mod ban;
pub mod cache;
pub mod config;
pub mod policy;
pub mod proxy;

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

pub use config::Config;
pub use proxy::Edge;

/// Serves on `listener` until the process ends.
pub async fn run(edge: Arc<Edge>, listener: TcpListener) {
    if edge.cfg.stats_interval > 0 {
        let (e, every) = (edge.clone(), Duration::from_secs(edge.cfg.stats_interval));
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(every).await;
                eprintln!("ferro-edge: {}", e.stats_line());
            }
        });
    }
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                // EMFILE and friends: back off instead of spinning.
                eprintln!("ferro-edge: accept: {e}");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let _ = stream.set_nodelay(true);
        let edge = edge.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req| {
                let edge = edge.clone();
                async move { Ok::<_, Infallible>(edge.handle(req, peer).await) }
            });
            let _ = http1::Builder::new().serve_connection(TokioIo::new(stream), svc).await;
        });
    }
}
