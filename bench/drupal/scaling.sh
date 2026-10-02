#!/usr/bin/env bash
# bench/drupal/scaling.sh — does ferro's classic pool lose throughput to
# threads, to the allocator, or to neither? The Drupal front page (classic
# mode: a fresh Vm per request) under wrk, served by
#   1 x 1   one process, one worker                (the per-worker reference)
#   1 x N   one process, N worker threads          (`--workers N`)
#   N x 1   N processes, one worker each, one port (`--workers 1 --reuse-port`)
# for each binary in BINS (default: the shipped mimalloc build and a
# `--features system-alloc` build). Per-worker efficiency = req/s / (N x the
# same binary's 1x1 req/s). Run from the host:
#   N=8 bench/drupal/scaling.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"; ROOT="$(cd "$REPO/.." && pwd)"
IMAGE="${RUSTYPHP_IMAGE:-rustyphp-dev:8.5.7}"
N="${N:-8}"; THREADS="${THREADS:-2}"; DURATION="${DURATION:-15s}"; WARMUP="${WARMUP:-5s}"; R="${R:-3}"
BINS="${BINS:-mimalloc=/target/release/ferro system=/target/sysalloc/release/ferro}"
NET=rustyphp-scaling
OUT="${OUT:-$REPO/bench/results/$(date -u +%Y-%m-%d)-drupal-scaling-n$N.md}"
MOUNTS=(-v "$ROOT":/work:ro -v rustyphp-scratch:/scratch -v rustyphp-target:/target:ro)
cleanup() { docker rm -f ferro-scal >/dev/null 2>&1; docker network rm "$NET" >/dev/null 2>&1; }
trap cleanup EXIT; cleanup
docker network create "$NET" >/dev/null
docker run --rm "${MOUNTS[@]}" "$IMAGE" bash -c '
  rm -rf /scratch/drupal-b-sc; cp -a /scratch/drupal-base /scratch/drupal-b-sc; chmod -R a+rwX /scratch/drupal-b-sc'

# start ARM on BIN: "1x1", "1xN" or "Nx1"
start() {
  local bin=$1 procs=$2 threads=$3
  docker rm -f ferro-scal >/dev/null 2>&1
  docker run -d --name ferro-scal --network "$NET" "${MOUNTS[@]}" "$IMAGE" bash -c "
    cd /scratch/drupal-b-sc/web
    for i in \$(seq 1 $procs); do
      $bin -S 0.0.0.0:8080 -t /scratch/drupal-b-sc/web .ht.router.php --workers $threads --reuse-port 2>/dev/null &
    done
    wait" >/dev/null
  for i in $(seq 1 60); do
    docker run --rm --network "$NET" "$IMAGE" curl -s -o /dev/null -f http://ferro-scal:8080/ && break; sleep 1
  done
}
measure() {  # prints the median req/s of R runs, CONNS connections
  local conns=$1 v=()
  docker run --rm --network "$NET" "$IMAGE" wrk -t"$THREADS" -c"$conns" -d"$WARMUP" http://ferro-scal:8080/ >/dev/null 2>&1
  for i in $(seq 1 "$R"); do
    v+=("$(docker run --rm --network "$NET" "$IMAGE" wrk -t"$THREADS" -c"$conns" -d"$DURATION" http://ferro-scal:8080/ 2>&1 | awk '/Requests\/sec/{print $2}')")
  done
  printf '%s\n' "${v[@]}" | sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}'
}

rows=()
for spec in $BINS; do
  name=${spec%%=*}; bin=${spec#*=}
  start "$bin" 1 1;    one=$(measure 4)
  start "$bin" 1 "$N"; thr=$(measure $((N * 4)))
  start "$bin" "$N" 1; prc=$(measure $((N * 4)))
  eff() { awk -v r="$1" -v o="$one" -v n="$2" 'BEGIN{printf "%.0f %%", 100 * r / (n * o)}'; }
  echo "$name: 1x1 $one | 1x$N $thr ($(eff "$thr" "$N")) | ${N}x1 $prc ($(eff "$prc" "$N"))" >&2
  rows+=("| $name | $one | $thr | $(eff "$thr" "$N") | $prc | $(eff "$prc" "$N") |")
done
{
  echo "# Drupal classic-mode scaling: threads vs processes — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo
  echo "- ferro $(git -C "$REPO" rev-parse --short HEAD), classic mode (fresh Vm per request), front page, page_cache off"
  echo "- wrk: $THREADS threads, 4 connections per worker, $DURATION per run, $R runs (median), $WARMUP warm-up; $(docker run --rm "$IMAGE" nproc) CPUs in the VM, shared by wrk and the servers"
  echo "- efficiency = req/s / (workers x the same binary's 1x1 req/s)"
  echo
  echo "| allocator | 1x1 req/s | 1x$N (threads) | efficiency | ${N}x1 (processes, SO_REUSEPORT) | efficiency |"
  echo "|---|---:|---:|---:|---:|---:|"
  printf '%s\n' "${rows[@]}"
} | tee "$OUT"
echo "wrote $OUT"
