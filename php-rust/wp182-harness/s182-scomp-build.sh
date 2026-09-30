#!/bin/bash
# s182-scomp-build.sh — bracci della SCOMPOSIZIONE di L-CR1 (criterio s182-criterio-scomp.md p.2): DERIVATO da
# ../wp181-harness/s181-leva-build.sh (manifest s182-scomp-build-copia.diff) coi SOLI adattamenti: (1) UNA sorgente
# (BASE = commit del pin s181, default f5f746cf) archiviata di nuovo per OGNI braccio; bracci = PATCH python ancorate con
# ASSERT di unicità sull'archivio (mai sul tree, mai sed globale — lezione S-181 cr1b): Z = base (atteso byte-id col pin
# 19a2faa83a492745; se ≠ = «gemello a contenuto», dichiarato), P = placebo (`cf.argc` dopo il ciclo), MA = −a, MB = −b,
# MC = −c, M2 = mutante «ip=1 anche sul cammino LENTO di MethodCall» (dispatch_instance_call, ENTRAMBI i siti di bind);
# (2) pin atteso s181, lock TOKEN s183 (esecuzione in S-183), out ab-out/s182-scomp, sorgente /Volumes/Extreme Pro/Claude/s182-scomp, TARGET sulla
# sparsebundle ~/Claude/phpr-target/s182-scomp-tgt (montata: gate nel pre-flight; mai ExFAT, mai la canonica);
# (3) parità di P/MA/MB/MC: fx-sw2-gc == pin a blocchi, fx-sw1/sl1/sl2/sl3/cr1 == oracle byte-id; M2 DEVE divergere
# dall'oracle su fx-cr1 ESATTAMENTE sulle righe «ACE m» (sparite solo righe ACE m; in più solo warning/vuote);
# (4) disasm run_loop (istr/bl/blr/sp_refs) del pin e di ogni braccio; (5) SKIP_BUILD=1 riusa i binari già costruiti.
# rc (ab-out/s182-scomp.done): 0 = bracci a parità e M2 che morde · 2 = un braccio diverge · 3 = M2 non morde ·
# 4 = build fallita · 5 = P byte-id col pin (placebo NULLO) · 7 = file/commit/patch · 8 = disco · 9 = lock/pin.
set -u
export PATH=/usr/bin:/bin:/usr/sbin:/opt/homebrew/bin:"$HOME/.cargo/bin"
REPO="/Volumes/Extreme Pro/Claude/php-rust-experiment"
H="$REPO/php-rust/wp182-harness"; OUT="$H/ab-out/s182-scomp"; mkdir -p "$OUT"
VERD="$H/s182-scomp-build-verdetto.out"; DONE="$H/ab-out/s182-scomp.done"; rm -f "$DONE"
LOCK=/private/tmp/phpr-measure.lock
PIN="$HOME/Claude/php-rust-output/release/phpr"; PIN_ATTESO="19a2faa83a492745"
ORACLE=/opt/homebrew/opt/php/bin/php
BASE="${BASE:-f5f746cf}"
H1="$REPO/php-rust/wp181-harness"
FX2="$REPO/php-rust/wp174-harness/fixtures/fx-sw2-gc.php"
FX1="$REPO/php-rust/wp174-harness/fixtures/fx-sw1.php"
H2="$REPO/php-rust/wp172-harness"
FXCR="$H1/fx-cr1.php"
SRC="/Volumes/Extreme Pro/Claude/s182-scomp"
# TARGET dei bracci sulla SPARSEBUNDLE APFS (decisione utente 2026-09-29: target incrementali/separate sulla bundle, mai
# su ExFAT né sulla canonica php-rust-output); la bundle DEVE essere montata (phpr-target-bundle.sh status) altrimenti rc=8
BUNDLE_MP="$HOME/Claude/phpr-target"; TGT="$BUNDLE_MP/s182-scomp-tgt"
: > "$VERD"
fin(){ echo "rc=$1 $(date +%T)" > "$DONE"; exit "$1"; }
note(){ echo "$*" >> "$VERD"; }

