#!/usr/bin/env bash
# bench/drupal/bench-wrk-drupal.sh — step 3: the anonymous Drupal 11 front page
# (page_cache uninstalled, dynamic_page_cache on) under wrk, three servers, each
# on its own copy of /scratch/drupal-base (bench/drupal/README.md):
#   ferro-worker   ferro -S --worker drupal-worker.php, RESET=recipe (step 2)
#   ferro-classic  ferro -S .ht.router.php --workers N: N threads, a fresh Vm per
#                  request through Drupal's own index.php (php-fpm's model)
#   fpm            nginx -> php-fpm 8.5.7, opcache on, static pool of N, Drupal's index.php
#   frankenphp     FrankenPHP worker mode, the same drupal-worker.php + recipe
# Run from the host. Responses are checked against php-fpm's (token-stripped)
# before measuring. FrankenPHP knobs: FK_THREADS (global num_threads, default
# 2*N), FK_GOMAXPROCS (unset = Go's default).
#   WORKERS=4 CONNS=32 THREADS=2 DURATION=15s R=3 bench/drupal/bench-wrk-drupal.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"; ROOT="$(cd "$REPO/.." && pwd)"
IMAGE="${RUSTYPHP_IMAGE:-rustyphp-dev:8.5.7}"
WORKERS="${WORKERS:-4}"; CONNS="${CONNS:-32}"; THREADS="${THREADS:-2}"
DURATION="${DURATION:-15s}"; WARMUP="${WARMUP:-5s}"; R="${R:-3}"
FK_THREADS="${FK_THREADS:-$((2 * WORKERS))}"; FK_GOMAXPROCS="${FK_GOMAXPROCS:-}"
ARMS="${ARMS:-fpm frankenphp ferro-worker ferro-classic}"
NET=rustyphp-drupal
OUT="${OUT:-$REPO/bench/results/$(date -u +%Y-%m-%d)-drupal-wrk-w$WORKERS.md}"
cleanup() { for c in ferro-worker ferro-classic fpm nginx frankenphp; do docker rm -f "$c" >/dev/null 2>&1; done; docker network rm "$NET" >/dev/null 2>&1; }
trap cleanup EXIT; cleanup
docker network create "$NET" >/dev/null
MOUNTS=(-v "$ROOT":/work:ro -v rustyphp-scratch:/scratch)
TMP="$(mktemp -d)"

# ---- fresh copies of the base, writable by every server's user ----
docker run --rm "${MOUNTS[@]}" "$IMAGE" bash -c '
  for s in fe fc fpm fk; do rm -rf /scratch/drupal-b-$s; cp -a /scratch/drupal-base /scratch/drupal-b-$s; done
  cp /work/php-rust/bench/drupal/drupal-worker.php /scratch/drupal-b-fe/web/drupal-worker.php
  cp /work/php-rust/bench/drupal/drupal-worker.php /scratch/drupal-b-fk/web/index.php
  chmod -R a+rwX /scratch/drupal-b-fe /scratch/drupal-b-fc /scratch/drupal-b-fpm /scratch/drupal-b-fk'

# ---- servers ----
docker run -d --name ferro-worker --network "$NET" "${MOUNTS[@]}" -v rustyphp-target:/target:ro "$IMAGE" bash -c "
  cd /scratch/drupal-b-fe/web && DRUPAL_ROOT=/scratch/drupal-b-fe/web DRUPAL_WORKER_RESET=recipe \
  exec /target/release/ferro -S 0.0.0.0:8080 -t /scratch/drupal-b-fe/web --worker drupal-worker.php --workers $WORKERS" >/dev/null
docker run -d --name ferro-classic --network "$NET" "${MOUNTS[@]}" -v rustyphp-target:/target:ro "$IMAGE" bash -c "
  cd /scratch/drupal-b-fc/web && exec /target/release/ferro -S 0.0.0.0:8080 -t /scratch/drupal-b-fc/web .ht.router.php --workers $WORKERS 2>/dev/null" >/dev/null
sed "s/^pm.max_children = .*/pm.max_children = $WORKERS/; /SYMFONY_DIR/d" "$REPO/bench/worker/fpm/zz-bench.conf" >"$TMP/zz-bench.conf"
docker run -d --name fpm --network "$NET" "${MOUNTS[@]}" -v "$TMP/zz-bench.conf":/usr/local/etc/php-fpm.d/zz-bench.conf:ro \
  -v "$REPO/bench/worker/fpm/opcache.ini":/usr/local/etc/php/conf.d/opcache.ini:ro php:8.5.7-fpm >/dev/null
docker run -d --name nginx --network "$NET" "${MOUNTS[@]}" -v "$HERE/nginx/drupal.conf":/etc/nginx/conf.d/default.conf:ro nginx:alpine >/dev/null
cat >"$TMP/Caddyfile" <<CADDY
{
	auto_https off
	frankenphp {
		num_threads $FK_THREADS
		worker {
			file /scratch/drupal-b-fk/web/index.php
			num $WORKERS
			env DRUPAL_ROOT /scratch/drupal-b-fk/web
			env DRUPAL_WORKER_RESET recipe
		}
	}
}
:8080 {
	root * /scratch/drupal-b-fk/web
	php_server
}
CADDY
FKENV=(); [[ -n "$FK_GOMAXPROCS" ]] && FKENV=(-e "GOMAXPROCS=$FK_GOMAXPROCS")
docker run -d --name frankenphp --network "$NET" "${MOUNTS[@]}" ${FKENV[@]+"${FKENV[@]}"} -v "$TMP/Caddyfile":/etc/frankenphp/Caddyfile:ro dunglas/frankenphp:latest >/dev/null

