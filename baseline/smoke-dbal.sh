#!/usr/bin/env bash
# baseline/smoke-dbal.sh — PLAN.md §1.2 smoke test (b): PHPUnit on Doctrine
# DBAL (default SQLite config) under the oracle and under phpr, same checkout.
#   docker/run.sh /work/php-rust/baseline/smoke-dbal.sh      (needs network once)
# Dependencies are installed with the ORACLE's composer; only PHPUnit itself is
# run on each engine. Order ABBA. `--no-progress --colors=never` keeps the
# output comparable; PHPUnit's result cache is disabled so runs are independent.
set -uo pipefail
PHPR="${PHPR:-/target/release/phpr}"
PHP="${PHP_ORACLE:-$(command -v php)}"
S="${SCRATCH:-/scratch}/smoke-dbal"
TAG="${DBAL_TAG:-4.5.0}"
TIMEOUT="${TIMEOUT:-1800}"
mkdir -p "$S"; cd "$S"
if [[ ! -d dbal ]]; then
  git clone -q --depth 1 --branch "$TAG" https://github.com/doctrine/dbal.git dbal
  (cd dbal && COMPOSER_ALLOW_SUPERUSER=1 composer install --no-interaction --no-progress --prefer-dist -q)
fi
cd dbal
echo "dbal $(git describe --tags) $(git rev-parse --short HEAD) · $("$PHP" vendor/bin/phpunit --version | head -1)"
OUT="$S/result.tsv"
printf 'run\tengine\trc\twall_s\tuser_s\tsys_s\trss_kb\tphpunit_summary\n' >"$OUT"
run() { # $1 id  $2 label  $3... argv
  local id="$1" label="$2"; shift 2
  local b="$S/$id-$label" rc=0
  /usr/bin/time -v -o "$b.time" timeout "$TIMEOUT" "$@" vendor/bin/phpunit \
      --no-progress --colors=never --do-not-cache-result >"$b.out" 2>"$b.err" || rc=$?
  local wall user sys rss sum
  wall=$(awk -F': ' '/Elapsed \(wall clock\)/{n=split($2,a,":"); s=0; for(i=1;i<=n;i++) s=s*60+a[i]; print s}' "$b.time")
  user=$(awk -F': ' '/User time/{print $2}' "$b.time")
  sys=$(awk -F': ' '/System time/{print $2}' "$b.time")
  rss=$(awk -F': ' '/Maximum resident set size/{print $2}' "$b.time")
  sum=$(grep -a -E '^(OK|Tests: |ERRORS!|FAILURES!|WARNINGS!)' "$b.out" | tr '\n' ' ' | sed 's/ *$//')
  [[ -n "$sum" ]] || sum="(no PHPUnit summary) $(tail -c 300 "$b.out" "$b.err" | tr '\n' ' ' | cut -c1-220)"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$id" "$label" "$rc" "$wall" "$user" "$sys" "$rss" "$sum" | tee -a "$OUT"
}
run 1 oracle "$PHP"
run 2 phpr   "$PHPR"
run 3 phpr   "$PHPR"
run 4 oracle "$PHP"
