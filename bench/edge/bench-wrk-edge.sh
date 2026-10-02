#!/usr/bin/env bash
# bench/edge/bench-wrk-edge.sh — ferro-edge in front of Drupal 11 (anonymous front
# page), against the same origins without it. Each origin runs on its own copy of
# /scratch/drupal-base with the page max-age at 3600 s (`system.performance
# cache.page.max_age`) and core's cache-tag header on
# (`http.response.debug_cacheability_headers`), so responses are public and
# tagged — what a Purge + Varnish site sends.
#   fpm            nginx -> php-fpm 8.5.7 + opcache, static pool of N
#   ferro          ferro -S classic pool, --workers N
#   edge-fpm       ferro-edge -> fpm         edge-ferro      ferro-edge -> ferro
#   edge-*-ban     the same, with a BAN `Purge-Cache-Tags: node_list` (a tag of
#                  the front page) every BAN_EVERY seconds during the run:
#                  misses, coalescing and refetches under invalidation
#   edge-*-pass    ferro-edge in front of an origin left at max-age 0: every
#                  request passes (the proxy's own cost)
# Run from the host:  WORKERS=4 bench/edge/bench-wrk-edge.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"; ROOT="$(cd "$REPO/.." && pwd)"
IMAGE="${RUSTYPHP_IMAGE:-rustyphp-dev:8.5.7}"
WORKERS="${WORKERS:-4}"; CONNS="${CONNS:-32}"; THREADS="${THREADS:-2}"; EDGE_THREADS="${EDGE_THREADS:-4}"
DURATION="${DURATION:-15}"; WARMUP="${WARMUP:-5s}"; R="${R:-3}"; BAN_EVERY="${BAN_EVERY:-0.1}"
ARMS="${ARMS:-fpm edge-fpm edge-fpm-ban edge-fpm-pass ferro edge-ferro edge-ferro-ban edge-ferro-pass}"
NET=rustyphp-edge
OUT="${OUT:-$REPO/bench/results/$(date -u +%Y-%m-%d)-edge-wrk-w$WORKERS.md}"
MOUNTS=(-v "$ROOT":/work:ro -v rustyphp-scratch:/scratch)
TMP="$(mktemp -d)"
CONTAINERS="e-fpm e-nginx e-ferro e-fpm0 e-nginx0 e-ferro0 edge-fpm edge-ferro edge-fpm0 edge-ferro0"
cleanup() { for c in $CONTAINERS; do docker rm -f "$c" >/dev/null 2>&1; done; docker network rm "$NET" >/dev/null 2>&1; }
trap cleanup EXIT; cleanup
docker network create "$NET" >/dev/null

# ---- copies: e-* cacheable (max-age 3600, tag header), e-*0 left at max-age 0 ----
docker run --rm "${MOUNTS[@]}" "$IMAGE" bash -c '
  set -e
  for s in fpm fc fpm0 fc0; do rm -rf /scratch/drupal-e-$s; cp -a /scratch/drupal-base /scratch/drupal-e-$s; done
  for s in fpm fc; do
    d=/scratch/drupal-e-$s
    sed "s/debug_cacheability_headers: false/debug_cacheability_headers: true/" $d/web/sites/default/default.services.yml >$d/web/sites/default/services.yml
    (cd $d && php vendor/drush/drush/drush.php -r web -q -y cset system.performance cache.page.max_age 3600 && php vendor/drush/drush/drush.php -r web -q cr)
  done
  chmod -R a+rwX /scratch/drupal-e-fpm /scratch/drupal-e-fc /scratch/drupal-e-fpm0 /scratch/drupal-e-fc0'