grep -qw s183 "$LOCK" 2>/dev/null || { note "rc=9 lock s183 assente (per TOKEN)"; fin 9; }
for f in "$FX2" "$FX1" "$H2/fx-sl1.php" "$H2/fx-sl2.php" "$H2/fx-sl3.php" "$FXCR"; do [ -s "$f" ] || { note "rc=7 fixture assente: $f"; fin 7; }; done
PH=$(shasum -a 256 "$PIN" | cut -c1-16)
[ "$PH" = "$PIN_ATTESO" ] || { note "rc=9 pin $PH ≠ atteso $PIN_ATTESO"; fin 9; }
cd "$REPO" || fin 7
SHA0=$(git rev-parse --verify "$BASE^{commit}") || { note "rc=7 commit BASE $BASE inesistente"; fin 7; }
AVX=$(df -k "/Volumes/Extreme Pro" | awk 'NR>1{printf "%.0f", $4/1048576}')
DAT=$(df -g /System/Volumes/Data | awk 'NR==2{print $4}')
note "== s182 bracci SCOMPOSIZIONE L-CR1 — riferimento A = pin s181 $PH, BASE = commit ${SHA0:0:12} (sorgente del pin), bracci Z/P/MA(−a)/MB(−b)/MC(−c)/M2, fixture fx-cr1 $(wc -l < "$FXCR" | tr -d ' ') righe; Extreme ${AVX}G Data ${DAT}G $(date '+%F %T') =="
awk -v a="$AVX" 'BEGIN{exit !(a+0 < 15)}' && { note "rc=8 Extreme ${AVX}G < 15G: niente build"; fin 8; }
/sbin/mount | grep -q " $BUNDLE_MP " || { note "rc=8 bundle NON montata ($BUNDLE_MP): 'wp182-harness/phpr-target-bundle.sh mount' prima della build"; fin 8; }
BFREE=$(df -g "$BUNDLE_MP" | awk 'NR==2{print $4}'); [ "${BFREE:-0}" -ge 10 ] || { note "rc=8 bundle ${BFREE}G liberi < 10G"; fin 8; }
[ "${DAT:-0}" -ge 10 ] || { note "rc=8 Data ${DAT}G < 10G: niente build"; fin 8; }

# riferimento: pin su fx-sw2-gc (marcatore) e gate bilaterali fx-sw1 + fx-cr1 sul pin (sanità del gate)
perl -e 'alarm 120; exec @ARGV or die' -- "$PIN" "$FX2" > "$OUT/ref.out" 2>&1
grep -q "FX-SW2 DONE" "$OUT/ref.out" || { note "rc=7 riferimento: marcatore FX-SW2 assente"; fin 7; }
"$ORACLE" "$FX1" > "$OUT/sw1-oracle.out" 2>&1
perl -e 'alarm 120; exec @ARGV or die' -- "$PIN" "$FX1" > "$OUT/sw1-pin.out" 2>&1
diff -q "$OUT/sw1-oracle.out" "$OUT/sw1-pin.out" > /dev/null || { note "rc=7 fx-sw1: pin ≠ oracle (gate rotto a monte)"; fin 7; }
"$ORACLE" -d log_errors=0 -d display_errors=1 "$FXCR" > "$OUT/fxcr1-oracle.out" 2>&1
perl -e 'alarm 120; exec @ARGV or die' -- "$PIN" "$FXCR" > "$OUT/fxcr1-pin.out" 2>&1
grep -q "FX-CR1 DONE" "$OUT/fxcr1-pin.out" || { note "rc=7 riferimento: marcatore FX-CR1 assente sul pin"; fin 7; }
diff -q "$OUT/fxcr1-oracle.out" "$OUT/fxcr1-pin.out" > /dev/null || { note "rc=7 fx-cr1: pin ≠ oracle (fixture non bilaterale a monte)"; fin 7; }
NACEM=$(grep -c '^ACE m' "$OUT/fxcr1-oracle.out"); NACE=$(grep -c '^ACE' "$OUT/fxcr1-oracle.out")
note "RIFERIMENTO: pin fx-sw2-gc $(tr '\n' ' ' < "$OUT/ref.out" | cut -c1-120) · fx-sw1 pin==oracle BYTE-ID · fx-cr1 pin==oracle BYTE-ID (righe ACE: $NACE, di cui «ACE m»: $NACEM)"

