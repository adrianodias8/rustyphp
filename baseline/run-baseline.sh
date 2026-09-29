#!/usr/bin/env bash
# baseline/run-baseline.sh — PLAN.md §1.2. Runs the .phpt corpus through
# phpt-runner and writes the committed baseline files:
#   baseline/zend-tests.{pass,fails,skips}.txt   (php-src/tests + php-src/Zend/tests)
#   baseline/ext/<ext>.{pass,fails}.txt          (per extension)
#   baseline/ext.md                              (per-extension table)
#   baseline/zend-tests.md                       (summary + skip categories)
# Run inside the dev container:  docker/run.sh ../baseline/run-baseline.sh
# Env: SUITES="zend ext" (default both) · EXTS="pcre json ..." (override list)
#      REPORT_ONLY=1 rebuilds the .md/.txt files from the raw logs of the last run
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib.sh"
RAW="${RAW:-/scratch/baseline-raw}"
SUITES="${SUITES:-zend ext}"
# PLAN §1.2 list, plus ext/standard first.
EXTS="${EXTS:-standard pcre json mbstring hash ctype dom simplexml pdo pdo_sqlite sqlite3 date spl reflection session filter tokenizer}"

[[ -x "$RUNNER" ]] || { echo "phpt-runner not found at $RUNNER (cargo build --release -p phpt-runner)"; exit 1; }
stage_corpus
mkdir -p "$RAW" "$HERE/ext"

meta() {
  echo "- phpr git SHA: \`$(git -C "$HERE/.." rev-parse HEAD)\`"
  echo "- phpt-runner binary sha256: \`$(sha256sum "$RUNNER" | cut -c1-16)\`"
  echo "- corpus: php-src \`$(git -C "$PHP_SRC" describe --tags)\` (\`$(git -C "$PHP_SRC" rev-parse --short HEAD)\`)"
  echo "- runner flags: \`--isolate --list-fails --list-skips\`, \`PHPT_TIMEOUT_SECS=$PHPT_TIMEOUT_SECS\`, default engine mode (no \`PHPR_REG_LOWER\` set)"
  echo "- platform: \`$(uname -srm)\`, $(. /etc/os-release; echo "$PRETTY_NAME"), in Docker"
  echo "- measured: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
}

if [[ " $SUITES " == *" zend "* ]]; then
  echo ">>> zend-tests (tests + Zend/tests)"
  [[ "${REPORT_ONLY:-0}" == 1 ]] || run_suite zend-tests "$RAW" tests Zend/tests
  check_suite_consistent "$RAW" zend-tests
  cp "$RAW/zend-tests.pass"  "$HERE/zend-tests.pass.txt"
  cp "$RAW/zend-tests.fails" "$HERE/zend-tests.fails.txt"
  cp "$RAW/zend-tests.skips.tsv" "$HERE/zend-tests.skips.tsv"
  # Also the Zend/tests-only split, the figure upstream quotes (2655/4067).
  ( . "$RAW/zend-tests.summary"
    zp=$(grep -c '^Zend/tests/' "$HERE/zend-tests.pass.txt" || true)
    zf=$(grep -c '^Zend/tests/' "$HERE/zend-tests.fails.txt" || true)
    zs=$(cut -f1 "$HERE/zend-tests.skips.tsv" | grep -c '^Zend/tests/' || true)
    tp=$(grep -c '^tests/' "$HERE/zend-tests.pass.txt" || true)
    tf=$(grep -c '^tests/' "$HERE/zend-tests.fails.txt" || true)
    ts=$(cut -f1 "$HERE/zend-tests.skips.tsv" | grep -c '^tests/' || true)
    pct() { awk -v p="$1" -v f="$2" 'BEGIN{ if (p+f>0) printf "%.1f%%", 100*p/(p+f); else printf "n/a" }'; }
    {
      echo "# Baseline — php-src \`tests/\` + \`Zend/tests/\`"
      echo
      meta
      echo "- wall time: ${seconds}s"
      echo
      echo "| scope | total | pass | fail | skip | pass rate (of runnable) |"
      echo "|---|---:|---:|---:|---:|---:|"
      echo "| \`tests/\` + \`Zend/tests/\` | $total | $pass | $fail | $skip | $(pct "$pass" "$fail") |"
      echo "| \`Zend/tests/\` only | $((zp+zf+zs)) | $zp | $zf | $zs | $(pct "$zp" "$zf") |"
      echo "| \`tests/\` only | $((tp+tf+ts)) | $tp | $tf | $ts | $(pct "$tp" "$tf") |"
      echo
      echo "Upstream's claim for \`Zend/tests/\` (README/COVERAGE, pin S-175, macOS, oracle 8.5.7):"
      echo "5305 total · 2655 pass · 1412 fail · 1238 skip = 65.3% of runnable."
      echo
      echo "## Skips by category"
      echo
      echo '```'
      awk '/^skips by category:/{s=1;next} s && /^$/{exit} s{print}' "$RAW/zend-tests.log"
      echo '```'
      echo
      echo "## Failure kinds"
      echo
      echo "| kind | count |"
      echo "|---|---:|"
      echo "| isolated worker crashed | $(grep -a -c '^isolated worker crashed' "$RAW/zend-tests.log" || true) |"
      echo "| isolated worker timed out | $(grep -a -c '^isolated worker timed out' "$RAW/zend-tests.log" || true) |"
      echo
      echo "Files: \`zend-tests.pass.txt\` (regression gate input), \`zend-tests.fails.txt\`, \`zend-tests.skips.tsv\` (path, category, reason)."
    } >"$HERE/zend-tests.md" )
  cat "$RAW/zend-tests.summary"
