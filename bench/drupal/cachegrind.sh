#!/usr/bin/env bash
# bench/drupal/cachegrind.sh — instruction and cache-miss counts of warm Drupal
# front-page requests under cachegrind (the Docker VM has no PMU, so this is the
# hardware-counter substitute; HWCOUNTERS_DRUPAL.md).
#   ENGINE=ferro  ferro -S, classic (a fresh Vm per request)        BIN=/target/cg/release/ferro
#   ENGINE=php    php -S + opcache (enable_cli=1, file cache warm)  BIN=/scratch/php-dbg/bin/php
# On a fresh copy of the base: WARM native requests (Drupal caches, opcache
# file cache), then two cachegrind runs of the server, the first with WARM_CG
# requests, the second with WARM_CG + M. Per request = (second - first) / M,
# which removes start-up, warm-up and shutdown exactly.
# Cache model = the host (Apple M2 Max P-core): D1 128K 8-way, I1 192K 6-way,
# LL (its L2) 16M 16-way, 128-byte lines.
#   docker/run.sh bash /work/php-rust/bench/drupal/cachegrind.sh
#   TOOL=callgrind TAG=ferro-cl M=10 ...   (per-instruction counts, see cg-handlers.py)
set -euo pipefail
ENGINE=${ENGINE:-ferro}; M=${M:-20}; WARM=${WARM:-5}; WARM_CG=${WARM_CG:-3}; PORT=${PORT:-8250}
OUT=/scratch/cg; mkdir -p $OUT
case $ENGINE in
  ferro) BIN=${BIN:-/target/cg/release/ferro}; ARGS=() ;;
  php)   BIN=${BIN:-/scratch/php-dbg/bin/php}
         ARGS=(-n -d opcache.enable=1 -d opcache.enable_cli=1 -d opcache.jit=off
               -d opcache.validate_timestamps=0 -d opcache.memory_consumption=128
               -d opcache.max_accelerated_files=10000 -d opcache.file_cache=$OUT/fc-$ENGINE)
         # the official (stripped) build has gd and sodium as shared extensions
         [[ $BIN == /usr/local/bin/php ]] && ARGS+=(-d extension=gd -d extension=sodium)
         rm -rf $OUT/fc-$ENGINE; mkdir -p $OUT/fc-$ENGINE ;;
esac
TAG=${TAG:-$ENGINE}
SITE=/scratch/drupal-cg-$TAG
rm -rf $SITE; cp -a /scratch/drupal-base $SITE; cd $SITE/web
hit() { for i in $(seq 1 "$1"); do curl -s -o /dev/null -w '%{http_code} ' --max-time 600 http://127.0.0.1:$PORT/; done; echo; }
up() { for i in $(seq 1 600); do curl -s -o /dev/null --max-time 600 http://127.0.0.1:$PORT/ && return; sleep 1; done; echo "server never came up" >&2; exit 1; }
"$BIN" "${ARGS[@]}" -S 127.0.0.1:$PORT .ht.router.php >/dev/null 2>&1 & P=$!
up; hit "$WARM"; kill $P; wait $P 2>/dev/null || true
# TOOL=callgrind: instruction counts per instruction address plus call edges
# (no cache model), for the per-handler attribution of cg-handlers.py
if [[ ${TOOL:-cachegrind} == callgrind ]]; then
  VG=(--tool=callgrind --dump-instr=yes --collect-jumps=no)
else
  VG=(--tool=cachegrind --cache-sim=yes --branch-sim=yes
      --I1=196608,6,128 --D1=131072,8,128 --LL=16777216,16,128)
fi
for n in 0 "$M"; do
  # up() is itself one request, so the run serves 1 + WARM_CG + n
  valgrind "${VG[@]}" \
    --${TOOL:-cachegrind}-out-file=$OUT/$TAG-$n.out --log-file=$OUT/$TAG-$n.log \
    "$BIN" "${ARGS[@]}" -S 127.0.0.1:$PORT .ht.router.php >/dev/null 2>&1 & P=$!
  up; hit "$WARM_CG"; [[ $n -gt 0 ]] && hit "$n"
  kill -TERM $P; wait $P 2>/dev/null || true
done
python3 - "$OUT/$TAG-0.out" "$OUT/$TAG-$M.out" "$M" "$TAG" <<'PY'
import sys
def summary(p):
    ev = summ = None
    for l in open(p):
        if l.startswith("events:"): ev = l.split()[1:]
        if l.startswith("summary:"): summ = [int(x) for x in l.split()[1:]]
    return dict(zip(ev, summ))
a, b, m = summary(sys.argv[1]), summary(sys.argv[2]), int(sys.argv[3])
print(sys.argv[4], " ".join(f"{k}={(b[k] - a[k]) / m:.0f}" for k in b))
PY
