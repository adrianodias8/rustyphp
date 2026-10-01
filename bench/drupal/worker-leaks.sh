#!/usr/bin/env bash
# bench/drupal/worker-leaks.sh — step 2: what Drupal state leaks between
# requests served by ONE ferro worker. The same request sequence goes to the
# oracle's one-shot `php -S` (the reference: a fresh process per request) and
# to a single ferro worker running bench/drupal/drupal-worker.php; every
# response pair is compared after the frontpage.sh token stripping.
#   RESET="drupal_static,request_stack" docker/run.sh /work/php-rust/bench/drupal/worker-leaks.sh
set -uo pipefail
FERRO="${FERRO:-/target/release/ferro}"; PHP="${PHP_ORACLE:-$(command -v php)}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
S=/scratch/worker-leaks; rm -rf "$S"; mkdir -p "$S"
for side in or wk; do rm -rf /scratch/drupal-$side; cp -a /scratch/drupal-base /scratch/drupal-$side; done
cp "$HERE/drupal-worker.php" /scratch/drupal-wk/web/drupal-worker.php
(cd /scratch/drupal-or/web && exec "$PHP" -S 127.0.0.1:8201 .ht.router.php >"$S/or.log" 2>&1) & P1=$!
(cd /scratch/drupal-wk/web && DRUPAL_ROOT=/scratch/drupal-wk/web DRUPAL_WORKER_RESET="${RESET:-}" DRUPAL_WORKER_STATICS="${STATICS:-}" DRUPAL_WORKER_DEBUG="$S/debug.log" \
   exec "$FERRO" -S 127.0.0.1:8203 -t /scratch/drupal-wk/web --worker drupal-worker.php --workers 1 >"$S/wk.log" 2>&1) & P2=$!
sleep 2
strip() {
  sed -E -e 's/form-[A-Za-z0-9_-]{43}/form-TOKEN/g' -e 's/data-drupal-selector="form-[a-z0-9-]+"/data-drupal-selector="form-TOKEN"/g' -e 's/127\.0\.0\.1:820[13]/127.0.0.1:82XX/g' \
         -e 's/js-view-dom-id-[0-9a-f]{64}/js-view-dom-id-X/g' -e 's/"view_dom_id":"[0-9a-f]{64}"/"view_dom_id":"X"/g' \
         -e 's/^Date: .*/Date: X/' -e '/^Content-Length: /d' -e 's/^(Expires|Last-Modified): .*/\1: X/'
}
PATHS="${PATHS:-/ /user/login /node/999 / /user/login?destination=/x /user/password /rss.xml / /node /user/login}"
i=0; same=0
for path in $PATHS; do
  i=$((i+1))
  for side in or wk; do
    port=$([ $side = or ] && echo 8201 || echo 8203)
    curl -s -D "$S/$i.$side.h" -o "$S/$i.$side.b" "http://127.0.0.1:$port$path"
    { tr -d '\r' <"$S/$i.$side.h" | grep -viE '^(Host|Connection|X-Powered-By):'; cat "$S/$i.$side.b"; } | strip >"$S/$i.$side.n"
  done
  if diff -q "$S/$i.or.n" "$S/$i.wk.n" >/dev/null; then r=IDENTICAL; same=$((same+1)); else r="DIFF ($(diff "$S/$i.or.n" "$S/$i.wk.n" | grep -c '^[<>]') lines)"; fi
  printf '%2d %-28s %s %s\n' "$i" "$path" "$(head -1 "$S/$i.or.n" | cut -d' ' -f2)/$(head -1 "$S/$i.wk.n" | cut -d' ' -f2)" "$r"
done
kill $P1 $P2 2>/dev/null; wait 2>/dev/null
echo "$same/$i identical with RESET='${RESET:-}' (transcripts in $S)"