archivio(){ # $1=sha
  rm -rf "$SRC"; mkdir -p "$SRC/php-rust"
  git archive "$1" php-rust/crates php-rust/Cargo.toml php-rust/Cargo.lock php-rust/rust-toolchain.toml php-rust/.cargo 2>/dev/null | tar -x -C "$SRC" || return 1
  [ -s "$SRC/php-rust/crates/php-runtime/src/vm/run.rs" ] || return 1
  /usr/bin/find "$SRC" -type f -exec touch {} +
}
build(){ # $1=etichetta → stampa hash16; log in $OUT/build-$1.log
  ( cd "$SRC/php-rust" && SOURCE_DATE_EPOCH=0 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR="$TGT" \
      cargo build --release -p php-cli ) > "$OUT/build-$1.log" 2>&1 || return 1
  cp "$TGT/release/phpr" "$OUT/phpr-$1"; shasum -a 256 "$OUT/phpr-$1" | cut -c1-16
}
patch_arm(){ # $1=braccio → applica la patch python sull'ARCHIVIO; stdout = esito; rc≠0 = ancora non trovata/non unica
  python3 - "$1" "$SRC/php-rust/crates/php-runtime/src/vm/run.rs" "$SRC/php-rust/crates/php-runtime/src/vm/mod.rs" <<'PY'
import sys
arm, prun, pmod = sys.argv[1:4]
def rd(p): return open(p, encoding='utf-8').read()
def wr(p, s): open(p, 'w', encoding='utf-8').write(s)
def rep(s, old, new, n=1):
    c = s.count(old); assert c == n, f"{arm}: atteso {n} occorrenze di {old[:50]!r}, trovate {c}"; return s.replace(old, new)
I24 = ' ' * 24; I28 = ' ' * 28
LOOP = (I24 + 'for i in (0..n).rev() {\n' + I28 + 'let a = caller[top].stack.pop().expect("call argument");\n' + I28 + 'cf.slots[i] = decay_arg(a);\n' + I24 + '}\n')
ARGC = I24 + 'cf.argc = n as u32;\n'
IPB_FAST = I24 + 'if matches!(callee.ops.first(), Some(Op::CheckArity { .. })) {\n' + I28 + 'cf.ip = 1;\n' + I24 + '}\n'
IPB_MC = ' ' * 8 + 'if matches!(callee.ops.first(), Some(Op::CheckArity { .. })) {\n' + ' ' * 12 + 'frame.ip = 1;\n' + ' ' * 8 + '}\n'
if arm == 'Z':
    print('Z: nessuna patch (base)'); sys.exit(0)
if arm == 'P':
    s = rd(prun); s = rep(s, ARGC + LOOP, LOOP + ARGC); wr(prun, s); print('P: cf.argc spostato dopo il ciclo (placebo)'); sys.exit(0)
if arm == 'MA':
    s = rd(prun)
    i = s.index('if log::log_enabled!(target: "phpr::call", log::Level::Trace) {\n' + I28 + 'log::trace!(')
    assert s.count('if log::log_enabled!(target: "phpr::call", log::Level::Trace) {\n' + I28 + 'log::trace!(') == 1, 'MA: trace-log del fast path non unico'
    j = s.index(IPB_FAST, i) + len(IPB_FAST)
    assert j - i < 2000, 'MA: blocco (a) troppo lungo'
    old = (I24 + 'let mut frame = self.pooled_frame(callee, m);\n' + I24 + 'frame.argc = n as u32;\n' + I24 + 'for i in (0..n).rev() {\n'
           + I28 + 'let a = self.frames[top].stack.pop().expect("call argument");\n' + I28 + 'frame.slots[i] = decay_arg(a);\n' + I24 + '}\n'
           + I24 + 'if matches!(callee.ops.first(), Some(Op::CheckArity { .. })) {\n' + I28 + 'frame.ip = 1;\n' + I24 + '}\n'
           + I24 + 'self.enter_callee(frame)?;\n')
    s = s[:i - 24] + old + s[j:]
    wr(prun, s); print('MA: (a) revertita (pooled_frame + enter_callee come nel pin s180, ip=1 tenuto)'); sys.exit(0)
if arm == 'MB':
    s = rd(prun); s = rep(s, IPB_FAST, ''); s = rep(s, IPB_MC, ''); wr(prun, s); print('MB: (b) revertita (ip=1 tolto su Call fast E methodcall_fast)'); sys.exit(0)
if arm == 'MC':
    s = rd(prun)
    s = rep(s, '.map(|e| std::mem::take(&mut e.guard_release));\n', '.map(|e| std::mem::take(&mut e.guard_release))\n' + I24 + '.unwrap_or_default();\n')
    s = rep(s, ' ' * 20 + 'if let Some(guard) = guard {\n' + I24 + 'for key in guard {\n' + I28 + 'self.magic_guard.remove(&key);\n' + I24 + '}\n' + ' ' * 20 + '}\n',
               ' ' * 20 + 'for key in guard {\n' + I24 + 'self.magic_guard.remove(&key);\n' + ' ' * 20 + '}\n')
    wr(prun, s); print('MC: (c) revertita (unwrap_or_default + for key in guard)'); sys.exit(0)
if arm == 'M2':
    s = rd(pmod)
    anchor = 'bind_params(&mut frame, args);\n' + ' ' * 16 + 'frame.this = Some(this);\n' + ' ' * 16 + 'frame.class = Some(defc);\n'
    s = rep(s, anchor, anchor + ' ' * 16 + 'if matches!(callee.ops.first(), Some(crate::bytecode::Op::CheckArity { .. })) { frame.ip = 1; } // MUTANTE M2 s182 (Op non importato nel modulo: path completo)\n', 2)
    wr(pmod, s); print('M2: ip=1 inserito sui DUE siti lenti di dispatch_instance_call (mutante)'); sys.exit(0)
raise SystemExit(f'braccio sconosciuto {arm}')
PY
}
rotte(){ # $1=ref.out $2=mut.out → stdout etichette rotte (blocchi per etichetta)
  python3 - "$1" "$2" <<'PY'
import sys, re
def blocks(p):
    d, cur = {}, None
    for line in open(p, encoding='utf-8', errors='replace'):
        m = re.match(r"^([A-Za-z0-9_().'#=-]+): ", line)
        if m: cur = m.group(1); d.setdefault(cur, [])
        if cur is not None: d[cur].append(line)
    return d
a, b = blocks(sys.argv[1]), blocks(sys.argv[2])
for k in a:
    if a[k] != b.get(k): print(k)
PY
}

