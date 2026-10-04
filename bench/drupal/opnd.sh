#!/bin/bash
# Operand-kind census of one warm Drupal front page (op-census + mem-census
# build, vm/opndcensus.rs): per op, the kind of each consumed stack operand
# (CV/CONST/TMP/DUP/THIS) by value class, the fate of the Rc-carrying
# CV/CONST/THIS ones (dropped by the consumer vs kept), the Zval clones and
# string-handle drops made while the op ran, and the operand-kind combinations.
# Inside the image:   bash opnd.sh [build]    -> /scratch/opnd-last.txt
if [ "$1" = build ]; then
  (cd /work/php-rust/php-rust && CARGO_TARGET_DIR=/target/opnd cargo build --release -p php-cli \
    --features php-runtime/op-census,php-cli/mem-census 2>&1 | grep -E "^error" -A10)
fi
rm -f /scratch/opnd.txt
cd "${SITE:-/scratch/drupal-srv-fe}/web"
PHPR_OP_CENSUS=/scratch/opc.txt PHPR_OPND_CENSUS=/scratch/opnd.txt /target/opnd/release/ferro \
  -S 127.0.0.1:8246 .ht.router.php >/scratch/opnd.log 2>&1 & P=$!
sleep 1
for i in 1 2 3 4 5; do curl -s -o /dev/null http://127.0.0.1:8246/; done
sleep 0.5; kill $P; wait $P 2>/dev/null
# The census dumps once per request: keep the last (warm) one.
L=$(grep -an "== PHPR_OPND_CENSUS" /scratch/opnd.txt | tail -1 | cut -d: -f1)
sed -n "$L,\$p" /scratch/opnd.txt > /scratch/opnd-last.txt
wc -l /scratch/opnd-last.txt
