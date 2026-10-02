#!/usr/bin/env bash
# bench/drupal/phases.sh — where a classic (one request, fresh state) Drupal
# front-page request spends its time, per engine: bootstrap (autoload, kernel
# creation and boot: everything before $kernel->handle()), handle, send,
# terminate. bench/drupal/phases.php replaces web/index.php on copies of the
# base; one worker each, N sequential warm requests, medians of the last 50.
#   php-fpm   nginx -> php-fpm 8.5.7 + opcache (the wrk bench's config)
#   ferro     ferro -S, classic (a fresh Vm per request)
#   php-S     the oracle's built-in server, no opcache (reference)
# Run from the host:  bench/drupal/phases.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"; ROOT="$(cd "$REPO/.." && pwd)"
IMAGE="${RUSTYPHP_IMAGE:-rustyphp-dev:8.5.7}"
N="${N:-60}"; NET=rustyphp-phases
MOUNTS=(-v "$ROOT":/work:ro -v rustyphp-scratch:/scratch)
TMP="$(mktemp -d)"
cleanup() { for c in ph-fpm ph-nginx ph-ferro ph-phps; do docker rm -f "$c" >/dev/null 2>&1; done; docker network rm "$NET" >/dev/null 2>&1; }
trap cleanup EXIT; cleanup
docker network create "$NET" >/dev/null
docker run --rm "${MOUNTS[@]}" "$IMAGE" bash -c '
  for s in fpm fc ps; do rm -rf /scratch/drupal-b-$s; cp -a /scratch/drupal-base /scratch/drupal-b-$s;
    cp /work/php-rust/bench/drupal/phases.php /scratch/drupal-b-$s/web/index.php; chmod -R a+rwX /scratch/drupal-b-$s; done
  : > /scratch/phases.log; chmod 666 /scratch/phases.log'
sed "s/^pm.max_children = .*/pm.max_children = 1/; /SYMFONY_DIR/d" "$REPO/bench/worker/fpm/zz-bench.conf" >"$TMP/zz-bench.conf"
docker run -d --name ph-fpm --network "$NET" --network-alias fpm "${MOUNTS[@]}" -v "$TMP/zz-bench.conf":/usr/local/etc/php-fpm.d/zz-bench.conf:ro \
  -v "$REPO/bench/worker/fpm/opcache.ini":/usr/local/etc/php/conf.d/opcache.ini:ro php:8.5.7-fpm >/dev/null
docker run -d --name ph-nginx --network "$NET" "${MOUNTS[@]}" -v "$HERE/nginx/drupal.conf":/etc/nginx/conf.d/default.conf:ro nginx:alpine >/dev/null
docker run -d --name ph-ferro --network "$NET" "${MOUNTS[@]}" -v rustyphp-target:/target:ro "$IMAGE" bash -c \
  "cd /scratch/drupal-b-fc/web && exec /target/release/ferro -S 0.0.0.0:8080 -t /scratch/drupal-b-fc/web .ht.router.php 2>/dev/null" >/dev/null
docker run -d --name ph-phps --network "$NET" "${MOUNTS[@]}" "$IMAGE" bash -c \
  "cd /scratch/drupal-b-ps/web && exec php -S 0.0.0.0:8080 .ht.router.php 2>/dev/null" >/dev/null
drive() {  # host, then N sequential requests
  docker run --rm --network "$NET" "$IMAGE" bash -c "
    for i in \$(seq 1 60); do curl -s -o /dev/null -f http://$1:8080/ && break; sleep 1; done
    for i in \$(seq 1 $N); do curl -s -o /dev/null http://$1:8080/; done"
}
# nginx listens on 8080 in drupal.conf
drive ph-nginx; drive ph-ferro; drive ph-phps
docker run --rm "${MOUNTS[@]}" "$IMAGE" python3 -c '
import collections, statistics
rows = collections.defaultdict(list)
for l in open("/scratch/phases.log"):
    f = l.split()
    if len(f) == 7:
        rows[f[0]].append([float(x) for x in f[1:6]])
print("| engine | requests | bootstrap | handle | send | terminate | total |")
print("|---|---:|---:|---:|---:|---:|---:|")
for eng, v in rows.items():
    v = v[-50:]
    med = [statistics.median(c) for c in zip(*v)]
    print(f"| {eng} | {len(v)} | " + " | ".join(f"{m:.2f} ms" for m in med) + " |")
'
