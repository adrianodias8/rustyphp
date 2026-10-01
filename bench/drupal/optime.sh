#!/bin/bash
# Op-time census of one warm Drupal front page (op-census build with
# PHPR_OP_CENSUS_TIME): per-op net time and per-callee time. Inside the image:
#   bash optime.sh [build]     (build = rebuild /target/census first)
if [ "$1" = build ]; then
  (cd /work/php-rust/php-rust && CARGO_TARGET_DIR=/target/census cargo build --release -p php-cli --features php-runtime/op-census 2>&1 | grep -E "^error" -A10)
fi
cd /scratch/drupal-srv-fe/web
PHPR_OP_CENSUS=1 PHPR_OP_CENSUS_TIME=1 PHPR_GC=${GCMODE:-} /target/census/release/ferro -S 127.0.0.1:8245 .ht.router.php 2>/scratch/census.log >/dev/null & P=$!
sleep 1
for i in 1 2 3 4 5; do curl -s -o /dev/null http://127.0.0.1:8245/; done
sleep 0.5; kill $P; wait $P 2>/dev/null
L=$(grep -an "== PHPR_OP_CENSUS" /scratch/census.log | tail -1 | cut -d: -f1)
sed -n "$L,\$p" /scratch/census.log | sed -n "1p;/op time/,/bigrams/p" | head -${N:-30}
