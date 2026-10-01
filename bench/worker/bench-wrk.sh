#!/usr/bin/env bash
# bench/worker/bench-wrk.sh — HTTP throughput under wrk, three servers, two
# apps (the Symfony HttpKernel app of bench/symfony-boot.php and a hello
# world), run from the HOST with the docker CLI (nothing installed on it):
#
#   phpr-worker    ferro -S --worker (this fork's worker mode), N workers
#   fpm            nginx -> php-fpm 8.5.7, opcache on, static pool of N
#   frankenphp     FrankenPHP worker mode (dunglas/frankenphp), N workers
#
# Every server runs in its own container on one docker network; wrk runs in
# a fourth container (the dev image). All three mount the same vendor/
# (rustyphp-scratch:/scratch/symfony-app) and the same front controllers
# (bench/worker/). Each arm is warmed for WARMUP, measured for DURATION,
# and its five responses are checked against php-fpm's (the oracle) first.
#
#   bench/worker/bench-wrk.sh                      # from the host
#   WORKERS=4 CONNS=32 THREADS=2 DURATION=15s R=3 bench/worker/bench-wrk.sh
#
# Output: bench/results/<date>-wrk.md (medians of R runs per arm and app).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
ROOT="$(cd "$REPO/.." && pwd)"
IMAGE="${RUSTYPHP_IMAGE:-rustyphp-dev:8.5.7}"
WORKERS="${WORKERS:-4}"; CONNS="${CONNS:-32}"; THREADS="${THREADS:-2}"
DURATION="${DURATION:-15s}"; WARMUP="${WARMUP:-3s}"; R="${R:-3}"
NET=rustyphp-bench
OUT="${OUT:-$REPO/bench/results/$(date -u +%Y-%m-%d)-wrk.md}"

cleanup() {
  for c in phpr-worker fpm nginx frankenphp; do docker rm -f "$c" >/dev/null 2>&1; done
  docker network rm "$NET" >/dev/null 2>&1
  [[ -n "${TMP:-}" ]] && rm -rf "$TMP"
}
trap cleanup EXIT
cleanup
TMP="$(mktemp -d)"
docker network create "$NET" >/dev/null

MOUNTS=(-v "$ROOT":/work:ro -v rustyphp-scratch:/scratch)

# ---- servers ----
docker run -d --name phpr-worker --network "$NET" "${MOUNTS[@]}" -v rustyphp-target:/target:ro \
  -e SYMFONY_DIR=/scratch/symfony-app "$IMAGE" bash -c "
    /target/release/ferro -S 0.0.0.0:8080 --worker /work/php-rust/bench/worker/symfony-worker.php --workers $WORKERS &
    /target/release/ferro -S 0.0.0.0:8081 --worker /work/php-rust/bench/worker/hello-worker.php --workers $WORKERS &
    wait" >/dev/null

sed "s/^pm.max_children = .*/pm.max_children = $WORKERS/" "$HERE/fpm/zz-bench.conf" >"$TMP/zz-bench.conf"
docker run -d --name fpm --network "$NET" "${MOUNTS[@]}" \
  -v "$TMP/zz-bench.conf":/usr/local/etc/php-fpm.d/zz-bench.conf:ro \
  -v "$HERE/fpm/opcache.ini":/usr/local/etc/php/conf.d/opcache.ini:ro \
  php:8.5.7-fpm >/dev/null
docker run -d --name nginx --network "$NET" "${MOUNTS[@]}" \
  -v "$HERE/nginx/default.conf":/etc/nginx/conf.d/default.conf:ro nginx:alpine >/dev/null

sed "s/^\(\t*num \).*/\1$WORKERS/" "$HERE/frankenphp/Caddyfile" >"$TMP/Caddyfile"
docker run -d --name frankenphp --network "$NET" "${MOUNTS[@]}" \
  -v "$TMP/Caddyfile":/etc/frankenphp/Caddyfile:ro dunglas/frankenphp:latest >/dev/null
sleep 3

wrk_in() { docker run --rm --network "$NET" -v "$HERE/paths.lua":/paths.lua:ro "$IMAGE" wrk "$@"; }
curl_in() { docker run --rm --network "$NET" "$IMAGE" curl -s "$@"; }
versions() {
  echo "- phpr: \`$(docker exec phpr-worker sh -c 'sha256sum /target/release/ferro | cut -c1-16')\` ($(git -C "$REPO" rev-parse --short HEAD))"
  echo "- php-fpm: \`$(docker exec fpm php-fpm -v 2>&1 | head -1)\`, opcache on (validate_timestamps=0, jit off), nginx \`$(docker exec nginx nginx -v 2>&1 | sed 's/.*nginx\///')\`"
  echo "- FrankenPHP: \`$(docker exec frankenphp frankenphp version 2>/dev/null | head -1)\`"
  echo "- wrk \`$(docker run --rm "$IMAGE" wrk --version 2>&1 | head -1 | cut -d' ' -f2)\`, $THREADS threads, $CONNS connections, $DURATION per run, $R runs (medians), $WARMUP warm-up; $WORKERS workers per server"
  echo "- host: $(docker run --rm "$IMAGE" nproc) CPUs visible in the VM; wrk and the servers share them"
}

