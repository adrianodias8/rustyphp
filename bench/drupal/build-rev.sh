#!/usr/bin/env bash
# bench/drupal/build-rev.sh — release-build a committed revision (default HEAD)
# into its own target dir, for an A/B against the working tree:
#   bench/drupal/build-rev.sh [REV] [DIR]     (from the host; DIR defaults to head)
# -> /target/$DIR/release/ferro, built from a plain copy of REV's php-rust/.
set -euo pipefail
REV=${1:-HEAD}; DIR=${2:-head}
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
git -C "$REPO" archive --format=tar "$REV" php-rust > "$REPO/../.rev-$DIR.tar"
"$REPO/docker/run.sh" bash -c "rm -rf /scratch/src-$DIR && mkdir -p /scratch/src-$DIR && tar -xf /work/.rev-$DIR.tar -C /scratch/src-$DIR \
  && cd /scratch/src-$DIR/php-rust && CARGO_TARGET_DIR=/target/$DIR cargo build --release -p php-cli 2>&1 | tail -1"
rm -f "$REPO/../.rev-$DIR.tar"
