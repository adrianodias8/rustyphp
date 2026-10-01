#!/usr/bin/env bash
# bench/drupal/frontpage.sh — step 2 parity: the anonymous front page of one
# Drupal install (/scratch/drupal-base: an oracle-made standard install with
# page_cache uninstalled) served by the oracle (`php -S`) and by ferro
# (one-shot cli-server, `ferro -S`), each on its own copy, compared byte for
# byte after stripping per-request tokens. Requests: cold `/`, warm `/`,
# `/node`, `/user/login`.
#   docker/run.sh /work/php-rust/bench/drupal/frontpage.sh
set -uo pipefail
FERRO="${FERRO:-/target/release/ferro}"; PHP="${PHP_ORACLE:-$(command -v php)}"
S=/scratch/frontpage; rm -rf "$S"; mkdir -p "$S"
for side in or fe; do rm -rf /scratch/drupal-srv-$side; cp -a /scratch/drupal-base /scratch/drupal-srv-$side; done
(cd /scratch/drupal-srv-or/web && exec "$PHP" -S 127.0.0.1:8201 .ht.router.php >"$S/or.log" 2>&1) & P1=$!
(cd /scratch/drupal-srv-fe/web && exec "$FERRO" -S 127.0.0.1:8202 .ht.router.php >"$S/fe.log" 2>&1) & P2=$!
sleep 1
strip() { # per-request tokens: form build ids/tokens, random view DOM ids, the port, the Date header
  sed -E -e 's/form-[A-Za-z0-9_-]{43}/form-TOKEN/g' -e 's/data-drupal-selector="form-[a-z0-9-]+"/data-drupal-selector="form-TOKEN"/g' -e 's/127\.0\.0\.1:820[12]/127.0.0.1:82XX/g' \
         -e 's/js-view-dom-id-[0-9a-f]{64}/js-view-dom-id-X/g' -e 's/"view_dom_id":"[0-9a-f]{64}"/"view_dom_id":"X"/g' -e 's/"form_build_id"[^>]*value="[^"]*"/"form_build_id" value="X"/g' \
         -e 's/^Date: .*/Date: X/' -e 's/^(Expires|Last-Modified): .*/\1: X/'
}
i=0; same=0
for path in / / /node /user/login; do
  i=$((i+1))
  for side in or fe; do
    port=$([ $side = or ] && echo 8201 || echo 8202)
    curl -s -D "$S/$i.$side.h" -o "$S/$i.$side.b" -w "%{time_total}" "http://127.0.0.1:$port$path" >"$S/$i.$side.t"
    { tr -d '\r' <"$S/$i.$side.h" | grep -viE '^(Host|Connection|X-Powered-By):'; cat "$S/$i.$side.b"; } | strip >"$S/$i.$side.n"
  done
  if diff -q "$S/$i.or.n" "$S/$i.fe.n" >/dev/null; then r=IDENTICAL; same=$((same+1)); else r="DIFF ($(diff "$S/$i.or.n" "$S/$i.fe.n" | grep -c '^[<>]') lines)"; fi
  printf '%-12s %-10s oracle %ss  ferro %ss  %s bytes  %s\n' "$path" "#$i" "$(cat $S/$i.or.t)" "$(cat $S/$i.fe.t)" "$(wc -c <"$S/$i.or.b")" "$r"
done
kill $P1 $P2 2>/dev/null; wait 2>/dev/null
echo "$same/$i identical (transcripts in $S)"