ARMS="Z P MA MB MC M2"
declare -A HH
if [ "${SKIP_BUILD:-0}" = 1 ]; then
  for a in $ARMS; do [ -s "$OUT/phpr-$a" ] || { note "rc=7 SKIP_BUILD: manca phpr-$a"; fin 7; }; HH[$a]=$(shasum -a 256 "$OUT/phpr-$a" | cut -c1-16); done
  note "SKIP_BUILD=1: riuso dei binari già costruiti — $(for a in $ARMS; do printf '%s %s · ' "$a" "${HH[$a]}"; done)"
else
  for a in $ARMS; do
    archivio "$SHA0" || { note "rc=7 archivio $a fallito"; fin 7; }
    pe=$(patch_arm "$a" 2>&1) || { note "rc=7 patch $a NON applicata: $pe"; fin 7; }
    note "PATCH $a: $pe"
    diff -ru "$REPO/php-rust/crates/php-runtime/src/vm" "$SRC/php-rust/crates/php-runtime/src/vm" > "$OUT/patch-$a.diff" 2>/dev/null || true
    HH[$a]=$(build "$a") || { note "rc=4 $a: build FALLITA (ab-out/s182-scomp/build-$a.log)"; fin 4; }
    note "$a: binario ${HH[$a]} ($(grep -c '^[-+][^-+]' "$OUT/patch-$a.diff") righe di patch vs tree)"
  done