fi

if [[ " $SUITES " == *" ext "* ]]; then
  rows=()
  for e in $EXTS; do
    if [[ ! -d "$CORPUS/ext/$e/tests" ]]; then
      rows+=("| \`ext/$e\` | — | — | — | — | — | — | no \`ext/$e/tests\` directory in php-src |")
      continue
    fi
    echo ">>> ext/$e"
    [[ "${REPORT_ONLY:-0}" == 1 ]] || run_suite "ext-$e" "$RAW" "ext/$e/tests"
    check_suite_consistent "$RAW" "ext-$e"
    cp "$RAW/ext-$e.pass"  "$HERE/ext/$e.pass.txt"
    cp "$RAW/ext-$e.fails" "$HERE/ext/$e.fails.txt"
    rows+=("$( . "$RAW/ext-$e.summary"
      rate=$(awk -v p="$pass" -v f="$fail" 'BEGIN{ if (p+f>0) printf "%.1f%%", 100*p/(p+f); else printf "n/a" }')
      crash=$(grep -a -c '^isolated worker crashed' "$RAW/ext-$e.log" || true)
      tmo=$(grep -a -c '^isolated worker timed out' "$RAW/ext-$e.log" || true)
      note=""
      [[ "$crash" != 0 ]] && note+="$crash crashed; "
      [[ "$tmo" != 0 ]] && note+="$tmo timed out; "
      # Why tests were skipped: the runner's categories, largest first.
      if [[ "$skip" != 0 ]]; then
        note+="skips: $(cut -f2 "$RAW/ext-$e.skips.tsv" | sort | uniq -c | sort -rn | head -3 | awk '{printf "%s%s %s", (NR>1?", ":""), $1, $2}'); "
      fi
      echo "| \`ext/$e\` | $total | $pass | $fail | $skip | $rate | ${seconds}s | ${note%; } |")")
    cat "$RAW/ext-$e.summary" | tr '\n' ' '; echo
  done
  {
    echo "# Baseline — per-extension \`.phpt\` results"
    echo
    meta
    echo
    echo "\"pass rate\" = pass / (pass + fail); skipped tests never executed."
    echo
    echo "Skip categories are the runner's own: \`extension\` = the test's \`--EXTENSIONS--\` names an"
    echo "extension outside phpt-runner's allowlist (the suite is then **not exercised at all**, whatever"
    echo "phpr implements — true today for \`dom\`, \`simplexml\` and \`filter\`); \`section\` = the test uses a"
    echo ".phpt section the runner does not execute (e.g. \`--SKIPIF--\` without \`--run-skipif\`, \`--POST--\`,"
    echo "\`--CGI--\`); \`builtin\` = calls a function phpr does not register; \`compile-error\` = the test expects a"
    echo "compile-time diagnostic the runner does not model; \`parse\` / \`unsupported\` / \`vm-unsupported\` ="
    echo "the front end or compiler rejected a construct. 1,767 of the \`section\` skips across all suites are \`--SKIPIF--\`."
    echo "A pass rate over a handful of runnable tests (\`ext/dom\`: 2, \`ext/pdo\`: 7) says nothing about the extension."
    echo
    echo "| suite | total | pass | fail | skip | pass rate (of runnable) | wall | notes |"
    echo "|---|---:|---:|---:|---:|---:|---:|---|"
    printf '%s\n' "${rows[@]}"
    echo
    echo "Per-extension lists: \`baseline/ext/<ext>.pass.txt\`, \`baseline/ext/<ext>.fails.txt\`."
  } >"$HERE/ext.md"
fi
echo "baseline written under $HERE"