# macOS ships bash 3.2: no associative arrays — small functions and files instead.
host_of() { case "$1" in fpm) echo nginx ;; *) echo "$1" ;; esac; }
res_set() { echo "$3" >"$TMP/res.$1.$2"; }     # key arm.app -> value
res_get() { cat "$TMP/res.$1.$2" 2>/dev/null; }
PATHS=("/" "/user/42/alice" "/api/widgets?page=2&sort=name" "/user/7" "/api/orders?x[]=1&x[]=2")

# ---- readiness + response check against php-fpm ----
for arm in fpm phpr-worker frankenphp; do
  for i in $(seq 1 30); do curl_in -o /dev/null "http://$(host_of "$arm"):8080/" && break; sleep 1; done
done
check_line=""
for arm in phpr-worker frankenphp; do
  same=0; diffp=""
  for p in "${PATHS[@]}"; do
    a="$(curl_in "http://nginx:8080$p")"; b="$(curl_in "http://$(host_of "$arm"):8080$p")"
    if [[ "$a" == "$b" ]]; then same=$((same+1)); else diffp="$diffp $p"; fi
  done
  h1="$(curl_in "http://nginx:8081/")"; h2="$(curl_in "http://$(host_of "$arm"):8081/")"
  [[ "$h1" == "$h2" ]] && same=$((same+1)) || diffp="$diffp hello"
  check_line="$check_line$arm: $same/6 responses identical to php-fpm${diffp:+ (differs:$diffp)}; "
done

# ---- measure ----
median_of() { sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}'; }
# wrk latencies ("1.23 ms" / "850.00 us" / "1.20 s" after the sed below) -> median in ms
lat_median() { awk '{v=$1; u=$2; if (u=="us") v/=1000; else if (u=="s") v*=1000; print v}' | sort -n | awk '{a[NR]=$1} END{printf "%.2fms", a[int((NR+1)/2)]}'; }
run_arm() { # $1 arm  $2 app(symfony|hello)
  local arm="$1" app="$2" port url script
  if [[ "$app" == symfony ]]; then port=8080; script="-s /paths.lua"; else port=8081; script=""; fi
  url="http://$(host_of "$arm"):$port/"
  wrk_in -t"$THREADS" -c"$CONNS" -d"$WARMUP" $script "$url" >/dev/null 2>&1
  : >"$TMP/rps"; : >"$TMP/p50"; : >"$TMP/p99"
  for i in $(seq 1 "$R"); do
    local o; o="$(wrk_in -t"$THREADS" -c"$CONNS" -d"$DURATION" --latency $script "$url" 2>&1)"
    local rps p50 p99 errs
    rps="$(awk '/Requests\/sec/{print $2}' <<<"$o")"; p50="$(awk '$1=="50%"{print $2}' <<<"$o")"; p99="$(awk '$1=="99%"{print $2}' <<<"$o")"
    echo "$rps" >>"$TMP/rps"; echo "$p50" | sed -E 's/([0-9])(us|ms|s)$/\1 \2/' >>"$TMP/p50"; echo "$p99" | sed -E 's/([0-9])(us|ms|s)$/\1 \2/' >>"$TMP/p99"
    errs="$(grep -E 'Socket errors|Non-2xx' <<<"$o" | tr '\n' ' ')"
    echo "  $arm $app run $i: $rps req/s  p50 $p50  p99 $p99 $errs" >&2
  done
  res_set "$arm" "$app.rps" "$(median_of <"$TMP/rps")"
  res_set "$arm" "$app.p50" "$(lat_median <"$TMP/p50")"
  res_set "$arm" "$app.p99" "$(lat_median <"$TMP/p99")"
}
for app in hello symfony; do
  for arm in fpm frankenphp phpr-worker; do run_arm "$arm" "$app"; done
done

# ---- report ----
{
  echo "# Worker-mode throughput under wrk — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo
  versions
  echo "- response check: $check_line"
  echo
  echo "Requests per second (median of $R), latency p50 / p99 (wrk --latency). Ratio = phpr ÷ arm."
  echo
  for app in hello symfony; do
    echo "## $app"; echo
    echo "| server | req/s | p50 | p99 | phpr-worker ÷ this |"; echo "|---|---:|---:|---:|---:|"
    for arm in fpm frankenphp phpr-worker; do
      ratio="$(awk -v a="$(res_get phpr-worker "$app.rps")" -v b="$(res_get "$arm" "$app.rps")" 'BEGIN{ if (b>0) printf "%.2f", a/b; else print "-" }')"
      echo "| $arm | $(res_get "$arm" "$app.rps") | $(res_get "$arm" "$app.p50") | $(res_get "$arm" "$app.p99") | $ratio |"
    done
    echo
  done
} | tee "$OUT"
echo "wrote $OUT"
