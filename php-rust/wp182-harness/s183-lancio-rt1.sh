#!/bin/bash
# s183-lancio-rt1.sh — LEVA L-RT1 (criterio s183-criterio-rt1.md p.3): COPIA DICHIARATA di s182-lancio-scomp.sh (manifest
# s183-lancio-rt1-copia.diff): UNA corsa a 3 bracci A=pin s181 / Z=gemello (ab-out/s183-leva/phpr-Z) / B=leva (phpr-B) con
# s183-ab-rt1.sh (tag rt1, R=5, PREV 31,60); attende ab-out/s183-leva.done rc=0; attese ambientali senza limite EREDITATE. Testo sotto:
# ---- derivato da ../wp181-harness/s181-lancio-cr1c.sh
# (manifest s182-lancio-scomp-copia.diff) coi SOLI adattamenti: attende ab-out/s182-scomp.done con `rc=0` (bracci a parità E M2 che
# morde); DUE corse A/B a 3 bracci sullo stesso A = pin s181: scomp1 (Z=P placebo, B=−b) e scomp2 (Z=−a, B=−c) con la banda-placebo
# |D_P| di scomp1 passata a scomp2 (PLAC_BAND letta dal verdetto); bracci come SYMLINK NEUTRI arm-A/arm-Z/arm-B; token s183; PREV
# same-binary prop-dq 31,60; E2 + quiescenza (60 tentativi) + watchdog E3 EREDITATI. rc (ab-out/lancio-scomp.rc): 0 = due corse in
# finestra pulita · 6 = finestra sporca · 7 = bracci/build assenti o rc≠0 · 9 = pre-condizioni (pin/lock/Data).
# EMENDA S-183 (dichiarata dopo il rc=9 delle 09:38 «updater vivo: Littlebird»): le pre-condizioni AMBIENTALI (updater/app utente,
# calma CPU, quiescenza) si ASPETTANO senza limite di tentativi, con log ogni campione — il lanciatore vive finché la macchina
# non è quieta (stesso modello dei gate del lanciatore pair t24); i gate restano INVARIATI nella sostanza.
set -u
export PATH=/usr/bin:/bin:/usr/sbin:/opt/homebrew/bin
SRC="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust"
H="$SRC/wp182-harness"; O="$H/ab-out"; mkdir -p "$O"
LOG="$O/lancio-rt1.log"; LRC="$O/lancio-rt1.rc"; LDONE="$O/lancio-rt1.done"; WDL="$O/watchdog-rt1.log"; ALARM="$O/watchdog-rt1.alarm"
DECL="$O/finestra-rt1.txt"
PIN="$HOME/Claude/php-rust-output/release/phpr"; PIN8=19a2faa8
AB="$O/s183-leva"
rm -f "$LRC" "$LDONE" "$ALARM"
l(){ echo "$(date '+%F %T') $*" >> "$LOG"; }
fine(){ l "fine rc=$1 ($2)"; echo "$1" > "$LRC"; echo "rc=$1" > "$LDONE"; exit "$1"; }
data_g(){ df -g /System/Volumes/Data | awk 'NR==2{print $4}'; }
updater(){ pgrep -fl -i "ShipIt|littlebird|keystone|GoogleUpdater" | head -3; }
sentinelle(){
  echo "-- sentinelle $1 $(date '+%F %T'): Data=$(data_g)G · $(sysctl -n vm.swapusage) · $(uptime | sed 's/.*load/load/')"
  echo "   updater: $(updater | tr '\n' ' ')"
  echo "   top-8 CPU:"; ps -Ao %cpu=,comm= -r | head -8 | awk '{printf "     %5.1f %s\n",$1,$2}'
}
BD="$O/s183-leva.done"
l "attesa $BD con rc=0"
while [ ! -e "$BD" ]; do sleep 60; done
grep -q '^rc=0' "$BD" || fine 7 "build/parità dei bracci rc≠0: $(cat "$BD")"
for a in Z B; do [ -s "$AB/phpr-$a" ] || fine 7 "braccio assente: $AB/phpr-$a"; done
[ "$(shasum -a 256 "$PIN" | cut -c1-8)" = "$PIN8" ] || fine 9 "pin ≠ $PIN8"
h8(){ shasum -a 256 "$1" | cut -c1-8; }
Z8=$(h8 "$AB/phpr-Z"); B8=$(h8 "$AB/phpr-B"); [ "$B8" != "$PIN8" ] || fine 9 "B == pin"
grep -qw s183 /private/tmp/phpr-measure.lock 2>/dev/null || fine 9 "lock senza token s183"
sleep 30
while pgrep -qx cargo || pgrep -qx rustc || pgrep -qf phpt-runner; do sleep 60; done
D0=$(data_g); [ "$D0" -ge 10 ] || fine 9 "Data ${D0}G <10G"
uc=0
while [ "$uc" -lt 2 ]; do U0=$(updater | head -1); if [ -z "$U0" ]; then uc=$((uc+1)); else uc=0; l "updater/app utente vivo: $U0 — attesa"; fi; [ "$uc" -lt 2 ] && sleep 60; done
calm=0; tries=0
while [ "$calm" -lt 4 ]; do
  c=$(ps -Ao %cpu | awk 'NR>1{s+=$1} END{printf "%d", s}')
  if [ "$c" -lt 150 ]; then calm=$((calm+1)); else calm=0; fi
  l "calma CPU tot=${c}% calm=$calm"; tries=$((tries+1))
  [ "$calm" -lt 4 ] && sleep 30