# ---- origins ----
sed "s/^pm.max_children = .*/pm.max_children = $WORKERS/; /SYMFONY_DIR/d" "$REPO/bench/worker/fpm/zz-bench.conf" >"$TMP/zz-bench.conf"
for v in "" 0; do
  sed "s#/scratch/drupal-b-fpm/#/scratch/drupal-e-fpm$v/#; s#server fpm:9000#server e-fpm$v:9000#" "$REPO/bench/drupal/nginx/drupal.conf" >"$TMP/nginx$v.conf"
  docker run -d --name "e-fpm$v" --network "$NET" "${MOUNTS[@]}" -v "$TMP/zz-bench.conf":/usr/local/etc/php-fpm.d/zz-bench.conf:ro \
    -v "$REPO/bench/worker/fpm/opcache.ini":/usr/local/etc/php/conf.d/opcache.ini:ro php:8.5.7-fpm >/dev/null
  docker run -d --name "e-nginx$v" --network "$NET" "${MOUNTS[@]}" -v "$TMP/nginx$v.conf":/etc/nginx/conf.d/default.conf:ro nginx:alpine >/dev/null
  docker run -d --name "e-ferro$v" --network "$NET" "${MOUNTS[@]}" -v rustyphp-target:/target:ro "$IMAGE" bash -c "
    cd /scratch/drupal-e-fc$v/web && exec /target/release/ferro -S 0.0.0.0:8080 -t /scratch/drupal-e-fc$v/web .ht.router.php --workers $WORKERS 2>/dev/null" >/dev/null
done
edge() {  # name, origin
  docker run -d --name "$1" --network "$NET" -v rustyphp-target:/target:ro "$IMAGE" \
    /target/release/ferro-edge --listen 0.0.0.0:8081 --upstream "$2" --threads "$EDGE_THREADS" >/dev/null
}
edge edge-fpm e-nginx:8080; edge edge-ferro e-ferro:8080; edge edge-fpm0 e-nginx0:8080; edge edge-ferro0 e-ferro0:8080

url_of() { case "$1" in
  fpm) echo http://e-nginx:8080/ ;; ferro) echo http://e-ferro:8080/ ;;
  edge-fpm|edge-fpm-ban) echo http://edge-fpm:8081/ ;; edge-ferro|edge-ferro-ban) echo http://edge-ferro:8081/ ;;
  edge-fpm-pass) echo http://edge-fpm0:8081/ ;; edge-ferro-pass) echo http://edge-ferro0:8081/ ;; esac; }
edge_of() { case "$1" in edge-fpm-pass) echo edge-fpm0 ;; edge-ferro-pass) echo edge-ferro0 ;; edge-fpm*) echo edge-fpm ;; edge-ferro*) echo edge-ferro ;; esac; }
in_net() { docker run --rm --network "$NET" "$IMAGE" "$@"; }
for u in http://e-nginx:8080/ http://e-ferro:8080/ http://e-nginx0:8080/ http://e-ferro0:8080/; do
  in_net bash -c "for i in \$(seq 1 60); do curl -s -o /dev/null -f $u && break; sleep 1; done; curl -s -o /dev/null $u; curl -s -o /dev/null $u"
done
# what the origin sends, and what the edge does with it
in_net bash -c 'curl -s -D- -o /dev/null http://e-nginx:8080/ | grep -iE "^(cache-control|x-drupal-cache-tags|vary)" | cut -c1-160' >"$TMP/origin-headers"
in_net bash -c 'curl -s -o /dev/null http://edge-fpm:8081/; curl -s -D- -o /dev/null http://edge-fpm:8081/ | grep -iE "^(x-cache|age|cache-control)"; curl -s -D- -o /dev/null http://edge-fpm0:8081/ | grep -i "^x-cache"' >"$TMP/edge-headers"
cat "$TMP/origin-headers" "$TMP/edge-headers" >&2

