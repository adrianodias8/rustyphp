#!/usr/bin/env bash
# bench/edge/purge-roundtrip.sh — ferro-edge against a real Drupal 11 + Purge
# (purge, purge_queuer_coretags, purge_purger_http + its tags header), as a site
# would run it. Run from the host:  ORIGIN=fpm|ferro bench/edge/purge-roundtrip.sh
#   1. a copy of the base with Purge installed (composer), page max-age 3600,
#      12 promoted articles; the HTTP Bundled Purger sends BAN to the edge
#   2. for each purger header — Purge-Cache-Tags (Varnish regex semantics) and
#      Surrogate-Key (exact) — warm every probe page through the edge, record
#      each page's tags from the origin, edit node 1 (drush), work the purge
#      queue, and compare the pages the edge dropped with the pages whose tags
#      the edit invalidated (exactly, and as Varnish's regex would)
#   3. session bypass: a logged-in admin's pages pass, never cached, and anonymous
#      pages never show the admin's
#   4. wrk, a mix of anonymous and authenticated requests (AUTH share), on the
#      origin alone and behind the edge
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"; ROOT="$(cd "$REPO/.." && pwd)"
IMAGE="${RUSTYPHP_IMAGE:-rustyphp-dev:8.5.7}"
ORIGIN="${ORIGIN:-fpm}"; WORKERS="${WORKERS:-4}"; AUTH="${AUTH:-0.1}"; DURATION="${DURATION:-15}"; CONNS="${CONNS:-32}"
NET=rustyphp-purge; SITE=/scratch/drupal-purge-run; PREP=/scratch/drupal-purge-prep
OUT="${OUT:-$REPO/bench/results/$(date -u +%Y-%m-%d)-edge-purge-$ORIGIN.md}"
MOUNTS=(-v "$ROOT":/work:ro -v rustyphp-scratch:/scratch)
TMP="$(mktemp -d)"
cleanup() { for c in pr-fpm pr-nginx pr-ferro pr-edge; do docker rm -f "$c" >/dev/null 2>&1; done; docker network rm "$NET" >/dev/null 2>&1; }
[[ -n "${KEEP:-}" ]] || trap cleanup EXIT; cleanup
docker network create "$NET" >/dev/null
in_net() { docker run --rm --network "$NET" "${MOUNTS[@]}" "$IMAGE" "$@"; }
drush() { in_net bash -c "cd $SITE && php vendor/drush/drush/drush.php -r web $*"; }

# ---- the site: Purge installed once into $PREP, copied per run ----
docker run --rm "${MOUNTS[@]}" "$IMAGE" bash -c "
  set -e
  if [[ ! -d $PREP ]]; then
    cp -a /scratch/drupal-base $PREP.tmp && cd $PREP.tmp
    COMPOSER_ALLOW_SUPERUSER=1 composer require -n -q drupal/purge drupal/purge_purger_http
    D='php vendor/drush/drush/drush.php -r web'
    \$D -q -y en purge purge_tokens purge_queuer_coretags purge_processor_lateruntime purge_purger_http purge_purger_http_tagsheader
    \$D -q -y cset system.performance cache.page.max_age 3600
    # this base install has no content types: an article type, then 12 promoted articles
    \$D -q php:eval '\Drupal\node\Entity\NodeType::create([\"type\" => \"article\", \"name\" => \"Article\"])->save();'
    \$D -q php:eval 'foreach (range(1, 12) as \$i) { \Drupal\node\Entity\Node::create([\"type\" => \"article\", \"title\" => \"Article \$i\", \"promote\" => 1, \"status\" => 1])->save(); }'
    mv $PREP.tmp $PREP
  fi
  rm -rf $SITE && cp -a $PREP $SITE && chmod -R a+rwX $SITE
  # Work the queue explicitly (drush) instead of the late runtime processor:
  # with it, the BAN at the end of the editing request lacked the entity's own
  # tag (node:N), which was queued but never sent (NOTES.md session 11)
  cd $SITE && php vendor/drush/drush/drush.php -r web -q -y p:processor-rm lateruntime
  php vendor/drush/drush/drush.php -r web -q p:processor-add drush_purge_queue_work"

