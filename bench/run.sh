#!/usr/bin/env bash
# bench/run.sh — PLAN.md §2.1. Runs every benchmark under phpr and the oracle,
# R runs each (default 5), and writes bench/results/<date>.md with the median
# wall time, user CPU and peak RSS, plus each benchmark's own per-section times.
#
# Meant to run INSIDE the dev container:   docker/run.sh ../bench/run.sh
#
# Engines
#   phpr          $PHPR (default /target/release/ferro)
#   php-noopc     php -n -d opcache.enable_cli=0           (PLAN: "php -n")
#   php-opc       php -d opcache.enable_cli=1              (PLAN: "php with opcache")
#   php-opc-warm  php-opc + opcache.file_cache, primed     (extra: what a resident
#                 FPM/worker with a warm opcache pays; a one-shot CLI run with
#                 enable_cli=1 still compiles every file, it only adds the optimizer)
#
# Env: R (runs, default 5) · ONLY="arrays oop" (subset) · OUT (results file)
#      HOST_NOTE (free text: the machine under the container, e.g. "Apple M2 Max, OrbStack")
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
PHP_SRC="${PHP_SRC:-$REPO/../php-src}"
PHPR="${PHPR:-/target/release/ferro}"
PHP="${PHP_ORACLE:-$(command -v php)}"
R="${R:-5}"
DATE="${DATE:-$(date -u +%Y-%m-%d)}"
SCRATCH="${SCRATCH:-/scratch}"
RAW="$SCRATCH/bench-raw/$DATE"
OUT="${OUT:-$HERE/results/$DATE.md}"
OPC_CACHE="$SCRATCH/opcache-file-cache"
ALL="zend_bench zend_micro_bench arrays strings oop autoload symfony-boot"
ONLY="${ONLY:-$ALL}"

[[ -x "$PHPR" ]] || { echo "phpr not found at $PHPR (cargo build --release -p php-cli)"; exit 1; }
[[ -f "$PHP_SRC/Zend/bench.php" ]] || { echo "php-src not found at $PHP_SRC"; exit 1; }

