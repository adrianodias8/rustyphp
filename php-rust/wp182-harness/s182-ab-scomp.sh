#!/bin/bash
# s182-ab-scomp.sh — SCOMPOSIZIONE di L-CR1 (criterio s182-criterio-scomp.md p.3-5): COPIA DICHIARATA di
# ../wp181-harness/s181-ab-cr1.sh (manifest s182-ab-scomp-copia.diff) coi SOLI adattamenti: (1) A = pin s181 canonico
# (19a2faa8), Z e B = DUE BRACCI della scomposizione per corsa (etichette ZLAB/BLAB: P, -a, -b, -c), NIENTE gemello;
# (2) segno D_X = X − A (positivo = braccio più lento del pin = costo del componente tolto; P = layout puro) su calls-dq
# (bersaglio) E su prop-dq/arith-dq (guardie) con ESITO ESPLICITO dell'attesa |D|<1 per OGNI braccio (rilievo 1 S-181);
# (3) attese per braccio pre-registrate (ZATT/BATT = "lo;hi" su calls-dq) e giudizio NOMINATO/DIREZIONE/NULLO con
# soglia max(4, rumore, banda-placebo) — banda-placebo = |D_P| in-run se PLAC_ARM ∈ {Z,B}, altrimenti PLAC_BAND (dalla
# corsa col placebo); (4) esiti: rc=0 verdetto EMESSO (nessuna promozione/revert: il pin resta) · rc=8 E1 (|A−PREV|
# prop-dq > 4, PREV 31,60 pin s181 S-181; calls-dq PREV 90,50 sola lettura) · rc=2 parità · rc=9 lock TOKEN s182 · rc=7 file;
# (5) verdetto s182-<TAG>-verdetto.out. Tutto il resto INVARIATO (R=5, rotazione dei bracci per coppia, floors med3 per
# binario, ns/iter=(med raw−floor)/N con N LETTO dal driver, mediane per colonna e appaiate, rumore drop-1, parità dei
# bracci contro l'ORACLE prima di misurare, bracci come SYMLINK NEUTRI arm-A/arm-Z/arm-B).
# Uso: ZLAB=P BLAB=-b ZATT="-2;2" BATT="3;5" PLAC_ARM=Z [PLAC_BAND=0] s182-ab-scomp.sh <APATH> <AEXP8> <ZPATH> <ZEXP8> <BPATH> <BEXP8> <TAG> [R] [PREV_PROP_A]
set -u
export PATH=/usr/bin:/bin:/usr/sbin:/opt/homebrew/bin
H="$(cd -P "$(dirname -- "$0")" && pwd -P)"
A="${1:?APATH}"; AEXP="${2:?AEXP8}"; ZZ="${3:?ZPATH}"; ZEXP="${4:?ZEXP8}"; BB="${5:?BPATH}"; BEXP="${6:?BEXP8}"; TAG="${7:?TAG}"; R="${8:-5}"; PREVPD="${9:-31.60}"; ZLAB="${ZLAB:?ZLAB}"; BLAB="${BLAB:?BLAB}"; ZATT="${ZATT:?ZATT lo;hi}"; BATT="${BATT:?BATT lo;hi}"; PLAC_ARM="${PLAC_ARM:-}"; PLAC_BAND="${PLAC_BAND:-0}"
O=/opt/homebrew/opt/php/bin/php
CD="$H/calls-dq.php"
GD="$H/../wp164-harness/arith-dq.php"
PD="$H/../wp172-harness/prop-dq.php"
EMPTY="$H/../wp160-harness/empty.php"
OUT="$H/ab-out"; mkdir -p "$OUT"
VERD="$H/s182-$TAG-verdetto.out"; RC="$OUT/$TAG.rc"
[ -e "$VERD" ] && { echo "verdetto ESISTE — TAG nuovo" >&2; exit 7; }
for f in "$CD" "$PD" "$GD" "$A" "$ZZ" "$BB" "$O"; do [ -s "$f" ] || { echo "file assente o VUOTO: $f" | tee -a "$VERD"; echo 7 > "$RC"; exit 7; }; done
[ -e "$EMPTY" ] || { echo "driver del pavimento assente: $EMPTY (VUOTO per costruzione: [ -e ], emenda S-170 p.4)" | tee -a "$VERD"; echo 7 > "$RC"; exit 7; }
grep -qw s182 /private/tmp/phpr-measure.lock 2>/dev/null || { echo "lock s182 assente (per TOKEN)" | tee -a "$VERD"; echo 9 > "$RC"; exit 9; }
"$H/../wp129-harness/s129-quiescenza.sh" "$OUT/quiesce-$TAG.rc" > /dev/null 2>&1 || { echo "quiescenza FAIL" | tee -a "$VERD"; echo 8 > "$RC"; exit 8; }
AM="$(shasum -a 256 "$A" | cut -c1-8)"; ZM="$(shasum -a 256 "$ZZ" | cut -c1-8)"; BM="$(shasum -a 256 "$BB" | cut -c1-8)"
[ "$AM" = "$AEXP" ] || { echo "A misurato $AM != atteso $AEXP" | tee -a "$VERD"; echo 1 > "$RC"; exit 1; }
[ "$ZM" = "$ZEXP" ] || { echo "Z misurato $ZM != atteso $ZEXP" | tee -a "$VERD"; echo 1 > "$RC"; exit 1; }
[ "$BM" = "$BEXP" ] || { echo "B misurato $BM != atteso $BEXP" | tee -a "$VERD"; echo 1 > "$RC"; exit 1; }
# N del giudice e delle guardie EMESSI dal sorgente (KS-GR-105-2; az.rev. S-170 #4): mai cablati
NCD=$(awk 'match($0, /\$i<[0-9]+/) {print substr($0, RSTART+3, RLENGTH-3); exit}' "$CD")
NGD=$(awk 'match($0, /\$i<[0-9]+/) {print substr($0, RSTART+3, RLENGTH-3); exit}' "$GD")
NPD=$(awk 'match($0, /\$i<[0-9]+/) {print substr($0, RSTART+3, RLENGTH-3); exit}' "$PD")
[ -n "$NCD" ] && [ -n "$NPD" ] && [ -n "$NGD" ] || { echo "N non leggibile dal driver (calls='$NCD' dq='$NGD' prop='$NPD')" | tee -a "$VERD"; echo 7 > "$RC"; exit 7; }
ucpu(){ { /usr/bin/time -p perl -e 'alarm 900; exec @ARGV or die' -- "$@" > /dev/null; } 2>&1 | awk '/^user/{print $2}'; }
floor3(){ local a b c; a=$(ucpu "$@" "$EMPTY"); b=$(ucpu "$@" "$EMPTY"); c=$(ucpu "$@" "$EMPTY"); printf '%s\n%s\n%s\n' "$a" "$b" "$c" | sort -n | awk 'NR==2'; }
{
echo "== s182 SCOMPOSIZIONE L-CR1 $TAG — A=$AM MISURATO ($A, pin s181) Z=$ZM MISURATO ($ZZ, braccio $ZLAB, attesa calls-dq [$ZATT]) B=$BM MISURATO ($BB, braccio $BLAB, attesa calls-dq [$BATT]) — R=$R, N: calls-dq=$NCD arith-dq=$NGD prop-dq=$NPD, PREV prop-dq A (pin s181, S-181): $PREVPD, placebo: PLAC_ARM=${PLAC_ARM:-nessuno} PLAC_BAND=$PLAC_BAND, $(date '+%F %T') =="
echo "sentinella LS: $(pgrep -fl 'rust-analyzer|Antigravity|serena' 2>/dev/null | grep -v pgrep | awk '{print $2}' | sort -u | tr '\n' ' ')"
# PARITÀ vs ATTESO: l'atteso è l'output dell'ORACLE sullo stesso driver (generato qui, una volta per driver, `[ -s ]`).
for D in "$CD" "$GD" "$PD"; do
  n="$(basename "$D" .php)"; EXP="$OUT/expected-$n.out"
  [ -s "$EXP" ] || "$O" "$D" > "$EXP" 2>&1
  [ -s "$EXP" ] || { echo "atteso VUOTO per $n — STOP"; echo 2 > "$RC"; exit 2; }
  for arm in A Z B; do
    case "$arm" in A) bin="$A";; Z) bin="$ZZ";; B) bin="$BB";; esac
    "$bin" "$D" > "$OUT/$TAG-$arm-$n.out" 2>&1
    cmp -s "$OUT/$TAG-$arm-$n.out" "$EXP" || { echo "output $arm ≠ ATTESO oracle su $n ($(head -c 60 "$OUT/$TAG-$arm-$n.out" | tr '\n' ' ')) — STOP"; echo 2 > "$RC"; exit 2; }
  done
  echo "parità $n: A==Z==B==atteso oracle ($(tr '\n' ' ' < "$EXP"))"
