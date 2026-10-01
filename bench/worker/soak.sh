#!/usr/bin/env bash
# bench/worker/soak.sh — long-run leak test of worker mode: drive one app
# under wrk for TOTAL seconds and sample the server's RSS every STEP seconds.
# A flat RSS after warm-up means requests leak nothing into the worker; a
# slope is a leak, and its size per request is printed.
#
#   docker/run.sh /work/php-rust/bench/worker/soak.sh                 # hello, 120 s
#   APP=symfony TOTAL=600 MAXREQ=10000 docker/run.sh /work/php-rust/bench/worker/soak.sh
#
# APP=hello|symfony  WORKERS (2)  CONNS (16)  TOTAL (120)  STEP (10)
# MAXREQ: passed as --max-requests (0 = never recycle).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FERRO="${FERRO:-/target/release/ferro}"
APP="${APP:-hello}"; WORKERS="${WORKERS:-2}"; CONNS="${CONNS:-16}"
TOTAL="${TOTAL:-120}"; STEP="${STEP:-10}"; MAXREQ="${MAXREQ:-0}"
S="${SCRATCH:-/scratch}/soak"; rm -rf "$S"; mkdir -p "$S"
export SYMFONY_DIR="${SYMFONY_DIR:-/scratch/symfony-app}"

"$FERRO" -S 127.0.0.1:8110 --worker "$HERE/$APP-worker.php" --workers "$WORKERS" \
  --max-requests "$MAXREQ" >"$S/server.log" 2>&1 & PID=$!
sleep 2
curl -sf -o /dev/null http://127.0.0.1:8110/ || { echo "server did not answer"; cat "$S/server.log"; kill $PID; exit 1; }

rss() { awk '/^VmRSS/ {print $2}' "/proc/$PID/status"; }
wrk -t2 -c"$CONNS" -d"${TOTAL}s" --latency http://127.0.0.1:8110/ >"$S/wrk.txt" 2>&1 & WRK=$!
echo "t_s rss_kb" >"$S/rss.txt"
t=0
while kill -0 $WRK 2>/dev/null; do
  echo "$t $(rss)" >>"$S/rss.txt"; sleep "$STEP"; t=$((t + STEP))
done
wait $WRK
echo "$t $(rss)" >>"$S/rss.txt"
kill $PID; wait $PID 2>/dev/null

REQS=$(awk '/requests in/ {print $1}' "$S/wrk.txt")
RECYCLES=$(grep -c "script returned; restarting" "$S/server.log")
FATALS=$(grep -cE "fatal|panicked" "$S/server.log")
echo "app=$APP workers=$WORKERS conns=$CONNS total=${TOTAL}s max_requests=$MAXREQ"
grep -E "Requests/sec|Non-2xx|Socket errors" "$S/wrk.txt"
echo "requests=$REQS recycles=$RECYCLES fatals/panics=$FATALS"
cat "$S/rss.txt"
# Slope from the sample after warm-up (the 2nd) to the last, per request.
awk -v reqs="$REQS" -v total="$TOTAL" 'NR==3 {t0=$1; r0=$2} {t1=$1; r1=$2}
  END { if (t1 > t0 && reqs > 0) { per_s = reqs / total; n = per_s * (t1 - t0);
        printf "rss growth after warm-up: %+d KB over ~%d requests = %.3f bytes/request\n",
               r1 - r0, n, (r1 - r0) * 1024 / n } }' "$S/rss.txt"
