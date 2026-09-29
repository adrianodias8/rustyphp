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
# volume paths and to a frozen fail-set (wp109-harness/corpus-gate/) that is
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
[[ $rc == 0 ]] && echo "GATE: PASS" || echo "GATE: FAIL"
exit $rc
