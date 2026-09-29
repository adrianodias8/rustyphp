#!/bin/bash
# s182-dev-release-edit-ab.sh [--check] — gamba EDIT (emenda E1 del criterio
# s182-criterio-dev-release.md): edit reale su coerce.rs a ogni build.
set -uo pipefail
REPO="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust"
T="$HOME/Claude/phpr-target/dev-output"; MP="$HOME/Claude/phpr-target"
OUT="$REPO/wp182-harness/target-bundle-out"; mkdir -p "$OUT"
V="$OUT/s182-dev-release-edit-verdetto.out"
F="$REPO/crates/php-runtime/src/coerce.rs"; ORIG="$OUT/coerce.rs.orig"
LOCK=/private/tmp/phpr-measure.lock
pregate() {
  local ko=0
  mount | grep -q " $MP " || { echo "PRE-GATE: bundle NON montata"; ko=1; }
  if [ -e "$LOCK" ] && ! grep -q 's182-target-bundle' "$LOCK"; then echo "PRE-GATE: lock ALTRUI ($(cat "$LOCK"))"; ko=1; fi
  local p; p=$(pgrep -fl 'phpr |php-server|corpus-gate|phpt-runner|cargo ' | grep -v -e "$0" -e 'target-bundle' -e 'dev-release' || true)
  [ -n "$p" ] && { echo "PRE-GATE: processi vivi:"; echo "$p"; ko=1; }
  local last; last=$(tail -1 "/Volumes/Extreme Pro/Claude/phpr-ci/CI_FEED.log")
  case "$last" in START*) echo "PRE-GATE: job CI in corso ($last)"; ko=1;; esac
  local la; la=$(sysctl -n vm.loadavg | awk '{print $2}')
  awk -v l="$la" 'BEGIN{exit !(l<3)}' || { echo "PRE-GATE: loadavg 1m = $la (>=3)"; ko=1; }
  echo "PRE-GATE: bundle=montata lock=$([ -e "$LOCK" ] && echo nostro || echo assente) loadavg=$la"
  return $ko
}
pregate || exit 8
[ "${1:-}" = "--check" ] && exit 0
cd "$REPO" || exit 9
cp "$F" "$ORIG" || exit 9
restore() { cp "$ORIG" "$F"; }
trap 'restore; cmp -s "$ORIG" "$F" && echo "ripristino coerce.rs: al byte" >> "$V" || echo "RIPRISTINO FALLITO coerce.rs" >> "$V"' EXIT
{ echo "# S-182 dev-release gamba EDIT — $(date '+%F %T')"
  echo "HEAD=$(git rev-parse --short HEAD) dirty=$(git status --porcelain | grep -v '^??' | wc -l | tr -d ' ') rustc=$(rustc --version | awk '{print $2}')"; } > "$V"
build() { local l=$1; shift; local s e rc
  s=$(date +%s.%N)
  CARGO_TARGET_DIR="$T" /usr/bin/time -p cargo build "$@" > "$OUT/dre-$l.log" 2> "$OUT/dre-$l.time"; rc=$?
  e=$(date +%s.%N)
  printf '%s wall=%.1f user=%s sys=%s rc=%d\n' "$l" "$(echo "$e - $s" | bc)" "$(awk '/^user/{print $2}' "$OUT/dre-$l.time")" "$(awk '/^sys/{print $2}' "$OUT/dre-$l.time")" "$rc" | tee -a "$V"
  [ $rc -eq 0 ] || { echo "BUILD FALLITA: $l (rc=$rc) — corsa NON giudicabile" | tee -a "$V"; exit 1; }
}
n=0
probe() { n=$((n+1)); restore; printf '\n#[allow(dead_code)]\npub fn __s182_probe_%d() -> u32 { %d }\n' "$n" "$n" >> "$F"; }
for i in 1 2; do
  probe; build "edit-A$i" --release
  probe; build "edit-B$i" --profile dev-release
done
restore
med() { sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}'; }
w() { grep "^$1" "$V" | sed 's/.*wall=\([0-9.]*\).*/\1/'; }
MA=$(w edit-A | med); MB=$(w edit-B | med)
{ echo "mediana_edit A=$MA B=$MB B/A=$(echo "scale=3; $MB / $MA" | bc)"
  echo "soglia: B/A<=0,50 promosso · <=0,80 facoltativo · >0,80 bocciato (E1: giudica la gamba EDIT)"; } | tee -a "$V"
echo "verdetto: $V"