# ---- origin + edge ----
if [[ "$ORIGIN" == fpm ]]; then
  sed "s/^pm.max_children = .*/pm.max_children = $WORKERS/; /SYMFONY_DIR/d" "$REPO/bench/worker/fpm/zz-bench.conf" >"$TMP/zz-bench.conf"
  sed "s#/scratch/drupal-b-fpm/#$SITE/#" "$REPO/bench/drupal/nginx/drupal.conf" >"$TMP/nginx.conf"
  docker run -d --name pr-fpm --network "$NET" --network-alias fpm "${MOUNTS[@]}" -v "$TMP/zz-bench.conf":/usr/local/etc/php-fpm.d/zz-bench.conf:ro \
    -v "$REPO/bench/worker/fpm/opcache.ini":/usr/local/etc/php/conf.d/opcache.ini:ro php:8.5.7-fpm >/dev/null
  docker run -d --name pr-nginx --network "$NET" "${MOUNTS[@]}" -v "$TMP/nginx.conf":/etc/nginx/conf.d/default.conf:ro nginx:alpine >/dev/null
  UP=pr-nginx:8080
else
  docker run -d --name pr-ferro --network "$NET" "${MOUNTS[@]}" -v rustyphp-target:/target:ro "$IMAGE" bash -c \
    "cd $SITE/web && exec /target/release/ferro -S 0.0.0.0:8080 -t $SITE/web .ht.router.php --workers $WORKERS 2>/dev/null" >/dev/null
  UP=pr-ferro:8080
fi
docker run -d --name pr-edge --network "$NET" --network-alias edge -v rustyphp-target:/target:ro "$IMAGE" \
  /target/release/ferro-edge --listen 0.0.0.0:8081 --upstream "$UP" --log-bans >/dev/null
in_net bash -c "for i in \$(seq 1 60); do curl -s -o /dev/null -f http://$UP/ && break; sleep 1; done"
[[ -n "${SETUP_ONLY:-}" ]] && { echo "up: origin $UP, edge edge:8081 on network $NET (KEEP=1 to keep)"; exit 0; }

PAGES="/ /node /rss.xml /node/1 /node/2 /node/10 /node/11 /node/12 /user/login"
report() { echo "$*" | tee -a "$TMP/report"; }
: >"$TMP/report"
pass_all=1
for mode in "Purge-Cache-Tags regex" "Surrogate-Key exact"; do
  header="${mode%% *}"; sem="${mode##* }"
  report ""; report "### purger header \`$header: [invalidations:separated_pipe]\` ($sem)"
  drush "php:script /work/php-rust/bench/edge/purge-config.php -- edge 8081 $header '[invalidations:separated_pipe]'" | tail -1
  drush "-q cr"; drush "-q p:queue-empty"
  # warm through the edge (twice: miss then hit), and each page's tags from the origin
  in_net bash -c "for p in $PAGES; do curl -s -o /dev/null http://edge:8081\$p; curl -s -o /dev/null http://edge:8081\$p; done
    for p in $PAGES; do printf '%s ' \$p; curl -s -D- -o /dev/null http://$UP\$p | tr -d '\r' | awk -F': ' 'tolower(\$1)==\"purge-cache-tags\"{print \$2; f=1} END{if(!f) print \"\"}'; done" >"$TMP/tags"
  in_net bash -c "for p in $PAGES; do printf '%s %s\n' \$p \$(curl -s -D- -o /dev/null http://edge:8081\$p | tr -d '\r' | awk -F': ' 'tolower(\$1)==\"x-cache\"{print \$2}'); done" >"$TMP/before"
  # the edit, the queue, the BAN
  drush "-q php:eval '\$n = \Drupal\node\Entity\Node::load(1); \$n->setTitle(\"Edited \" . time()); \$n->save();'"
  drush "p:queue-browse --format=json --limit=200" >"$TMP/queue.json" 2>/dev/null
  drush "p:queue-work --finish" >/dev/null 2>&1
  in_net bash -c "for p in $PAGES; do printf '%s %s\n' \$p \$(curl -s -D- -o /dev/null http://edge:8081\$p | tr -d '\r' | awk -F': ' 'tolower(\$1)==\"x-cache\"{print \$2}'); done" >"$TMP/after"
  docker logs pr-edge 2>&1 | grep "ferro-edge: BAN" | tail -1 | cut -c1-400 | sed 's/^/    edge log: /' | tee -a "$TMP/report"
  # the pages showing node 1 must show its new title, from the edge
  in_net bash -c "t=\$(cd $SITE && php vendor/drush/drush/drush.php -r web php:eval 'echo \\Drupal\\node\\Entity\\Node::load(1)->getTitle();'); for p in /node/1 /rss.xml; do printf '    new title on %s through the edge: %s\n' \$p \$(curl -s http://edge:8081\$p | grep -c \"\$t\"); done" | tee -a "$TMP/report"
  python3 - "$TMP" "$sem" <<'PY' | tee -a "$TMP/report"
