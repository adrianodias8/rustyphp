#!/bin/bash
# s182-target-bundle-waiter.sh — MODO LOCK (decisione utente 2026-09-29, per
# accorciare l'attesa della coda CI): prende /private/tmp/phpr-measure.lock col
# token s182-target-bundle (il ci-runner si ferma in quiet_wait PRIMA del job
# successivo, il job in corso finisce da solo), aspetta che il job in corso sia
# chiuso (CI_FEED: ultima riga non START, nessun cargo/corpus-gate/phpt-runner)
# e la macchina quieta per 2 controlli (loadavg 1m < 3), lancia
# lo script A/B ($1, default s182-target-bundle-ab.sh; log $2.log) e RILASCIA il lock in ogni caso (trap), così la CI
# riparte. Detached, log su file. Nota: il nome del file combacia col pattern
# `harness/s1NN-` del runner: doppia sicurezza, il runner resta in pausa finché
# questo processo vive (timeout runner 4h >> durata misura).
set -uo pipefail
H="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust/wp182-harness"
FEED="/Volumes/Extreme Pro/Claude/phpr-ci/CI_FEED.log"
LOCK=/private/tmp/phpr-measure.lock
OUT="$H/target-bundle-out"; mkdir -p "$OUT"
AB="${1:-$H/s182-target-bundle-ab.sh}"; N="${2:-waiter}"
LOG="$OUT/$N.log"; DONE="$OUT/$N.done"
rm -f "$DONE"
if [ -e "$LOCK" ]; then echo "$(date '+%F %T') ABORT lock altrui: $(cat "$LOCK")" >> "$LOG"; echo 9 > "$DONE"; exit 9; fi
echo "s182-target-bundle pid=$$ $(date '+%F %T')" > "$LOCK"
trap 'rm -f "$LOCK"; echo "$(date "+%F %T") LOCK rilasciato" >> "$LOG"' EXIT
echo "$(date '+%F %T') LOCK preso ($LOCK)" >> "$LOG"
quiet=0; n=0
while :; do
  n=$((n+1))
  last=$(tail -1 "$FEED"); job=0; case "$last" in START*) job=1;; esac
  busy=$(pgrep -f 'corpus-gate|phpt-runner|cargo ' | wc -l | tr -d ' ')
  la=$(sysctl -n vm.loadavg | awk '{print $2}')
  if [ "$job" -eq 0 ] && [ "$busy" -eq 0 ] && awk -v l="$la" 'BEGIN{exit !(l<3)}'; then quiet=$((quiet+1)); else quiet=0; fi
  [ $((n % 5)) -eq 1 ] && echo "$(date '+%F %T') WAIT job=$job busy=$busy loadavg=$la quiet=$quiet feed='${last:0:40}'" >> "$LOG"
  if [ "$quiet" -ge 2 ]; then
    echo "$(date '+%F %T') LAUNCH job=0 busy=0 loadavg=$la" >> "$LOG"
    "$AB" >> "$LOG" 2>&1; rc=$?
    echo "$(date '+%F %T') DONE rc=$rc" >> "$LOG"; echo "$rc" > "$DONE"
    exit $rc
  fi
  sleep 60
done