done
FA=$(floor3 "$A"); FZ=$(floor3 "$ZZ"); FB=$(floor3 "$BB"); FO=$(floor3 "$O")
echo "floors: A=$FA Z=$FZ B=$FB oracle=$FO"
TSV="$OUT/$TAG-runs.tsv"; : > "$TSV"
ORDS="AZB BAZ ZBA ABZ BZA ZAB"
for i in $(seq 1 "$R"); do
  ord=$(echo "$ORDS" | awk -v k="$i" '{print $(((k-1)%6)+1)}')
  CA=; CZ=; CB=; PA=; PZ=; PB=; GA=; GZ=; GB=
  for x in $(echo "$ord" | sed 's/./& /g'); do
    case "$x" in
      A) CA=$(ucpu "$A" "$CD"); PA=$(ucpu "$A" "$PD"); GA=$(ucpu "$A" "$GD");;
      Z) CZ=$(ucpu "$ZZ" "$CD"); PZ=$(ucpu "$ZZ" "$PD"); GZ=$(ucpu "$ZZ" "$GD");;
      B) CB=$(ucpu "$BB" "$CD"); PB=$(ucpu "$BB" "$PD"); GB=$(ucpu "$BB" "$GD");;
    esac
  done
  COR=$(ucpu "$O" "$CD"); POR=$(ucpu "$O" "$PD"); GOR=$(ucpu "$O" "$GD")
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$CA" "$CZ" "$CB" "$PA" "$PZ" "$PB" "$GA" "$GZ" "$GB" "$COR" "$POR" "$GOR" >> "$TSV"
  echo "  coppia$i [$ord]: callsA=$CA callsZ=$CZ callsB=$CB propA=$PA propZ=$PZ propB=$PB dqA=$GA dqZ=$GZ dqB=$GB | callsO=$COR propO=$POR dqO=$GOR"
