#!/usr/bin/env bash
# baseline/cli-server-router.sh — `ferro -S … router.php` vs the oracle's
# `php -S`: the SAPI variables the router sees (SCRIPT_FILENAME, SCRIPT_NAME,
# PHP_SELF, PATH_INFO) for paths that resolve to a script, to a static file,
# to the index.php fallback, or to nothing. Symfony Runtime front controllers
# (Drupal's autoload_runtime.php) include SCRIPT_FILENAME. Session 10 fix.
#   docker/run.sh /work/php-rust/baseline/cli-server-router.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FERRO="${FERRO:-/target/release/ferro}"; PHP="${PHP_ORACLE:-$(command -v php)}"
URLS="/ /index.php /style.css /nope /sub/page.php /sub/page.php/extra /sub/ /sub/nope /x/y?q=1"
fail=0
for site in router-index router-plain; do
  cd "$HERE/cli-server/$site"
  "$PHP" -S 127.0.0.1:8291 ../router.php >/dev/null 2>&1 & P=$!
  "$FERRO" -S 127.0.0.1:8292 ../router.php >/dev/null 2>&1 & F=$!
  sleep 1
  for u in $URLS; do
    a=$(curl -s "http://127.0.0.1:8291$u"); b=$(curl -s "http://127.0.0.1:8292$u")
    if [ "$a" != "$b" ]; then echo "DIFF $site $u: php=[$a] ferro=[$b]"; fail=1; fi
  done
  kill $P $F 2>/dev/null; wait 2>/dev/null
  # The same docroot without a router: status code and what the script sees.
  "$PHP" -S 127.0.0.1:8291 >/dev/null 2>&1 & P=$!
  "$FERRO" -S 127.0.0.1:8292 >/dev/null 2>&1 & F=$!
  sleep 1
  for u in $URLS; do
    a=$(curl -s -w ' %{http_code}' "http://127.0.0.1:8291$u" | grep -v '^<'); b=$(curl -s -w ' %{http_code}' "http://127.0.0.1:8292$u" | grep -v '^<')
    if [ "$a" != "$b" ]; then echo "DIFF $site (no router) $u: php=[$a] ferro=[$b]"; fail=1; fi
  done
  kill $P $F 2>/dev/null; wait 2>/dev/null
done
[ $fail = 0 ] && echo "cli-server path resolution (router and plain): IDENTICAL" || exit 1