fi
if [ "${HH[Z]}" = "$PH" ]; then note "Z: base == pin BYTE-ID (la target separata riproduce il pin: i bracci sono confrontabili al byte)"; else note "Z: base ${HH[Z]} ≠ pin $PH (gemello a CONTENUTO — dichiarato: LC_UUID/firma; A resta il pin canonico, il placebo P copre la banda-layout)"; fi
[ "${HH[P]}" != "$PH" ] && [ "${HH[P]}" != "${HH[Z]}" ] || { note "rc=5 P: binario == pin/base (placebo NULLO: l'edit non entra nel codegen — cambiare placebo)"; fin 5; }
for a in MA MB MC M2; do [ "${HH[$a]}" != "${HH[Z]}" ] || { note "rc=7 $a: binario == base (patch NON entrata)"; fin 7; }; done

RC=0
bilat(){ # $1=braccio $2=nome $3=file $4=marcatore $5..=opzioni oracle
  local a="$1" n="$2" f="$3" m="$4"; shift 4
  [ -s "$OUT/$n-oracle.out" ] || "$ORACLE" "$@" "$f" > "$OUT/$n-oracle.out" 2>&1
  perl -e 'alarm 300; exec @ARGV or die' -- "$OUT/phpr-$a" "$f" > "$OUT/$n-$a.out" 2>&1
  [ -s "$OUT/$n-$a.out" ] || { note "$a: $n output VUOTO -> rc=2"; RC=2; return; }
  [ -z "$m" ] || grep -q "$m" "$OUT/$n-$a.out" || { note "$a: $n marcatore $m ASSENTE -> rc=2"; RC=2; return; }
  if diff -q "$OUT/$n-oracle.out" "$OUT/$n-$a.out" > /dev/null; then note "$a: $n == oracle BYTE-ID"; else diff "$OUT/$n-oracle.out" "$OUT/$n-$a.out" > "$OUT/$n-$a.diff" || true; note "$a: $n DIVERGE dall'oracle ($(wc -l < "$OUT/$n-$a.diff" | tr -d ' ') righe di diff) -> rc=2"; RC=2; fi
}
for a in P MA MB MC; do
  perl -e 'alarm 120; exec @ARGV or die' -- "$OUT/phpr-$a" "$FX2" > "$OUT/sw2-$a.out" 2>&1
  if diff -q "$OUT/ref.out" "$OUT/sw2-$a.out" > /dev/null; then note "$a: fx-sw2-gc == pin BYTE-ID (invarianza)"; else rotte "$OUT/ref.out" "$OUT/sw2-$a.out" | sort -u > "$OUT/sw2-$a.rotte"; note "$a: fx-sw2-gc ≠ pin — blocchi ROTTI: $(tr '\n' ' ' < "$OUT/sw2-$a.rotte") -> rc=2"; RC=2; fi
  bilat "$a" fxsw1 "$FX1" "FX-SW1 DONE"
  bilat "$a" fxsl1 "$H2/fx-sl1.php" "FX-SL1 DONE" -d log_errors=0 -d display_errors=1
  bilat "$a" fxsl2 "$H2/fx-sl2.php" "FX-SL2 DONE" -d log_errors=0 -d display_errors=1
  bilat "$a" fxsl3 "$H2/fx-sl3.php" "FX-SL3 DONE" -d log_errors=0 -d display_errors=1
  bilat "$a" fxcr1 "$FXCR" "FX-CR1 DONE" -d log_errors=0 -d display_errors=1
