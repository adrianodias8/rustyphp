#!/usr/bin/env bash
# bench/drupal/install.sh — Drupal 11 standard install (SQLite) through drush,
# on a fresh copy of the codebase, under ferro (default) or the oracle.
#   docker/run.sh /work/php-rust/bench/drupal/install.sh            # ferro
#   ENGINE=php docker/run.sh /work/php-rust/bench/drupal/install.sh # oracle
# The codebase (drupal/recommended-project ^11 + drush/drush) lives in
# /scratch/drupal; see bench/drupal/README.md for how it was created.
set -uo pipefail
ENGINE="${ENGINE:-/target/release/ferro}"
SRC="${DRUPAL_SRC:-/scratch/drupal}"
DST="/scratch/drupal-$(basename "$ENGINE")"
LOG="/scratch/drupal-$(basename "$ENGINE").log"
rm -rf "$DST" && cp -a "$SRC" "$DST" && cd "$DST" || exit 2
/usr/bin/time -f "wall %es rss %MKB" timeout "${TIMEOUT:-900}" "$ENGINE" -d memory_limit=-1 \
  vendor/drush/drush/drush.php site:install standard \
  --db-url=sqlite://sites/default/files/.ht.sqlite --account-pass=admin --site-name=Ferrophant -y \
  >"$LOG" 2>&1
echo "exit $?"
grep -n -m3 -iE "fatal|uncaught|error:|exception|success" "$LOG" | cut -c1-400
tail -3 "$LOG" | cut -c1-300
