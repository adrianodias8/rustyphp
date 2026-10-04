#!/usr/bin/env bash
# bench/drupal/cg-report.sh — subsystem buckets of the current ferro tree vs php
# (HWCOUNTERS_DRUPAL.md §5): line-table build into /target/cg, a callgrind run
# pair (cachegrind.sh TOOL=callgrind M=10), per-handler attribution
# (cg-handlers.py) and the bucket table (cg-buckets.py) against the php TSV
# left by session 14 (/scratch/cg/php-handlers.tsv).
#   docker/run.sh bash /work/php-rust/bench/drupal/cg-report.sh TAG [MEMBERS]
set -euo pipefail
TAG=${1:?tag}; MEM=${2:-0}
D=/work/php-rust/bench/drupal
(cd /work/php-rust/php-rust && CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_TARGET_DIR=/target/cg \
  cargo build --release -p php-cli 2>&1 | tail -1)
ENGINE=ferro BIN=/target/cg/release/ferro TAG=$TAG TOOL=callgrind M=10 bash $D/cachegrind.sh | tail -1
cd /scratch/cg
python3 $D/cg-handlers.py $TAG-0.out $TAG-10.out 10 /target/cg/release/ferro --engine ferro \
  --src /work/php-rust/php-rust/crates/php-runtime/src/vm/run.rs --top 0 --tsv $TAG-handlers.tsv | head -1
python3 $D/cg-buckets.py $TAG-handlers.tsv php-handlers.tsv --members "$MEM"
