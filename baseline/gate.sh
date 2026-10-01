#!/usr/bin/env bash
# baseline/gate.sh — regression gate (PLAN.md §1.2): fails if any test listed in
# baseline/zend-tests.pass.txt no longer passes.
#
#   docker/run.sh ../baseline/gate.sh
#
# pass→fail is always fatal. pass→skip is fatal too (the test stopped running,
# which hides a regression just as well) unless ALLOW_PASS_TO_SKIP=1.
# New passes are reported so the baseline can be advanced deliberately with
# run-baseline.sh — the gate never rewrites the baseline itself.
#
# Why not upstream's scripts/corpus-gate.sh: it is hard-wired to upstream's
# volume paths and to a frozen fail-set (its wp109-harness/corpus-gate/) that was
# not published in the repository.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib.sh"
RAW="${RAW:-/scratch/gate-raw}"
BASE_PASS="$HERE/zend-tests.pass.txt"
[[ -s "$BASE_PASS" ]] || { echo "missing $BASE_PASS"; exit 2; }
[[ -x "$RUNNER" ]] || { echo "phpt-runner not found at $RUNNER"; exit 2; }

stage_corpus
rm -rf "$RAW"; mkdir -p "$RAW"
run_suite zend-tests "$RAW" tests Zend/tests
check_suite_consistent "$RAW" zend-tests

LC_ALL=C sort -u "$BASE_PASS" >"$RAW/base.pass"
LC_ALL=C comm -12 "$RAW/base.pass" "$RAW/zend-tests.fails" >"$RAW/pass-to-fail"
LC_ALL=C comm -12 "$RAW/base.pass" "$RAW/zend-tests.skips" >"$RAW/pass-to-skip"
LC_ALL=C comm -23 "$RAW/base.pass" "$RAW/zend-tests.all"   >"$RAW/pass-to-missing"
LC_ALL=C comm -13 "$RAW/base.pass" "$RAW/zend-tests.pass"  >"$RAW/new-pass"

n_fail=$(wc -l <"$RAW/pass-to-fail" | tr -d ' ')
n_skip=$(wc -l <"$RAW/pass-to-skip" | tr -d ' ')
n_miss=$(wc -l <"$RAW/pass-to-missing" | tr -d ' ')
n_new=$(wc -l <"$RAW/new-pass" | tr -d ' ')

echo "baseline pass: $(wc -l <"$RAW/base.pass" | tr -d ' ')   now pass: $(wc -l <"$RAW/zend-tests.pass" | tr -d ' ')"
echo "pass→fail: $n_fail   pass→skip: $n_skip   pass→missing: $n_miss   new passes: $n_new"
rc=0
if [[ "$n_fail" != 0 ]]; then echo "--- pass→fail"; sed 's/^/  /' "$RAW/pass-to-fail"; rc=1; fi
if [[ "$n_miss" != 0 ]]; then echo "--- pass→missing (test file no longer in corpus)"; sed 's/^/  /' "$RAW/pass-to-missing"; rc=1; fi
if [[ "$n_skip" != 0 ]]; then
  echo "--- pass→skip"; sed 's/^/  /' "$RAW/pass-to-skip"
  [[ "${ALLOW_PASS_TO_SKIP:-0}" == 1 ]] || rc=1
fi
if [[ "$n_new" != 0 ]]; then echo "--- new passes (advance the baseline with run-baseline.sh)"; sed 's/^/  /' "$RAW/new-pass"; fi
# The fork's own regression tests: every .phpt under baseline/repro/ (each one
# verified to PASS on the oracle with php-src's run-tests.php before it was
# committed) must pass; a failure here is a regression of a fixed bug.
if compgen -G "$HERE/repro/*.phpt" >/dev/null; then
  rm -rf "$RAW/repro"; mkdir -p "$RAW/repro"; cp "$HERE"/repro/*.phpt "$RAW/repro/"
  "$RUNNER" --isolate --list-fails "$RAW/repro" >"$RAW/repro.log" 2>&1 || true
  rp=$(awk '/^pass: /{print $2}' "$RAW/repro.log" | tail -1); rf=$(awk '/^fail: /{print $2}' "$RAW/repro.log" | tail -1); rs=$(awk '/^skip: /{print $2}' "$RAW/repro.log" | tail -1)
  echo "repro tests: pass=$rp fail=$rf skip=$rs"
  if [[ "${rf:-1}" != 0 || "${rs:-1}" != 0 ]]; then
    tr -d '\0' <"$RAW/repro.log" | grep -aE '^--- .*\.phpt ---$' | sed 's/^/  /'; rc=1
  fi
fi
# Known divergences (KNOWN_DIVERGENCES.md): every .phpt under baseline/divergences/
# PASSES on the oracle and is EXPECTED to fail (or skip) here. One that passes
# has been fixed: move it to baseline/repro/ and drop its row. Informational —
# never fatal.
if compgen -G "$HERE/divergences/*.phpt" >/dev/null; then
  rm -rf "$RAW/div"; mkdir -p "$RAW/div"; cp "$HERE"/divergences/*.phpt "$RAW/div/"
  "$RUNNER" --isolate --list-fails --list-skips "$RAW/div" >"$RAW/div.log" 2>&1 || true
  dp=$(awk '/^pass: /{print $2}' "$RAW/div.log" | tail -1); df=$(awk '/^fail: /{print $2}' "$RAW/div.log" | tail -1); ds=$(awk '/^skip: /{print $2}' "$RAW/div.log" | tail -1)
  echo "known divergences: still failing=$df skipped=$ds now PASSING=$dp"
  if [[ "${dp:-0}" != 0 ]]; then
    echo "--- fixed (promote to baseline/repro/, drop the KNOWN_DIVERGENCES.md row):"
    # passing = every test minus the failing and skipped ones
    ls "$RAW"/div/*.phpt | LC_ALL=C sort >"$RAW/div.all"
    tr -d '\0' <"$RAW/div.log" | awk '/^failures: [0-9]+$/{f=1;next} f && /^--- .*\.phpt ---$/{sub(/^--- /,"");sub(/ ---$/,"");print}' | LC_ALL=C sort -u >"$RAW/div.fails"
    tr -d '\0' <"$RAW/div.log" | awk '/^=== skips: [0-9]+ ===$/{s=1;next} /^=== phpt-runner ===$/{s=0} s && /\.phpt\t/{print}' | cut -f1 | LC_ALL=C sort -u >"$RAW/div.skips"
    LC_ALL=C comm -23 "$RAW/div.all" <(LC_ALL=C sort -u "$RAW/div.fails" "$RAW/div.skips") | sed 's/^/  /'
  fi
fi
[[ $rc == 0 ]] && echo "GATE: PASS" || echo "GATE: FAIL"
exit $rc
