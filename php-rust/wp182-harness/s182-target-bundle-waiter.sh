#!/bin/bash
# s182-target-bundle-waiter.sh — attende che la CI locale abbia svuotato la coda
# (nessun job in phpr-ci/queue, nessun ci-runner/corpus-gate vivo) e che la
# macchina sia quieta per 2 controlli consecutivi (loadavg 1m < 3), poi lancia
# s182-target-bundle-ab.sh (che ha il proprio pre-gate). Detached, log su file.
set -uo pipefail
H="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust/wp182-harness"
Q="/Volumes/Extreme Pro/Claude/phpr-ci/queue"
OUT="$H/target-bundle-out"; mkdir -p "$OUT"
LOG="$OUT/waiter.log"; DONE="$OUT/waiter.done"
rm -f "$DONE"
quiet=0; n=0
while :; do
  n=$((n+1))
  pending=$(ls "$Q" 2>/dev/null | grep -vc '^\._')
  runner=$(pgrep -f 'ci-runner|corpus-gate' | wc -l | tr -d ' ')
  la=$(sysctl -n vm.loadavg | awk '{print $2}')
  if [ "$pending" -eq 0 ] && [ "$runner" -eq 0 ] && awk -v l="$la" 'BEGIN{exit !(l<3)}'; then
    quiet=$((quiet+1))
  else
    quiet=0
  fi
  [ $((n % 10)) -eq 1 ] && echo "$(date '+%F %T') WAIT queue=$pending runner=$runner loadavg=$la quiet=$quiet" >> "$LOG"
  if [ "$quiet" -ge 2 ]; then
    echo "$(date '+%F %T') LAUNCH queue=0 runner=0 loadavg=$la" >> "$LOG"
    "$H/s182-target-bundle-ab.sh" >> "$LOG" 2>&1; rc=$?
    echo "$(date '+%F %T') DONE rc=$rc" >> "$LOG"; echo "$rc" > "$DONE"
    exit $rc
  fi
  sleep 60
done