done

# M2: DEVE divergere dall'oracle su fx-cr1 ESATTAMENTE sulle righe «ACE m» (esito esatto, mai «diverso da»)
perl -e 'alarm 120; exec @ARGV or die' -- "$OUT/phpr-M2" "$FXCR" > "$OUT/fxcr1-M2.out" 2>&1
MACEM=$(grep -c '^ACE m' "$OUT/fxcr1-M2.out")
diff "$OUT/fxcr1-oracle.out" "$OUT/fxcr1-M2.out" > "$OUT/fxcr1-M2.diff" || true
GONE_OTHER=$(grep '^<' "$OUT/fxcr1-M2.diff" | grep -vc '^< ACE m')
EXTRA_BAD=$(grep '^>' "$OUT/fxcr1-M2.diff" | grep -vcE '^> (Warning: Undefined variable \$[a-z]+ in .* on line [0-9]+)?$')
if [ "$MACEM" -lt "$NACEM" ] && [ "$GONE_OTHER" -eq 0 ] && [ "$EXTRA_BAD" -eq 0 ]; then
  note "MUTANTE M2: morde — righe «ACE m» oracle $NACEM vs M2 $MACEM; sparite SOLO righe «ACE m», in più solo warning «Undefined variable»/vuote ($(grep -c '^> Warning' "$OUT/fxcr1-M2.diff")): fx-cr1 presidia (b) anche sul cammino lento dei METODI (rilievo 7 S-181 chiuso)"
else
  note "MUTANTE M2: NON morde nella forma attesa («ACE m» oracle $NACEM vs M2 $MACEM, sparite altre $GONE_OTHER, in più non-warning $EXTRA_BAD; ab-out/s182-scomp/fxcr1-M2.diff) -> rc=3"; [ "$RC" -eq 0 ] && RC=3
fi

# disasm agli atti (p.2): istr/bl/blr/sp_refs di run_loop del pin e di ogni braccio
dis(){ # $1=binario $2=etichetta
  local sym; sym=$(nm -n "$1" | awk '{print $3}' | grep -iE '8run_loop17h|2Vm8run_loop$' | head -n 1)
  [ -n "$sym" ] || { echo "simbolo run_loop non trovato"; return; }
  objdump -d --no-show-raw-insn --disassemble-symbols="$sym" "$1" > "$OUT/disasm-$2-run_loop.s" 2>/dev/null
  echo "istr=$(grep -cE '^ *[0-9a-f]+:' "$OUT/disasm-$2-run_loop.s") bl=$(grep -cE '[[:space:]]bl[[:space:]]' "$OUT/disasm-$2-run_loop.s") blr=$(grep -cE '[[:space:]]blr[[:space:]]' "$OUT/disasm-$2-run_loop.s") sp_refs=$(grep -c '\[sp' "$OUT/disasm-$2-run_loop.s")"
}
note "DISASM run_loop: pin s181 $(dis "$PIN" pin)"
for a in Z P MA MB MC; do note "DISASM run_loop: $a $(dis "$OUT/phpr-$a" "$a")"; done
rm -rf "$TGT" "$SRC"
note "ESITO rc=$RC (0 = bracci a parità e M2 che morde: pronti per s182-ab-scomp.sh; 2 = un braccio diverge; 3 = M2 non morde) fine $(date '+%F %T')"
fin $RC
