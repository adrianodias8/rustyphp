# ferro-edge + Drupal 11 + Purge — origin: ferro — 2026-10-02T10:20:15Z

- ferro-edge 321c582d; origin `pr-ferro:8080` (ferro, 4 workers); Purge 8.x-3.7
- page max-age 3600, purge_queuer_coretags, HTTP Bundled Purger → `BAN http://edge:8081/`; the edit is `Node::load(1)->setTitle()->save()` (drush), the queue worked with `drush p:queue-work --finish`

### purger header `Purge-Cache-Tags: [invalidations:separated_pipe]` (regex)
    edge log: ferro-edge: BAN / [purge-cache-tags: node:1:revisions|node_list|node_list:article|node:1] -> 7 objects
    new title on /node/1 through the edge: 2
    new title on /rss.xml through the edge: 2
invalidated tags (4): node:1 node:1:revisions node_list node_list:article

| page | cached before | exact match | Varnish regex match | edge after | expected | ok |
|---|---|---|---|---|---|---|
| `/` | HIT | yes | yes | MISS | MISS | ✓ |
| `/node` | HIT | yes | yes | MISS | MISS | ✓ |
| `/rss.xml` | HIT | yes | yes | MISS | MISS | ✓ |
| `/node/1` | HIT | yes | yes | MISS | MISS | ✓ |
| `/node/2` | HIT | no | no | HIT | HIT | ✓ |
| `/node/10` | HIT | no | yes | MISS | MISS | ✓ |
| `/node/11` | HIT | no | yes | MISS | MISS | ✓ |
| `/node/12` | HIT | no | yes | MISS | MISS | ✓ |
| `/user/login` | HIT | no | no | HIT | HIT | ✓ |

RESULT: PASS

### purger header `Surrogate-Key: [invalidations:separated_pipe]` (exact)
    edge log: ferro-edge: BAN / [surrogate-key: node:1:revisions|node_list|node_list:article|node:1] -> 4 objects
    new title on /node/1 through the edge: 2
    new title on /rss.xml through the edge: 2
invalidated tags (4): node:1 node:1:revisions node_list node_list:article

| page | cached before | exact match | Varnish regex match | edge after | expected | ok |
|---|---|---|---|---|---|---|
| `/` | HIT | yes | yes | MISS | MISS | ✓ |
| `/node` | HIT | yes | yes | MISS | MISS | ✓ |
| `/rss.xml` | HIT | yes | yes | MISS | MISS | ✓ |
| `/node/1` | HIT | yes | yes | MISS | MISS | ✓ |
| `/node/2` | HIT | no | no | HIT | HIT | ✓ |
| `/node/10` | HIT | no | yes | HIT | HIT | ✓ |
| `/node/11` | HIT | no | yes | HIT | HIT | ✓ |
| `/node/12` | HIT | no | yes | HIT | HIT | ✓ |
| `/user/login` | HIT | no | no | HIT | HIT | ✓ |

RESULT: PASS

### session-cookie bypass
cookie: SESSa1cb100f57e971cacf269e7c26e4630a=…
admin /: x-cache PASS, logged in (drupalSettings uid 1): 3
admin /node/1/edit: status HTTP/1.1 200 OK, x-cache PASS
anonymous /: x-cache HIT, shows the admin's page: 0

### wrk, front page, 0.1 of requests logged in (32 connections, 15s after a 10s warm-up with the same mix, 4 origin workers)

| target | req/s | p50 | p99 | edge: hit / pass / origin |
|---|---:|---:|---:|---|
| origin | 142.83 | 222.70ms | 244.33ms | 0 / 0 / 0 |
| edge | 1191.16 | 33.93ms | 257.31ms | 16075 / 1941 / 1941 |
