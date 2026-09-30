#!/bin/bash
# s182-lancio-scomp.sh — SCOMPOSIZIONE L-CR1 (criterio s182-criterio-scomp.md p.3): DERIVATO da ../wp181-harness/s181-lancio-cr1c.sh
# (manifest s182-lancio-scomp-copia.diff) coi SOLI adattamenti: attende ab-out/s182-scomp.done con `rc=0` (bracci a parità E M2 che
# morde); DUE corse A/B a 3 bracci sullo stesso A = pin s181: scomp1 (Z=P placebo, B=−b) e scomp2 (Z=−a, B=−c) con la banda-placebo
# |D_P| di scomp1 passata a scomp2 (PLAC_BAND letta dal verdetto); bracci come SYMLINK NEUTRI arm-A/arm-Z/arm-B; token s183; PREV
# same-binary prop-dq 31,60; E2 + quiescenza (60 tentativi) + watchdog E3 EREDITATI. rc (ab-out/lancio-scomp.rc): 0 = due corse in
# finestra pulita · 6 = finestra sporca · 7 = bracci/build assenti o rc≠0 · 8 = calma/quiescenza mai PASS · 9 = pre-condizioni.
set -u
export PATH=/usr/bin:/bin:/usr/sbin:/opt/homebrew/bin
SRC="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust"
H="$SRC/wp182-harness"; O="$H/ab-out"; mkdir -p "$O"
LOG="$O/lancio-scomp.log"; LRC="$O/lancio-scomp.rc"; LDONE="$O/lancio-scomp.done"; WDL="$O/watchdog-scomp.log"; ALARM="$O/watchdog-scomp.alarm"
DECL="$O/finestra-scomp.txt"
PIN="$HOME/Claude/php-rust-output/release/phpr"; PIN8=19a2faa8
AB="$O/s182-scomp"
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
BD="$O/s182-scomp.done"
l "attesa $BD con rc=0"
while [ ! -e "$BD" ]; do sleep 60; done
grep -q '^rc=0' "$BD" || fine 7 "build/parità dei bracci rc≠0: $(cat "$BD")"
for a in P MA MB MC; do [ -s "$AB/phpr-$a" ] || fine 7 "braccio assente: $AB/phpr-$a"; done
[ "$(shasum -a 256 "$PIN" | cut -c1-8)" = "$PIN8" ] || fine 9 "pin ≠ $PIN8"
h8(){ shasum -a 256 "$1" | cut -c1-8; }
P8=$(h8 "$AB/phpr-P"); MA8=$(h8 "$AB/phpr-MA"); MB8=$(h8 "$AB/phpr-MB"); MC8=$(h8 "$AB/phpr-MC")
grep -qw s183 /private/tmp/phpr-measure.lock 2>/dev/null || fine 9 "lock senza token s183"
sleep 30
while pgrep -qx cargo || pgrep -qx rustc || pgrep -qf phpt-runner; do sleep 60; done
D0=$(data_g); [ "$D0" -ge 10 ] || fine 9 "Data ${D0}G <10G"
U0=$(updater); [ -z "$U0" ] || fine 9 "updater vivo: $U0"
calm=0; tries=0
while [ "$calm" -lt 4 ]; do
  c=$(ps -Ao %cpu | awk 'NR>1{s+=$1} END{printf "%d", s}')
  if [ "$c" -lt 150 ]; then calm=$((calm+1)); else calm=0; fi
  l "calma CPU tot=${c}% calm=$calm"; tries=$((tries+1))
  [ "$tries" -ge 180 ] && fine 8 "calma CPU mai raggiunta in 90 min"
  [ "$calm" -lt 4 ] && sleep 30
done
Q=0
for i in $(seq 1 60); do
  /bin/bash "$SRC/wp129-harness/s129-quiescenza.sh" "$O/quiesce-scomp.rc" > "$O/quiesce-scomp-$i.log" 2>&1
  if [ "$(cat "$O/quiesce-scomp.rc" 2>/dev/null)" = 0 ]; then Q=$i; break; fi
  l "quiescenza tentativo $i FAIL: $(tail -1 "$O/quiesce-scomp-$i.log")"; sleep 30
done
[ "$Q" -gt 0 ] || fine 8 "quiescenza mai PASS in 60 tentativi"
{ echo "== finestra A/B SCOMPOSIZIONE L-CR1: quiescenza PASS al tentativo $Q, Data ${D0}G, A=$PIN8 P=$P8 −a=$MA8 −b=$MB8 −c=$MC8 =="; sentinelle INIZIO; } > "$DECL"
: > "$WDL"
( prev=0; while :; do d=$(data_g); s=$(updater | head -1)
    echo "$(date '+%T') Data=${d}G${s:+ UPDATER:$s}" >> "$WDL"
    if [ -n "$s" ]; then cur=1; else cur=0; fi
    if [ "$d" -lt 10 ] || { [ "$cur" = 1 ] && [ "$prev" = 1 ]; }; then echo "ALLARME" >> "$WDL"; touch "$ALARM"; fi
    prev=$cur; sleep 30; done ) &
WD=$!
# corsa 1: Z = P (placebo), B = −b
ln -sfn "$PIN" "$O/arm-A"; ln -sfn "$AB/phpr-P" "$O/arm-Z"; ln -sfn "$AB/phpr-MB" "$O/arm-B"
l "finestra aperta (quiescenza t$Q, Data ${D0}G) — corsa scomp1: Z=P($P8) B=−b($MB8) R=5"
ZLAB=P BLAB=-b ZATT="-2;2" BATT="3;5" PLAC_ARM=Z /bin/bash "$H/s182-ab-scomp.sh" "$O/arm-A" "$PIN8" "$O/arm-Z" "$P8" "$O/arm-B" "$MB8" scomp1 5 31.60
R1=$?
PB=$(sed -n 's/.*banda-placebo = \([0-9.]*\) ns\/iter.*/\1/p' "$H/s182-scomp1-verdetto.out" | head -1); PB="${PB:-0}"
l "scomp1 rc=$R1 — banda-placebo letta: $PB"
# corsa 2: Z = −a, B = −c (banda-placebo di scomp1 come soglia)
ln -sfn "$AB/phpr-MA" "$O/arm-Z"; ln -sfn "$AB/phpr-MC" "$O/arm-B"
l "corsa scomp2: Z=−a($MA8) B=−c($MC8) R=5 PLAC_BAND=$PB"
ZLAB=-a BLAB=-c ZATT="0;2" BATT="0;1" PLAC_ARM= PLAC_BAND="$PB" /bin/bash "$H/s182-ab-scomp.sh" "$O/arm-A" "$PIN8" "$O/arm-Z" "$MA8" "$O/arm-B" "$MC8" scomp2 5 31.60
R2=$?
kill "$WD" 2>/dev/null; wait "$WD" 2>/dev/null
{ sentinelle FINE; echo "watchdog: $(wc -l < "$WDL" | tr -d ' ') campioni, Data min=$(sed -n 's/.*Data=\([0-9]*\)G.*/\1/p' "$WDL" | sort -n | head -1)G, campioni con updater=$(grep -c UPDATER "$WDL"), allarmi=$(grep -c ALLARME "$WDL")"; } >> "$DECL"
for v in scomp1 scomp2; do cat "$DECL" >> "$H/s182-$v-verdetto.out" 2>/dev/null; done
l "scomp1 rc=$R1 · scomp2 rc=$R2"
[ -e "$ALARM" ] && fine 6 "finestra SPORCA (watchdog): verdetti a GUARDIA"
fine 0 "due corse in finestra pulita; rc criterio scomp1=$R1 scomp2=$R2"