done
Q=0
i=0
while :; do
  i=$((i+1)); /bin/bash "$SRC/wp129-harness/s129-quiescenza.sh" "$O/quiesce-rt1.rc" > "$O/quiesce-rt1-$i.log" 2>&1
  if [ "$(cat "$O/quiesce-rt1.rc" 2>/dev/null)" = 0 ]; then Q=$i; break; fi
  l "quiescenza tentativo $i FAIL: $(tail -1 "$O/quiesce-rt1-$i.log")"; sleep 60
done
{ echo "== finestra A/B L-RT1: quiescenza PASS al tentativo $Q, Data ${D0}G, bracci A=$PIN8 Z=$Z8 B=$B8 =="; sentinelle INIZIO; } > "$DECL"
: > "$WDL"
( prev=0; while :; do d=$(data_g); s=$(updater | head -1)
    echo "$(date '+%T') Data=${d}G${s:+ UPDATER:$s}" >> "$WDL"
    if [ -n "$s" ]; then cur=1; else cur=0; fi
    if [ "$d" -lt 10 ] || { [ "$cur" = 1 ] && [ "$prev" = 1 ]; }; then echo "ALLARME" >> "$WDL"; touch "$ALARM"; fi
    prev=$cur; sleep 30; done ) &
WD=$!
ln -sfn "$PIN" "$O/arm-A"; ln -sfn "$AB/phpr-Z" "$O/arm-Z"; ln -sfn "$AB/phpr-B" "$O/arm-B"
l "finestra aperta (quiescenza t$Q, Data ${D0}G, B=$B8) — s183-ab-rt1.sh rt1 R=5"
/bin/bash "$H/s183-ab-rt1.sh" "$O/arm-A" "$PIN8" "$O/arm-Z" "$Z8" "$O/arm-B" "$B8" rt1 5 31.60
R1=$?
kill "$WD" 2>/dev/null; wait "$WD" 2>/dev/null
{ sentinelle FINE; echo "watchdog: $(wc -l < "$WDL" | tr -d ' ') campioni, Data min=$(sed -n 's/.*Data=\([0-9]*\)G.*/\1/p' "$WDL" | sort -n | head -1)G, campioni con updater=$(grep -c UPDATER "$WDL"), allarmi=$(grep -c ALLARME "$WDL")"; } >> "$DECL"
cat "$DECL" >> "$H/s183-rt1-verdetto.out" 2>/dev/null
l "s183-ab-rt1.sh rc=$R1"
[ -e "$ALARM" ] && fine 6 "finestra SPORCA (watchdog): verdetti a GUARDIA"
fine 0 "A/B eseguito in finestra pulita; criterio rc=$R1"
