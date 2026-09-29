#!/bin/bash
# s182-dev-release-ab.sh [--check] — A/B `--release` vs `--profile dev-release`
# sulla target bundle (criterio: s182-criterio-dev-release.md). Solo numeri.
set -uo pipefail
REPO="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust"
T="$HOME/Claude/phpr-target/dev-output"; MP="$HOME/Claude/phpr-target"
OUT="$REPO/wp182-harness/target-bundle-out"; mkdir -p "$OUT"
V="$OUT/s182-dev-release-verdetto.out"
TOUCH="$REPO/crates/php-runtime/src/coerce.rs"
LOCK=/private/tmp/phpr-measure.lock
ORACLE=/opt/homebrew/opt/php/bin/php
MICRO="$REPO/wp97-harness/micro/arith_small.php"

pregate() {
  local ko=0
  mount | grep -q " $MP " || { echo "PRE-GATE: bundle NON montata ($MP)"; ko=1; }
  if [ -e "$LOCK" ] && ! grep -q 's182-target-bundle' "$LOCK"; then echo "PRE-GATE: lock ALTRUI ($(cat "$LOCK"))"; ko=1; fi
  local p; p=$(pgrep -fl 'phpr |php-server|corpus-gate|phpt-runner|cargo ' | grep -v -e "$0" -e 'target-bundle' -e 'dev-release' || true)
  [ -n "$p" ] && { echo "PRE-GATE: processi vivi:"; echo "$p"; ko=1; }
  local last; last=$(tail -1 "/Volumes/Extreme Pro/Claude/phpr-ci/CI_FEED.log")
  case "$last" in START*) echo "PRE-GATE: job CI in corso ($last)"; ko=1;; esac
  local la; la=$(sysctl -n vm.loadavg | awk '{print $2}')
  awk -v l="$la" 'BEGIN{exit !(l<3)}' || { echo "PRE-GATE: loadavg 1m = $la (>=3)"; ko=1; }
  local free; free=$(df -g "$MP" | awk 'NR==2{print $4}')
  [ "$free" -ge 20 ] || { echo "PRE-GATE: bundle liberi ${free}G (<20)"; ko=1; }
  echo "PRE-GATE: bundle=montata lock=$([ -e "$LOCK" ] && echo nostro || echo assente) loadavg=$la bundle_free=${free}G"
  return $ko
}
pregate || exit 8
[ "${1:-}" = "--check" ] && exit 0
cd "$REPO" || exit 9
{ echo "# S-182 dev-release A/B — $(date '+%F %T')"
  echo "HEAD=$(git rev-parse --short HEAD) dirty=$(git status --porcelain | grep -v '^??' | wc -l | tr -d ' ') rustc=$(rustc --version | awk '{print $2}')"
  echo "target=$T A=--release B=--profile dev-release"; } > "$V"

build() { # build <label> <cargo args...>
  local l=$1; shift; local s e rc
  s=$(date +%s.%N)
  CARGO_TARGET_DIR="$T" /usr/bin/time -p cargo build "$@" > "$OUT/dr-$l.log" 2> "$OUT/dr-$l.time"; rc=$?
  e=$(date +%s.%N)
  printf '%s wall=%.1f user=%s sys=%s rc=%d\n' "$l" "$(echo "$e - $s" | bc)" "$(awk '/^user/{print $2}' "$OUT/dr-$l.time")" "$(awk '/^sys/{print $2}' "$OUT/dr-$l.time")" "$rc" | tee -a "$V"
  [ $rc -eq 0 ] || { echo "BUILD FALLITA: $l (rc=$rc) — corsa NON giudicabile" | tee -a "$V"; exit 1; }
}
# freddo B (solo la cartella del profilo)
rm -rf "$T/dev-release"
build cold-B --profile dev-release
# parità (gate): dev-release e release vs oracle, entrambi i modi
EXP=$("$ORACLE" "$MICRO")
for m in on off; do
  if [ $m = on ]; then R=$(env -u PHPR_REG_LOWER "$T/release/phpr" "$MICRO"); D=$(env -u PHPR_REG_LOWER "$T/dev-release/phpr" "$MICRO")
  else R=$(PHPR_REG_LOWER=0 "$T/release/phpr" "$MICRO"); D=$(PHPR_REG_LOWER=0 "$T/dev-release/phpr" "$MICRO"); fi
  if [ "$EXP" = "$R" ] && [ "$EXP" = "$D" ]; then echo "parita modo=$m release=ok dev-release=ok" | tee -a "$V"
  else echo "parita modo=$m FALLITA (oracle='$EXP' release='$R' dev='$D') ⇒ profilo BOCCIATO" | tee -a "$V"; exit 2; fi
done
# caldo R=3 interleaved
for i in 1 2 3; do
  touch "$TOUCH"; build "warm-A$i" --release
  touch "$TOUCH"; build "warm-B$i" --profile dev-release
done
med() { sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}'; }
w() { grep "^$1" "$V" | sed 's/.*wall=\([0-9.]*\).*/\1/'; }
MA=$(w warm-A | med); MB=$(w warm-B | med); A1=$(w warm-A1); A3=$(w warm-A3)
{ echo "mediana_caldo A=$MA B=$MB B/A=$(echo "scale=3; $MB / $MA" | bc)"
  echo "rumore_A |A1-A3|/A1=$(echo "scale=3; ($A1 - $A3) / $A1" | bc | tr -d -)"
  echo "du release=$(du -sh "$T/release" | cut -f1) dev-release=$(du -sh "$T/dev-release" | cut -f1) bundle_su_disco=$(du -sh "/Volumes/Extreme Pro/Claude/phpr-target.sparsebundle" | cut -f1)"
  echo "soglia: B/A<=0,50 promosso · <=0,80 facoltativo · >0,80 bocciato (s182-criterio-dev-release.md)"; } | tee -a "$V"
echo "verdetto: $V"
