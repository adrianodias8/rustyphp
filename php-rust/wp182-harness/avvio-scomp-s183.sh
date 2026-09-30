#!/bin/bash
# avvio-scomp-s183.sh — S-183: derivato da avvio-istruttoria-s182.sh. Attende il DRENAGGIO della CI (runner morto ×2 a 60 s,
# niente cargo/rustc), poi SCRIVE il lock col TOKEN s183 (finestra A/B della scomposizione) e daemonizza s182-lancio-scomp.sh
# (che aspetta da sé updater/app utente, calma CPU, quiescenza). Nome senza «harness/s1NN-» né «phpr» di proposito.
set -u
export PATH=/usr/bin:/bin:/usr/sbin:/opt/homebrew/bin
H="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust/wp182-harness"
DZ="/Volumes/Extreme Pro/Claude/wp52-harness/daemonize.pl"
CIQ="/Volumes/Extreme Pro/Claude/phpr-ci/queue"
LOG="$H/ab-out/avvio-scomp.log"; mkdir -p "$H/ab-out"
l(){ echo "$(date '+%F %T') $*" >> "$LOG"; }
ci_viva(){ pgrep -f 'ci/ci-runner.sh' > /dev/null 2>&1 || pgrep -qx cargo || pgrep -qx rustc; }
l "attesa drenaggio CI (runner morto ×2 a 60 s, niente cargo/rustc); coda ora: $(ls -1 "$CIQ" 2>/dev/null | grep -vc '^\._')"
n=0
while [ "$n" -lt 2 ]; do if ci_viva; then n=0; else n=$((n+1)); fi; sleep 60; done
l "CI ferma; coda residua: $(ls -1 "$CIQ" 2>/dev/null | grep -vc '^\._')"
[ -s "$H/ab-out/s182-scomp.done" ] && grep -q '^rc=0' "$H/ab-out/s182-scomp.done" || { l "bracci non pronti — non lancio"; exit 7; }
echo "s183 scomposizione L-CR1 $(date '+%F %T') pid=$$" > /private/tmp/phpr-measure.lock
l "lock scritto (TOKEN s183) — lancio s182-lancio-scomp.sh"
perl "$DZ" "$H/ab-out/lancio-scomp-dz.log" /bin/bash "$H/s182-lancio-scomp.sh"
sleep 3; l "lanciato: $(ps -Ao pid,command | grep '[s]182-lancio-scomp' | awk '{print $1}' | tr '\n' ' ')"
exit 0