host_of() { case "$1" in fpm) echo nginx ;; *) echo "$1" ;; esac; }
curl_in() { docker run --rm --network "$NET" "$IMAGE" curl -s "$@"; }
wrk_in() { docker run --rm --network "$NET" "$IMAGE" wrk "$@"; }
strip() { sed -E -e 's/form-[A-Za-z0-9_-]{43}/form-TOKEN/g' -e 's/data-drupal-selector="form-[a-z0-9-]+"/data-drupal-selector="form-TOKEN"/g' \
  -e 's/js-view-dom-id-[0-9a-f]{64}/js-view-dom-id-X/g' -e 's#http://[a-z-]+:8080#http://HOST#g'; }
for arm in $ARMS; do for i in $(seq 1 60); do curl_in -o /dev/null -f "http://$(host_of "$arm"):8080/" && break; sleep 1; done; done
# two warm-up requests each (cold caches), then the check
for arm in $ARMS; do curl_in -o /dev/null "http://$(host_of "$arm"):8080/"; curl_in -o /dev/null "http://$(host_of "$arm"):8080/"; done
ref="$(curl_in "http://nginx:8080/" | strip)"; check=""
for arm in $ARMS; do
  [[ "$arm" == fpm ]] && continue
  got="$(curl_in "http://$(host_of "$arm"):8080/" | strip)"
  if [[ "$got" == "$ref" ]]; then check="$check$arm: identical to php-fpm; "; else check="$check$arm: DIFFERS from php-fpm ($(diff <(echo "$ref") <(echo "$got") | grep -c '^[<>]') lines); "; fi
done
echo "check: $check" >&2

median_of() { sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}'; }
lat_ms() { sed -E 's/([0-9.]+)(us|ms|s)$/\1 \2/' | awk '{v=$1; if ($2=="us") v/=1000; else if ($2=="s") v*=1000; print v}' | sort -n | awk '{a[NR]=$1} END{printf "%.1fms", a[int((NR+1)/2)]}'; }
for arm in $ARMS; do
  url="http://$(host_of "$arm"):8080/"
  wrk_in -t"$THREADS" -c"$CONNS" -d"$WARMUP" "$url" >/dev/null 2>&1
  : >"$TMP/$arm.rps"; : >"$TMP/$arm.p50"; : >"$TMP/$arm.p99"
  for i in $(seq 1 "$R"); do
    o="$(wrk_in -t"$THREADS" -c"$CONNS" -d"$DURATION" --latency "$url" 2>&1)"
    awk '/Requests\/sec/{print $2}' <<<"$o" >>"$TMP/$arm.rps"
    awk '$1=="50%"{print $2}' <<<"$o" >>"$TMP/$arm.p50"; awk '$1=="99%"{print $2}' <<<"$o" >>"$TMP/$arm.p99"
    echo "  $arm run $i: $(awk '/Requests\/sec/{print $2}' <<<"$o") req/s $(grep -E 'Non-2xx|Socket errors' <<<"$o" | tr '\n' ' ')" >&2
  done
done
{
  echo "# Drupal 11 front page under wrk — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo
  echo "- ferro: \`$(docker run --rm -v rustyphp-target:/target:ro "$IMAGE" sh -c 'sha256sum /target/release/ferro | cut -c1-16')\` ($(git -C "$REPO" rev-parse --short HEAD)), worker mode, RESET=recipe; classic: \`--workers $WORKERS\`, fresh Vm per request, one request per connection (no keep-alive, like \`php -S\`)"
  echo "- php-fpm: \`$(docker exec fpm php-fpm -v 2>&1 | head -1)\`, opcache on (validate_timestamps=0, jit off), static pool of $WORKERS, nginx"
  echo "- FrankenPHP: \`$(docker exec frankenphp frankenphp version 2>/dev/null | head -1)\`, $WORKERS workers, num_threads $FK_THREADS, GOMAXPROCS ${FK_GOMAXPROCS:-default}"
  echo "- wrk: $THREADS threads, $CONNS connections, $DURATION per run, $R runs (median), $WARMUP warm-up; $(docker run --rm "$IMAGE" nproc) CPUs in the VM, shared by wrk and the servers"
  echo "- check (token-stripped body of \`/\`): $check"
  echo
  echo "| server | req/s | p50 | p99 | ferro ÷ this |"; echo "|---|---:|---:|---:|---:|"
  fe=""; for a in ferro-worker ferro-classic; do [[ -s "$TMP/$a.rps" ]] && { fe="$(median_of <"$TMP/$a.rps")"; break; }; done
  for arm in $ARMS; do
    rps="$(median_of <"$TMP/$arm.rps")"
    echo "| $arm | $rps | $(lat_ms <"$TMP/$arm.p50") | $(lat_ms <"$TMP/$arm.p99") | $(awk -v a="$fe" -v b="$rps" 'BEGIN{ if (b>0 && a>0) printf "%.2f", a/b; else print "-" }') |"
  done
} | tee "$OUT"
echo "wrote $OUT"
