#!/bin/bash
# s182-lancio-orm-t24.sh — ISTRUTTORIA S-182: attende pair182-t24.done; SOLO se rc=0
# lancia s176-orm-coppia.sh (MAPPA_SP dedicato APFS). Un pair fallito NON
# fa partire l'ORM: la sessione istruisce prima.
# COPIA DICHIARATA di s181-lancio-orm-t23.sh (manifest s182-lancio-orm-copia.diff): soli nomi s182/t24/pair182.
set -u
export PATH=/usr/bin:/bin:/usr/sbin:/opt/homebrew/bin
REPO="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust"; H="$REPO/wp182-harness"
LOG="$H/orm-out/lancio-orm-t24.log"; mkdir -p "$H/orm-out"
PD="$H/pair-out/pair182-t24.done"
echo "$(date '+%F %T') attesa $PD" >> "$LOG"
while [ ! -e "$PD" ]; do sleep 120; done
if ! grep -q '^rc=0' "$PD"; then
  echo "$(date '+%F %T') pair t24 NON rc=0 ($(cat "$PD")) — ORM NON lanciato" >> "$LOG"
  exit 5
fi
sleep 60
SPD=/private/tmp/phpr-s182-orm; mkdir -p "$SPD"
echo "$(date '+%F %T') pair rc=0 — lancio ORM (MAPPA_SP=$SPD)" >> "$LOG"
PIN_ATTESO="${PIN_ATTESO:?pin s181 atteso (istruttoria S-182)}" MAPPA_SP="$SPD" exec /bin/bash "$REPO/wp176-harness/s176-orm-coppia.sh"
