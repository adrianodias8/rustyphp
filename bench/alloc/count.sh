#!/usr/bin/env bash
# bench/alloc/count.sh — allocations per loop iteration (PLAN.md §2.2).
#   docker/run.sh /work/php-rust/bench/alloc/count.sh
# Uses a SEPARATE phpr built with upstream's own `mem-census` feature
# (CARGO_TARGET_DIR=/target/census), whose global-allocator wrapper counts every
# alloc/free/realloc. No source is modified. Each variant runs at N1 and N2; the
# per-iteration figure is the difference quotient, so startup cancels exactly.
# The census binary is for COUNTING only — never for timing.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CENSUS="${CENSUS:-/target/census/release/ferro}"
PHPR="${PHPR:-/target/release/ferro}"
PHP="${PHP_ORACLE:-$(command -v php)}"
N1="${N1:-200000}"; N2="${N2:-400000}"
T="$(mktemp -d)"
if [[ ! -x "$CENSUS" ]]; then
  (cd "$HERE/../../php-rust" && CARGO_TARGET_DIR=/target/census cargo build --release --locked -p php-cli --features mem-census 2>&1 | tail -1)
fi
get() { # file key
  grep -a ' tag=exit ' "$1" | tr ' ' '\n' | awk -F= -v k="$2" '$1==k{print $2}' | tail -1
}
printf 'variant\tallocs/iter\tfrees/iter\treallocs/iter\tzval_clones/iter (scalar+str+arr+obj)\tresult_matches_oracle\n'
for v in empty_global arith_global arith_fn float_fn array_read_fn array_write_global array_write_fn assoc_write_fn array_append_fn \
         foreach_fn call_fn builtin_fn prop_write_fn method_fn concat_fn; do
  for n in "$N1" "$N2"; do
    rm -f "$T/c.$n"
    PHPR_MEM_CENSUS="$T/c.$n" "$CENSUS" "$HERE/variants.php" "$v" "$n" >"$T/out.$n" 2>/dev/null
  done
  ok=no; [[ "$("$PHP" -n "$HERE/variants.php" "$v" "$N2")" == "$(cat "$T/out.$N2")" ]] && ok=yes
  python3 - "$v" "$N1" "$N2" "$ok" \
     "$(get "$T/c.$N1" s143.galloc_n)" "$(get "$T/c.$N2" s143.galloc_n)" \
     "$(get "$T/c.$N1" s143.gfree_n)"  "$(get "$T/c.$N2" s143.gfree_n)" \
     "$(get "$T/c.$N1" s144.grealloc_n)" "$(get "$T/c.$N2" s144.grealloc_n)" \
     "$(grep -a ' tag=exit ' "$T/c.$N1" | tr ' ' '\n' | awk -F= '$1 ~ /^s145\.clone_(scalar|str|arr|obj)_n$/{s+=$2} END{print s+0}')" \
     "$(grep -a ' tag=exit ' "$T/c.$N2" | tr ' ' '\n' | awk -F= '$1 ~ /^s145\.clone_(scalar|str|arr|obj)_n$/{s+=$2} END{print s+0}')" <<'PY'
import sys
v, n1, n2, ok = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
a1, a2, f1, f2, r1, r2, c1, c2 = map(int, sys.argv[5:13])
d = n2 - n1
print(f"{v}\t{(a2-a1)/d:.3f}\t{(f2-f1)/d:.3f}\t{(r2-r1)/d:.3f}\t{(c2-c1)/d:.3f}\t{ok}")
PY
done
rm -rf "$T"