import json, re, sys
tmp, sem = sys.argv[1], sys.argv[2]
q = json.load(open(f"{tmp}/queue.json")) if open(f"{tmp}/queue.json").read().strip() else []
inv = sorted({i.get("expression") for i in (q.values() if isinstance(q, dict) else q) if str(i.get("type", "")).lower() == "tag"})
inv = [t for t in inv if t]
tags = {}
for line in open(f"{tmp}/tags"):
    p, _, t = line.rstrip("\n").partition(" ")
    tags[p] = t.split()
before = dict(l.split() for l in open(f"{tmp}/before") if len(l.split()) == 2)
after = dict(l.split() for l in open(f"{tmp}/after") if len(l.split()) == 2)
rx = re.compile("|".join(inv)) if inv else None
print(f"invalidated tags ({len(inv)}): {' '.join(inv)}")
print()
print("| page | cached before | exact match | Varnish regex match | edge after | expected | ok |")
print("|---|---|---|---|---|---|---|")
bad = 0
for p in tags:
    exact = bool(set(tags[p]) & set(inv))
    regex = bool(rx and rx.search(" ".join(tags[p])))
    expected_miss = exact if sem == "exact" else regex
    got_miss = after.get(p) == "MISS"
    ok = before.get(p) == "HIT" and got_miss == expected_miss
    bad += not ok
    print(f"| `{p}` | {before.get(p)} | {'yes' if exact else 'no'} | {'yes' if regex else 'no'} | {after.get(p)} | {'MISS' if expected_miss else 'HIT'} | {'✓' if ok else '✗'} |")
print()
print(f"RESULT: {'PASS' if not bad else f'FAIL ({bad} pages)'}")
PY
  grep "RESULT:" "$TMP/report" | tail -1 | grep -q PASS || pass_all=0
done