# ---- stage inputs on the container-local volume (keeps bind-mount I/O out
# ---- of the measurement) -------------------------------------------------
W="$SCRATCH/bench-work"
rm -rf "$W" "$RAW"; mkdir -p "$W/lib" "$RAW" "$OPC_CACHE"
cp "$HERE"/*.php "$W/"; cp "$HERE"/lib/*.php "$W/lib/"
cp "$PHP_SRC/Zend/bench.php" "$W/zend_bench.php"
# phpr rejects ONE statement of micro_bench.php at parse time —
# `$x = isset(Foo::$a);` ("unsupported construct (assignment target)") — and
# then runs nothing. That statement is neutralised in the copy BOTH engines run,
# so the other 36 rows are comparable; the `isset(Foo::$x)` row is reported as
# not measured. The unpatched file's failure is recorded by the run itself.
cp "$PHP_SRC/Zend/micro_bench.php" "$W/zend_micro_bench_unpatched.php"
sed 's|^\( *\)\$x = isset(Foo::\$a);|\1$x = 0; // patched out for phpr: isset(Class::$static)|' \
    "$PHP_SRC/Zend/micro_bench.php" >"$W/zend_micro_bench.php"
if cmp -s "$W/zend_micro_bench.php" "$W/zend_micro_bench_unpatched.php"; then
  echo "micro_bench.php patch did not apply"; exit 1
fi
( "$PHPR" "$W/zend_micro_bench_unpatched.php" 2>&1 | head -1 ) >"$RAW/zend_micro_bench.unpatched.phpr.txt" || true

if [[ " $ONLY " == *" autoload "* && ! -f "$SCRATCH/autoload/vendor/autoload.php" ]]; then
  rm -rf "$SCRATCH/autoload"
  "$PHP" "$HERE/gen/gen-autoload.php" "$SCRATCH/autoload" 2000
  (cd "$SCRATCH/autoload" && composer dump-autoload --no-interaction --quiet)
fi
if [[ " $ONLY " == *" symfony-boot "* && ! -f "$SCRATCH/symfony-app/vendor/autoload.php" ]]; then
  rm -rf "$SCRATCH/symfony-app"; mkdir -p "$SCRATCH/symfony-app"
  cp "$HERE/symfony-app/composer.json" "$SCRATCH/symfony-app/"
  [[ -f "$HERE/symfony-app/composer.lock" ]] && cp "$HERE/symfony-app/composer.lock" "$SCRATCH/symfony-app/"
  (cd "$SCRATCH/symfony-app" && composer install --no-interaction --no-progress --prefer-dist --quiet)
  cp "$SCRATCH/symfony-app/composer.lock" "$HERE/symfony-app/composer.lock"
fi
export AUTOLOAD_DIR="$SCRATCH/autoload" SYMFONY_DIR="$SCRATCH/symfony-app"

engine_cmd() { # $1 = engine name → prints the argv prefix, one arg per line
  case "$1" in
    # memory_limit=-1 everywhere: the oracle image ships no php.ini, so its
    # default 128M limit would abort the larger benchmarks; phpr enforces none.
    phpr)         printf '%s\n' "$PHPR" -d memory_limit=-1 ;;
    php-noopc)    printf '%s\n' "$PHP" -n -d memory_limit=-1 -d opcache.enable_cli=0 ;;
    php-opc)      printf '%s\n' "$PHP" -d memory_limit=-1 -d opcache.enable=1 -d opcache.enable_cli=1 ;;
    php-opc-warm) printf '%s\n' "$PHP" -d memory_limit=-1 -d opcache.enable=1 -d opcache.enable_cli=1 \
                    -d "opcache.file_cache=$OPC_CACHE" -d opcache.validate_timestamps=1 ;;
  esac
}
ENGINES="phpr php-noopc php-opc php-opc-warm"

run_one() { # $1 bench  $2 engine  $3 run-index
  local b="$1" e="$2" i="$3" script="$W/$1.php"
  local -a cmd; mapfile -t cmd < <(engine_cmd "$e")
  local base="$RAW/$b.$e.$i"
  local rc=0
  /usr/bin/time -v -o "$base.time" "${cmd[@]}" "$script" >"$base.out" 2>"$base.err" || rc=$?
  echo "$rc" >"$base.rc"
}

{
  echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "phpr_sha=$(git -C "$REPO" rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "phpr_dirty=$(git -C "$REPO" status --porcelain -- php-rust/crates php-rust/Cargo.toml php-rust/Cargo.lock 2>/dev/null | wc -l | tr -d ' ')"
  echo "phpr_bin_sha256=$(sha256sum "$PHPR" | cut -c1-16)"
  echo "phpr_bin_bytes=$(stat -c %s "$PHPR")"
  echo "oracle_version=$("$PHP" -n -r 'echo PHP_VERSION;')"
  echo "oracle_opcache=$("$PHP" -d opcache.enable_cli=1 -r 'echo function_exists("opcache_get_status") ? "available" : "MISSING"; $s=@opcache_get_status(false); echo " enabled=", var_export($s["opcache_enabled"] ?? null, true), " jit=", var_export($s["jit"]["enabled"] ?? null, true);')"
  echo "php_src_tag=$(git -C "$PHP_SRC" describe --tags 2>/dev/null || echo unknown)"
  echo "rustc=$(rustc -V 2>/dev/null || echo unknown)"
  echo "kernel=$(uname -srm)"
  echo "cpus=$(nproc)"
  echo "host=${HOST_NOTE:-not recorded (set HOST_NOTE)}"
  echo "mem_kb=$(grep MemTotal /proc/meminfo | awk '{print $2}')"
  echo "runs=$R"
  echo "loadavg_start=$(cut -d' ' -f1-3 /proc/loadavg)"
} >"$RAW/meta.env"

for b in $ONLY; do
  # Prime the opcache file cache once so php-opc-warm measures a warm cache.
  mapfile -t warm < <(engine_cmd php-opc-warm)
  "${warm[@]}" "$W/$b.php" >/dev/null 2>&1 || true
  for i in $(seq 1 "$R"); do
    for e in $ENGINES; do          # interleaved: one of each engine per round
      run_one "$b" "$e" "$i"
      printf '%-18s %-13s run %d/%d  rc=%s  %ss\n' "$b" "$e" "$i" "$R" \
        "$(cat "$RAW/$b.$e.$i.rc")" \
        "$(awk -F': ' '/Elapsed \(wall clock\)/{print $2}' "$RAW/$b.$e.$i.time")"
    done
  done
done
echo "loadavg_end=$(cut -d' ' -f1-3 /proc/loadavg)" >>"$RAW/meta.env"

python3 "$HERE/lib/summarize.py" "$RAW" "$OUT" "$ONLY"
echo "wrote $OUT"
