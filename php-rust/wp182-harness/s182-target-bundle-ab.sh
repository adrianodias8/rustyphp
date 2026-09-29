#!/bin/bash
# s182-target-bundle-ab.sh [--check] — A/B target interna vs sparsebundle
# (criterio: s182-criterio-target-bundle.md). Solo numeri nel verdetto.
set -uo pipefail
REPO="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust"
A="$HOME/Claude/php-rust-dev-output"
B="$HOME/Claude/phpr-target/dev-output"
MP="$HOME/Claude/phpr-target"
OUT="$REPO/wp182-harness/target-bundle-out"; mkdir -p "$OUT"
V="$OUT/s182-target-bundle-verdetto.out"
TOUCH="$REPO/crates/php-runtime/src/coerce.rs"
LOCK=/private/tmp/phpr-measure.lock

pregate() {
  local ko=0
  mount | grep -q " $MP " || { echo "PRE-GATE: bundle NON montata ($MP)"; ko=1; }
  # lock di misura: accettato SOLO se è il nostro (token s182-target-bundle, messo dal waiter)
  if [ -e "$LOCK" ] && ! grep -q 's182-target-bundle' "$LOCK"; then echo "PRE-GATE: lock di misura ALTRUI ($LOCK: $(cat "$LOCK"))"; ko=1; fi
  # processi: il ci-runner in pausa (quiet_wait) è idle e NON conta; contano job CI in corso e binari vivi
  local p; p=$(pgrep -fl 'phpr |php-server|corpus-gate|phpt-runner|cargo ' | grep -v -e "$0" -e 'target-bundle' || true)
  [ -n "$p" ] && { echo "PRE-GATE: processi vivi:"; echo "$p"; ko=1; }
  local last; last=$(tail -1 "/Volumes/Extreme Pro/Claude/phpr-ci/CI_FEED.log")
  case "$last" in START*) echo "PRE-GATE: job CI in corso ($last)"; ko=1;; esac
  local la; la=$(sysctl -n vm.loadavg | awk '{print $2}')
  awk -v l="$la" 'BEGIN{exit !(l<3)}' || { echo "PRE-GATE: loadavg 1m = $la (>=3)"; ko=1; }
  local free; free=$(df -g /System/Volumes/Data | awk 'NR==2{print $4}')
  [ "$free" -ge 10 ] || { echo "PRE-GATE: Data liberi ${free}G (<10)"; ko=1; }
  [ -d "$B" ] || { echo "PRE-GATE: $B assente"; ko=1; }
  echo "PRE-GATE: bundle=montata lock=$([ -e "$LOCK" ] && echo nostro || echo assente) loadavg=$la data_free=${free}G"
  return $ko
}
pregate || exit 8
[ "${1:-}" = "--check" ] && exit 0

cd "$REPO" || exit 9
{
  echo "# S-182 target-bundle A/B — $(date '+%F %T')"
  echo "HEAD=$(git rev-parse --short HEAD) dirty=$(git status --porcelain | grep -v '^??' | wc -l | tr -d ' ') rustc=$(rustc --version | awk '{print $2}')"
  echo "A=$A"; echo "B=$B"
} > "$V"

build() { # build <target> <label>  → appende "label wall user sys rc"
  local t=$1 l=$2 s e rc
  s=$(date +%s.%N)
  CARGO_TARGET_DIR="$t" /usr/bin/time -p cargo build --release > "$OUT/$l.log" 2> "$OUT/$l.time"; rc=$?
  e=$(date +%s.%N)
  local u sy; u=$(awk '/^user/{print $2}' "$OUT/$l.time"); sy=$(awk '/^sys/{print $2}' "$OUT/$l.time")
  printf '%s wall=%.1f user=%s sys=%s rc=%d\n' "$l" "$(echo "$e - $s" | bc)" "$u" "$sy" "$rc" | tee -a "$V"
  [ $rc -eq 0 ] || { echo "BUILD FALLITA: $l (rc=$rc) — corsa NON giudicabile" | tee -a "$V"; exit 1; }
}

# freddo R=1 per braccio (svuota SOLO il contenuto: il mountpoint B è dentro la bundle)
rm -rf "$A"; mkdir -p "$A"; find "$B" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
build "$A" cold-A
build "$B" cold-B
# caldo R=3 interleaved
for i in 1 2 3; do
  touch "$TOUCH"; build "$A" "warm-A$i"
  touch "$TOUCH"; build "$B" "warm-B$i"
done
# giudice: mediana caldo, rapporto B/A
med() { sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}'; }
MA=$(grep '^warm-A' "$V" | sed 's/.*wall=\([0-9.]*\).*/\1/' | med)
MB=$(grep '^warm-B' "$V" | sed 's/.*wall=\([0-9.]*\).*/\1/' | med)
A1=$(grep '^warm-A1' "$V" | sed 's/.*wall=\([0-9.]*\).*/\1/'); A3=$(grep '^warm-A3' "$V" | sed 's/.*wall=\([0-9.]*\).*/\1/')
{
  echo "mediana_caldo A=$MA B=$MB B/A=$(echo "scale=3; $MB / $MA" | bc)"
  echo "rumore_A |A1-A3|/A1=$(echo "scale=3; ($A1 - $A3) / $A1" | bc | tr -d -)"
  echo "du A=$(du -sh "$A" | cut -f1) B=$(du -sh "$B" | cut -f1) bundle_su_disco=$(du -sh "/Volumes/Extreme Pro/Claude/phpr-target.sparsebundle" | cut -f1)"
  echo "soglia: B/A<=1,25 dev · <=1,50 solo CI · >1,50 bocciata (criterio s182-criterio-target-bundle.md)"
} | tee -a "$V"
echo "verdetto: $V"