done
python3 - "$TSV" "$FA" "$FZ" "$FB" "$FO" "$NCD" "$NPD" "$NGD" "$PREVPD" "$R" "$ZLAB" "$BLAB" "$ZATT" "$BATT" "$PLAC_ARM" "$PLAC_BAND" <<'PY'
import sys
rows = [l.split() for l in open(sys.argv[1])]
fa, fz, fb, fo = map(float, sys.argv[2:6]); ncd = float(sys.argv[6]); npd = float(sys.argv[7]); ngd = float(sys.argv[8]); prevpd = float(sys.argv[9]); R = int(sys.argv[10])
ZLAB, BLAB = sys.argv[11], sys.argv[12]
def att(x): lo, hi = x.split(';'); return float(lo), float(hi)
ZATT, BATT = att(sys.argv[13]), att(sys.argv[14]); PLAC_ARM = sys.argv[15]; PLAC_BAND = float(sys.argv[16])
def col(i, f, n): return [(float(r[i])-f)/n*1e9 for r in rows]
def med(v):
    v = sorted(v); k = len(v); return v[k//2] if k % 2 else (v[k//2-1]+v[k//2])/2
def dr1(v):
    m = med(v); w = sorted(v, key=lambda x:(abs(x-m),x))[:-1]; return max(w)-min(w)
def pmed(X, Y): return med([x-y for x, y in zip(X, Y)])   # mediana delle differenze APPAIATE
def signs(X, Y): return sum(1 for x, y in zip(X, Y) if x - y > 0)
cA, cZ, cB, cO = col(0,fa,ncd), col(1,fz,ncd), col(2,fb,ncd), col(9,fo,ncd)
pA, pZ, pB, pO = col(3,fa,npd), col(4,fz,npd), col(5,fb,npd), col(10,fo,npd)
gA, gZ, gB, gO = col(6,fa,ngd), col(7,fz,ngd), col(8,fb,ngd), col(11,fo,ngd)
rc = 0
print(f"A (pin s181): calls-dq={med(cA):.2f} (PREV 90.50 sola lettura, |A−PREV|={abs(med(cA)-90.50):.2f}) prop-dq={med(pA):.2f} (PREV {prevpd:.2f}, |A−PREV|={abs(med(pA)-prevpd):.2f}) arith-dq={med(gA):.2f}; rumore drop-1 A: calls {dr1(cA):.2f} prop {dr1(pA):.2f} arith {dr1(gA):.2f}")
print(f"oracle: calls-dq={med(cO):.2f} prop-dq={med(pO):.2f} arith-dq={med(gO):.2f} ns/iter; A/oracle calls {med(cA)/med(cO):.2f}× prop {med(pA)/med(pO):.2f}× arith {med(gA)/med(gO):.2f}×")
BANDp = abs(med(pA)-prevpd)
if BANDp > 4.0:
    print(f"BANDA same-binary FUORI: |A−PREV| prop-dq {BANDp:.2f} > 4 ⇒ finestra CONTAMINATA (pesi esterni): nessun verdetto (rc=8, emenda E1 S-177)"); rc = 8
arms = [("Z", ZLAB, cZ, pZ, gZ, ZATT), ("B", BLAB, cB, pB, gB, BATT)]
# banda-placebo: |D_P| in-run su calls-dq (colonna E appaiata, il max) se il placebo è in corsa; altrimenti quella passata
plac = PLAC_BAND
for arm, lab, cX, pX, gX, a in arms:
    if arm == PLAC_ARM:
        plac = max(abs(med(cX)-med(cA)), abs(pmed(cX, cA)))
        print(f"PLACEBO {lab} (braccio {arm}): D_P calls-dq colonna={med(cX)-med(cA):+.2f} appaiata={pmed(cX,cA):+.2f} ⇒ banda-placebo = {plac:.2f} ns/iter ({'NOMINATA: la fortuna di layout del candidato vale come SL per gli altri bracci' if plac >= 2 else 'sotto 2: layout del candidato non nominato'})")
for arm, lab, cX, pX, gX, a in arms:
    Dc = med(cX)-med(cA); Dp = pmed(cX, cA); noise = max(dr1(cA), dr1(cX)); sg = signs(cX, cA)
    thr = max(4.0, noise, plac if arm != PLAC_ARM else 0.0)
    lo, hi = a; cent = "CENTRATA" if (lo <= Dc <= hi and lo <= Dp <= hi) else "FUORI"
    if arm == PLAC_ARM:
        giud = "placebo (nessun componente)"
    elif Dc >= thr and Dp >= thr: giud = f"NOMINATO: CIFRA [{min(Dc,Dp):+.2f};{max(Dc,Dp):+.2f}]"
    elif min(Dc, Dp) >= max(noise, plac) and sg == R: giud = "DIREZIONE (sotto la soglia, segni pieni)"
    elif min(Dc, Dp) <= -max(4.0, noise): giud = "NEGATIVO NOMINATO: il braccio senza il componente è PIÙ VELOCE del pin (il componente COSTA)"
    else: giud = "NULLO (vale 0)"
    print(f"GIUDICE calls-dq braccio {lab} ({arm}): A={med(cA):.2f} {lab}={med(cX):.2f} ns/iter D={lab}−A colonna={Dc:+.2f} appaiata={Dp:+.2f} soglia={thr:.2f} (rumore drop-1 {noise:.2f}, banda-placebo {plac if arm != PLAC_ARM else 0.0:.2f}) segni {sg}/{R} -> {giud}; attesa [{lo:+.1f};{hi:+.1f}]: {cent}")
    for jn, X, Aj in (("prop-dq", pX, pA), ("arith-dq", gX, gA)):
        Dc2 = med(X)-med(Aj); Dp2 = pmed(X, Aj); n2 = max(dr1(Aj), dr1(X)); sg2 = signs(X, Aj)
        esito = "RISPETTATA" if max(abs(Dc2), abs(Dp2)) < 1.0 else ("VIOLATA — segni pieni: effetto TRASVERSALE firmato su questo braccio" if sg2 in (0, R) else "VIOLATA — segni misti: rumore/layout")
        print(f"GUARDIA {jn} braccio {lab} ({arm}): A={med(Aj):.2f} {lab}={med(X):.2f} D={lab}−A colonna={Dc2:+.2f} appaiata={Dp2:+.2f} (rumore {n2:.2f}) segni {sg2}/{R} -> attesa |D|<1: {esito}")
print(f"PREV per la prossima misura (same-binary pin s181): calls-dq A={med(cA):.2f} prop-dq A={med(pA):.2f} arith-dq A={med(gA):.2f}")
print(f"ESITO rc={rc} ({'verdetto EMESSO: nessuna promozione/revert deriva da qui (criterio-scomp p.5)' if rc == 0 else 'finestra contaminata'})")
sys.exit(rc)
PY
prc=$?
echo "sentinella LS fine: $(pgrep -fl 'rust-analyzer|Antigravity|serena' 2>/dev/null | grep -v pgrep | awk '{print $2}' | sort -u | tr '\n' ' ')"
echo "$prc" > "$RC"; exit "$prc"
} >> "$VERD" 2>&1
