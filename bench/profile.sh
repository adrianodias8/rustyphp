#!/usr/bin/env bash
# bench/profile.sh — PLAN.md §2.2. Profiles phpr with perf and writes
# flamegraphs + folded stacks to bench/profiles/, and the bucket attribution
# for PROFILE.md to stdout.
#
# Needs perf_event access:   PRIV=1 docker/run.sh ../bench/profile.sh
#
# The profiled binary is a SEPARATE build in /target/prof with
# CARGO_PROFILE_RELEASE_DEBUG=1 (PLAN: "[profile.release] debug = 1"), set
# through the environment so upstream's Cargo.toml — the pin recipe — is not
# edited. Same opt-level / fat LTO / 1 CGU as the measured binary.
#
# Env: ONLY="zend_bench arrays oop symfony-boot" · FREQ (Hz, default 2999)
#      REUSE=1 re-collapses existing perf.data instead of recording again
#      OUT=<dir> output directory (default bench/profiles — the session-1 set)
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
PHP_SRC="${PHP_SRC:-$REPO/../php-src}"
SCRATCH="${SCRATCH:-/scratch}"
PROF_TARGET="${PROF_TARGET:-/target/prof}"
PHPR_PROF="$PROF_TARGET/release/phpr"
FREQ="${FREQ:-2999}"
ONLY="${ONLY:-zend_bench arrays oop symfony-boot symfony-steady strings autoload}"
# OUT: where the folded stacks / flamegraphs / bucket tables go. The default is
# the committed session-1 set (unmodified engine); a profile of a modified
# engine goes in a subdirectory (session 4: bench/profiles/after-slice-5/).
OUT="${OUT:-$HERE/profiles}"
W="$SCRATCH/bench-work"
PD="$SCRATCH/perf-data"
mkdir -p "$OUT" "$PD" "$W/lib"

(cd "$REPO/php-rust" && CARGO_TARGET_DIR="$PROF_TARGET" CARGO_PROFILE_RELEASE_DEBUG=1 \
   CARGO_PROFILE_RELEASE_STRIP=false cargo build --release --locked -p php-cli 2>&1 | tail -1)

cp "$HERE"/*.php "$W/"; cp "$HERE"/lib/*.php "$W/lib/"
cp "$PHP_SRC/Zend/bench.php" "$W/zend_bench.php"
sed 's|^\( *\)\$x = isset(Foo::\$a);|\1$x = 0; // patched out for phpr: isset(Class::$static)|' \
    "$PHP_SRC/Zend/micro_bench.php" >"$W/zend_micro_bench.php"
export AUTOLOAD_DIR="$SCRATCH/autoload" SYMFONY_DIR="$SCRATCH/symfony-app"

echo 0 > /proc/sys/kernel/kptr_restrict 2>/dev/null || true
echo -1 > /proc/sys/kernel/perf_event_paranoid 2>/dev/null || true

for b in $ONLY; do
  echo "=== $b"
  script="$W/$b.php"; freq="$FREQ"
  case "$b" in
    # The benchmarked configuration (200 requests) runs for ~80 ms: sample it
    # faster. symfony-steady is the same script with 5,000 requests, so the
    # per-request path dominates instead of boot + autoload.
    symfony-boot)   export SYMFONY_REQUESTS=200;  freq=19999 ;;
    symfony-steady) export SYMFONY_REQUESTS=5000; script="$W/symfony-boot.php" ;;
    autoload)       freq=9999 ;;
  esac
  [[ "${REUSE:-0}" == 1 && -s "$PD/$b.perf.data" ]] || \
  perf record -o "$PD/$b.perf.data" -e cpu-clock -F "$freq" --call-graph dwarf,16384 \
      -- "$PHPR_PROF" -d memory_limit=-1 "$script" >/dev/null 2>"$PD/$b.record.err" || { cat "$PD/$b.record.err"; exit 1; }
  perf script -i "$PD/$b.perf.data" --inline 2>/dev/null \
      | inferno-collapse-perf --kernel 2>/dev/null | rustfilt >"$OUT/$b.folded"
  inferno-flamegraph --title "phpr — $b ($(git -C "$REPO" rev-parse --short HEAD), cpu-clock ${freq}Hz)" \
      --width 1800 <"$OUT/$b.folded" >"$OUT/$b.svg"
  python3 "$HERE/lib/buckets.py" "$OUT/$b.folded" --md >"$OUT/$b.buckets.md"
  gzip -9 -k -f "$OUT/$b.folded"        # the .gz is what gets committed
  head -16 "$OUT/$b.buckets.md"
done
