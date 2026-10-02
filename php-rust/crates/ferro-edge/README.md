# ferro-edge

A cache-tag aware HTTP reverse proxy for PHP applications, Drupal first. It
fronts any origin — nginx + php-fpm, `ferro -S`, FrankenPHP — and does not
depend on the engine. Safe Rust on tokio + hyper.

```bash
ferro-edge --upstream 127.0.0.1:8080 --listen 0.0.0.0:80
ferro-edge --help
```

## What it caches

A `GET`/`HEAD` response is stored when the origin marks it public:
`Cache-Control` with `s-maxage` or `max-age` > 0 (or `--default-ttl`), not
`private`/`no-store`/`no-cache`, no `Set-Cookie`, no `Vary: *`, and a
heuristically cacheable status (200, 203, 204, 300, 301, 308, 404, 405, 410,
414, 501). `Vary` keeps variants apart.

Requests that carry `Authorization` or a session cookie (`--session-cookies`,
default `SESS`, `SSESS`, `NO_CACHE` prefixes: Drupal's session cookies) go
straight to the origin. Other cookies are stripped from cacheable requests,
as the usual Drupal Varnish VCL does (`--keep-cookies` to forward them).

For Drupal, set the page max-age (`system.performance cache.page.max_age`,
"Browser and proxy cache maximum age") and emit cache tags, either with the
Purge module's tags header (`purge_purger_http_tagsheader`: `Purge-Cache-Tags`)
or core's debug header (`http.response.debug_cacheability_headers: true` in
`services.yml`: `X-Drupal-Cache-Tags`). `Cache-Tags` and `Surrogate-Key` are
read too. Tag headers are removed from client responses (`--expose-tags`).

## Invalidation

Allowed from `--ban-allow` (default loopback and private ranges):

| request | effect |
|---|---|
| `BAN` + `Purge-Cache-Tags: <regex>` or `Cache-Tags: <regex>` | removes objects whose space-joined tag list matches — Varnish's `ban("obj.http.Cache-Tags ~ " + req.http.Cache-Tags)`, so `node:1` also matches `node:10`; anchor it (`(^|\s)node:1(\s|$)`) for exact tags |
| `BAN` + `Surrogate-Key: a b c` | exact tags |
| `BAN` + `X-Url: <regex>` (or `Purge-Url`), optional `X-Host: <regex>` | URL (path and query) ban |
| `PURGE /path?query` | every variant of that URL |
| `STATS /` | counters (hit, stale, miss, coalesced, pass, upstream, errors, bans, objects, bytes) |

The answer carries `X-Edge-Banned: <n>`. A fetch in flight when a ban lands
is answered but not stored. A Purge "HTTP Bundled Purger" pointed at the edge
with method `BAN` and header `Purge-Cache-Tags: [invalidations:separator|]`
(or `Cache-Tags`) works as with Varnish.

## Under load

- **Coalescing**: concurrent misses on one URL make one origin request; the
  others wait for it. A waiter whose `Vary` values differ re-checks. A
  response that turns out uncacheable is streamed to the first waiter and the
  URL is remembered as hit-for-pass (`--hit-for-pass`, 120 s) so later
  requests go straight to the origin without queueing.
- **stale-while-revalidate**: past its TTL, an object is served for
  `stale-while-revalidate` seconds (or `--grace`, default 10) while one
  background request revalidates it, with `If-None-Match` when it has an
  ETag (a 304 refreshes it).
- **stale-if-error**: when the origin fails (connection error or 5xx), an
  object within `stale-if-error` (or `--stale-if-error`) is served instead.

Every response says what happened in `X-Cache` (`HIT`, `MISS`, `STALE`,
`PASS`) and carries `Age`. Memory is bounded by `--max-mb` (FIFO eviction) and
`--max-object-mb`.

Not handled: `Range` (a cached object is served whole), WebSocket upgrades,
HTTP/2, TLS (put it behind a TLS terminator).

Numbers: `bench/edge/bench-wrk-edge.sh`, results in `bench/results/*-edge-wrk-*.md`.