stats() { in_net curl -s -X STATS "http://$1:8081/" | awk '{for(i=1;i<NF;i+=2) if ($i=="upstream") print $(i+1)}'; }
median_of() { sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}'; }
lat_ms() { sed -E 's/([0-9.]+)(us|ms|s)$/\1 \2/' | awk '{v=$1; if ($2=="us") v/=1000; else if ($2=="s") v*=1000; print v}' | sort -n | awk '{a[NR]=$1} END{printf "%.2fms", a[int((NR+1)/2)]}'; }
for arm in $ARMS; do
  url="$(url_of "$arm")"; e="$(edge_of "$arm")"
  in_net wrk -t"$THREADS" -c"$CONNS" -d"$WARMUP" "$url" >/dev/null 2>&1
  : >"$TMP/$arm.rps"; : >"$TMP/$arm.p50"; : >"$TMP/$arm.p99"; : >"$TMP/$arm.origin"
  for i in $(seq 1 "$R"); do
    banner=""
    if [[ "$arm" == *-ban ]]; then
      docker run -d --name "banner" --network "$NET" "$IMAGE" bash -c \
        "while :; do curl -s -o /dev/null -X BAN -H 'Purge-Cache-Tags: node_list' http://$e:8081/; sleep $BAN_EVERY; done" >/dev/null
      banner=1
    fi
    before="$([[ -n "$e" ]] && stats "$e")"
    o="$(in_net wrk -t"$THREADS" -c"$CONNS" -d"${DURATION}s" --latency "$url" 2>&1)"
    after="$([[ -n "$e" ]] && stats "$e")"
    [[ -n "$banner" ]] && docker rm -f banner >/dev/null 2>&1
    rps="$(awk '/Requests\/sec/{print $2}' <<<"$o")"; echo "$rps" >>"$TMP/$arm.rps"
    awk '$1=="50%"{print $2}' <<<"$o" >>"$TMP/$arm.p50"; awk '$1=="99%"{print $2}' <<<"$o" >>"$TMP/$arm.p99"
    [[ -n "$e" ]] && awk -v a="$before" -v b="$after" -v d="$DURATION" 'BEGIN{printf "%.1f\n", (b-a)/d}' >>"$TMP/$arm.origin"
    echo "  $arm run $i: $rps req/s, origin $(tail -1 "$TMP/$arm.origin" 2>/dev/null) req/s $(grep -E 'Non-2xx|Socket errors' <<<"$o" | tr '\n' ' ')" >&2
  done
done
{
  echo "# ferro-edge in front of Drupal 11 — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo
  echo "- ferro-edge \`$(docker run --rm -v rustyphp-target:/target:ro "$IMAGE" sh -c 'sha256sum /target/release/ferro-edge | cut -c1-16')\` ($(git -C "$REPO" rev-parse --short HEAD)), $EDGE_THREADS threads; ferro classic \`--workers $WORKERS\`; php-fpm 8.5.7 + opcache, static pool of $WORKERS, nginx"
  echo "- anonymous front page, page max-age 3600 s, \`X-Drupal-Cache-Tags\` on; \`-pass\` arms: an identical origin left at max-age 0"
  echo "- \`-ban\` arms: \`BAN Purge-Cache-Tags: node_list\` every ${BAN_EVERY} s during the run"
  echo "- wrk: $THREADS threads, $CONNS connections (keep-alive), ${DURATION}s per run, $R runs (median), $WARMUP warm-up; $(docker run --rm "$IMAGE" nproc) CPUs in the VM, shared by wrk, the edge and the origins"
  echo "- origin: $(tr '\n' ';' <"$TMP/origin-headers" | cut -c1-300)"
  echo "- edge: $(tr '\n' ';' <"$TMP/edge-headers")"
  echo
  echo "| arm | req/s | p50 | p99 | origin req/s |"; echo "|---|---:|---:|---:|---:|"
  for arm in $ARMS; do
    org="-"; [[ -s "$TMP/$arm.origin" ]] && org="$(median_of <"$TMP/$arm.origin")"
    echo "| $arm | $(median_of <"$TMP/$arm.rps") | $(lat_ms <"$TMP/$arm.p50") | $(lat_ms <"$TMP/$arm.p99") | $org |"
  done
} | tee "$OUT"
echo "wrote $OUT"
