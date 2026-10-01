#!/usr/bin/env bash
# bench/compile-cost.sh — PLAN.md §2.2: "time phpr on a 5,000-line file that
# does nothing vs the oracle with and without opcache. This quantifies what a
# bytecode cache would buy."
#
#   docker/run.sh ../bench/compile-cost.sh
#
# Two scripts per engine: empty.php (the startup floor) and big.php (5,000
# lines of declarations, nothing executed). compile cost = big − empty.
# Also big20k.php (4×) to check the cost scales with source size.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PHPR="${PHPR:-/target/release/ferro}"
PHP="${PHP_ORACLE:-$(command -v php)}"
R="${R:-30}"
D="${SCRATCH:-/scratch}/compile-cost"
OPC="$D/opcache"
rm -rf "$D"; mkdir -p "$D" "$OPC"
printf '<?php\n' >"$D/empty.php"
"$PHP" "$HERE/gen/gen-compile.php" "$D/big.php" 5000
"$PHP" "$HERE/gen/gen-compile.php" "$D/big20k.php" 20000
for f in empty big big20k; do
  "$PHP" -n -l "$D/$f.php" >/dev/null
  echo "## $f.php  ($(wc -l <"$D/$f.php") lines, $(stat -c %s "$D/$f.php") bytes)"
  python3 "$HERE/lib/timeit.py" "$R" \
    "phpr=$PHPR" "$D/$f.php" --- \
    "phpr-nounitcache=env" PHPR_UNIT_CACHE=0 "$PHPR" "$D/$f.php" --- \
    "php-noopc=$PHP" -n -d opcache.enable_cli=0 "$D/$f.php" --- \
    "php-opc=$PHP" -d opcache.enable=1 -d opcache.enable_cli=1 "$D/$f.php" --- \
    "php-opc-warm=$PHP" -d opcache.enable=1 -d opcache.enable_cli=1 -d "opcache.file_cache=$OPC" "$D/$f.php"
  echo
done
echo "opcache file cache entries: $(find "$OPC" -name '*.bin' | wc -l)"