# ---- session bypass ----
report ""; report "### session-cookie bypass"
link="$(drush "uli --uri=http://edge:8081 --no-browser" | tail -1 | tr -d '\r')"
in_net bash -c "
  curl -s -c /tmp/j -b /tmp/j -o /dev/null -L '$link'
  sess=\$(awk '\$6 ~ /^S?SESS/ {print \$6\"=\"\$7}' /tmp/j)
  echo \"cookie: \${sess%%=*}=…\"
  a=\$(curl -s -D /tmp/h -b \"\$sess\" http://edge:8081/); echo \"admin /: x-cache \$(awk -F': ' 'tolower(\$1)==\"x-cache\"{print \$2}' /tmp/h | tr -d '\r'), logged in (drupalSettings uid 1): \$(grep -c '\"uid\":\"1\"' <<<\"\$a\")\"
  a=\$(curl -s -D /tmp/h -b \"\$sess\" http://edge:8081/node/1/edit); echo \"admin /node/1/edit: status \$(head -1 /tmp/h | tr -d '\r'), x-cache \$(awk -F': ' 'tolower(\$1)==\"x-cache\"{print \$2}' /tmp/h | tr -d '\r')\"
  a=\$(curl -s -D /tmp/h http://edge:8081/); echo \"anonymous /: x-cache \$(awk -F': ' 'tolower(\$1)==\"x-cache\"{print \$2}' /tmp/h | tr -d '\r'), shows the admin's page: \$(grep -c '\"uid\":\"1\"' <<<\"\$a\")\"
  echo \"\$sess\" > /scratch/purge-session.txt
" | tee -a "$TMP/report"

# ---- mixed anonymous / authenticated load ----
# bench/edge/mix.lua: a share AUTH of the requests carries the admin's session cookie
report ""; report "### wrk, front page, ${AUTH} of requests logged in ($CONNS connections, ${DURATION}s after a ${WARMUP:-10}s warm-up with the same mix, $WORKERS origin workers)"
report ""; report "| target | req/s | p50 | p99 | edge: hit / pass / origin |"; report "|---|---:|---:|---:|---|"
for target in "origin http://$UP/" "edge http://edge:8081/"; do
  name="${target%% *}"; url="${target#* }"
  # warm-up with the same mix (per-thread caches on ferro, admin code paths)
  in_net bash -c "SESSION=\$(cat /scratch/purge-session.txt) AUTH=$AUTH wrk -t2 -c$CONNS -d${WARMUP:-10}s -s /work/php-rust/bench/edge/mix.lua $url >/dev/null"
  in_net bash -c "s0=\$(curl -s -X STATS http://edge:8081/)
    o=\$(SESSION=\$(cat /scratch/purge-session.txt) AUTH=$AUTH wrk -t2 -c$CONNS -d${DURATION}s --latency -s /work/php-rust/bench/edge/mix.lua $url)
    s1=\$(curl -s -X STATS http://edge:8081/)
    rps=\$(awk '/Requests\/sec/{print \$2}' <<<\"\$o\"); p50=\$(awk '\$1==\"50%\"{print \$2}' <<<\"\$o\"); p99=\$(awk '\$1==\"99%\"{print \$2}' <<<\"\$o\")
    d() { awk -v k=\$1 -v a=\"\$s0\" -v b=\"\$s1\" 'BEGIN{n=split(a,x,\" \"); split(b,y,\" \"); for(i=1;i<n;i+=2) if (x[i]==k) print y[i+1]-x[i+1]}'; }
    echo \"| $name | \$rps | \$p50 | \$p99 | \$(d hit) / \$(d pass) / \$(d upstream) |\"" | tee -a "$TMP/report"
done

{
  echo "# ferro-edge + Drupal 11 + Purge — origin: $ORIGIN — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo
  echo "- ferro-edge $(git -C "$REPO" rev-parse --short HEAD); origin \`$UP\` ($ORIGIN, $WORKERS workers); Purge $(docker run --rm "${MOUNTS[@]}" "$IMAGE" bash -c "grep -m1 '^version' $SITE/web/modules/contrib/purge/purge.info.yml | cut -d\\' -f2")"
  echo "- page max-age 3600, purge_queuer_coretags, HTTP Bundled Purger → \`BAN http://edge:8081/\`; the edit is \`Node::load(1)->setTitle()->save()\` (drush), the queue worked with \`drush p:queue-work --finish\`"
  cat "$TMP/report"
} >"$OUT"
echo "wrote $OUT"
[[ $pass_all == 1 ]]
