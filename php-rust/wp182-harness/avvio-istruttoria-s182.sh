#!/bin/bash
# avvio-istruttoria-s182.sh — S-182 ISTRUTTORIA (criterio s182-criterio-istruttoria.md p.3): derivato da s181-lancio-coppia.sh
# con l'attesa della promozione sostituita dall'attesa del DRENAGGIO della CI (la coda arretrata da 172814ae gira PRIMA
# della finestra: il runner legge «harness/s1NN-» come misura viva e i lanciatori leggono cargo/rustc come CI viva ⇒
# senza questo ordine i due si aspettano a vicenda). Il nome NON contiene «harness/s1NN-» né «phpr» di proposito.
# Quando il runner CI è MORTO per 2 controlli a 60 s e non c'è cargo/rustc: legge gli hash degli stash phpr-s181 /
# php-server-s181, li VERIFICA uguali ai binari canonici, li esporta come PIN_ATTESO/SRV_ATTESO e daemonizza
# s182-lancio-pair-t24.sh (→ s182-pair.sh t24) e s182-lancio-orm-t24.sh (→ s176-orm-coppia.sh dopo pair182-t24.done rc=0).
set -u
export PATH=/usr/bin:/bin:/usr/sbin:/opt/homebrew/bin
H="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust/wp182-harness"
STASH="/Volumes/Extreme Pro/Claude/phpr-old-target/release"
DZ="/Volumes/Extreme Pro/Claude/wp52-harness/daemonize.pl"
CIQ="/Volumes/Extreme Pro/Claude/phpr-ci/queue"
LOG="$H/pair-out/avvio-istruttoria.log"; mkdir -p "$H/pair-out" "$H/orm-out"
l(){ echo "$(date '+%F %T') $*" >> "$LOG"; }
ci_viva(){ pgrep -f 'ci/ci-runner.sh' > /dev/null 2>&1 || pgrep -qx cargo || pgrep -qx rustc; }
l "attesa drenaggio CI (runner morto ×2 a 60 s, niente cargo/rustc); coda ora: $(ls -1 "$CIQ" 2>/dev/null | grep -vc '^\._')"
n=0
while [ "$n" -lt 2 ]; do
  if ci_viva; then n=0; else n=$((n+1)); fi
  sleep 60
done
l "CI ferma; coda residua: $(ls -1 "$CIQ" 2>/dev/null | grep -vc '^\._') (se >0: CI stallata, dichiarare nel verbale)"
[ -s "$STASH/phpr-s181" ] && [ -s "$STASH/php-server-s181" ] || { l "stash s181 assenti — istruttoria NON lanciata"; exit 5; }
export PIN_ATTESO=$(shasum -a 256 "$STASH/phpr-s181" | cut -c1-16)
export SRV_ATTESO=$(shasum -a 256 "$STASH/php-server-s181" | cut -c1-16)
PC=$(shasum -a 256 "$HOME/Claude/php-rust-output/release/phpr" | cut -c1-16)
SC=$(shasum -a 256 "$HOME/Claude/php-rust-output/release/php-server" | cut -c1-16)
if [ "$PC" != "$PIN_ATTESO" ] || [ "$SC" != "$SRV_ATTESO" ]; then l "binari canonici ≠ stash s181 (phpr $PC vs $PIN_ATTESO · server $SC vs $SRV_ATTESO) — istruttoria NON lanciata"; exit 9; fi
l "pin s181 phpr=$PIN_ATTESO server=$SRV_ATTESO == canonico — lancio pair t24 + attesa ORM"
perl "$DZ" "$H/pair-out/lancio-t24.log" /bin/bash "$H/s182-lancio-pair-t24.sh"
perl "$DZ" "$H/orm-out/lancio-orm-t24.log" /bin/bash "$H/s182-lancio-orm-t24.sh"
sleep 5
l "lanciati (daemonize): $(pgrep -fl 's182-lancio-(pair|orm)-t24' | grep -v pgrep | awk '{print $1}' | tr '\n' ' ')"
exit 0
