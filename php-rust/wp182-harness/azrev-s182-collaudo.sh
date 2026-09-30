#!/bin/bash
# azrev-s182-collaudo.sh — S-183 pre-flight, azioni della revisione S-182 (rilievi 2-3): (1) build ricetta sulla CANONICA con
# CARGO_TARGET_DIR esplicito ⇒ l'hash di phpr DEVE tornare 19a2faa83a492745 (collaudo «release invariato al byte» dopo il profilo
# dev-release; se diverge il pin è ROTTO: STOP e istruttoria); (2) una tantum `cargo test --profile dev-release` + corpus-gate col
# phpt-runner dev-release (target dev-output sulla bundle) ⇒ il profilo è dichiarato solo se batteria rc=0 e corpus rc=0.
set -u
export PATH=/usr/bin:/bin:/usr/sbin:/opt/homebrew/bin:"$HOME/.cargo/bin"
SRC="/Volumes/Extreme Pro/Claude/php-rust-experiment/php-rust"; O="$SRC/wp182-harness/azrev-out"; mkdir -p "$O"
V="$SRC/wp182-harness/s183-azrev-s182-verdetto.out"; : > "$V"; n(){ echo "$(date '+%T') $*" >> "$V"; }
CAN="$HOME/Claude/php-rust-output"; PIN=19a2faa83a492745
n "== collaudo az.rev. S-182 — HEAD $(git -C "$SRC" rev-parse --short HEAD), bundle $(/sbin/mount | grep -q " $HOME/Claude/phpr-target " && echo MONTATA || echo SMONTATA) =="
H0=$(shasum -a 256 "$CAN/release/phpr" | cut -c1-16); n "canonica prima: $H0"
( cd "$SRC" && CARGO_TARGET_DIR="$CAN" SOURCE_DATE_EPOCH=0 CARGO_INCREMENTAL=0 cargo build --release ) > "$O/build-canonica.log" 2>&1; rc=$?
H1=$(shasum -a 256 "$CAN/release/phpr" | cut -c1-16)
if [ "$rc" = 0 ] && [ "$H1" = "$PIN" ]; then n "(1) build ricetta canonica rc=0: phpr $H1 == pin $PIN AL BYTE (rilievo 2 CHIUSO)"; else n "(1) build ricetta canonica rc=$rc: phpr $H1 vs pin $PIN — PIN ROTTO? STOP (stash phpr-s181 da ripristinare)"; echo "rc=1" > "$O/collaudo.done"; exit 1; fi
( cd "$SRC" && CARGO_INCREMENTAL=0 cargo test --profile dev-release ) > "$O/batteria-dev-release.log" 2>&1; rc=$?
n "(2a) cargo test --profile dev-release rc=$rc: $(grep -E '^test result' "$O/batteria-dev-release.log" | awk '{p+=$4; f+=$6} END{print p" passed / "f" failed"}')"
RUN="$HOME/Claude/phpr-target/dev-output/dev-release/phpt-runner"
[ -x "$RUN" ] || RUN="$(ls -d "$HOME"/Claude/phpr-target/dev-output/*/phpt-runner 2>/dev/null | head -1)"
if [ -x "$RUN" ]; then "$SRC/scripts/corpus-gate.sh" "$RUN" "$O/corpus" > "$O/corpus.stdout" 2>&1; rc2=$?; n "(2b) corpus-gate col runner dev-release ($RUN) rc=$rc2: $(tail -1 "$O/corpus.stdout" | cut -c1-120)"; else rc2=7; n "(2b) phpt-runner dev-release NON trovato: rc=7"; fi
n "ESITO: rilievo 2 $( [ "$H1" = "$PIN" ] && echo CHIUSO || echo APERTO) · rilievo 3 $( [ "$rc" = 0 ] && [ "$rc2" = 0 ] && echo 'CHIUSO (profilo dev-release dichiarato: batteria rc=0, corpus rc=0)' || echo "APERTO (batteria rc=$rc, corpus rc=$rc2)")"
echo "rc=$(( rc + rc2 ))" > "$O/collaudo.done"
