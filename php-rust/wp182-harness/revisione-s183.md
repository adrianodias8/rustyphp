# Revisione S-183 — lente PROCESSO

## Verdetto: REGGE CON RILIEVI

## Ciò che regge
- Istruttoria: criterio pre-registrato in S-182, esiti da `.done`/verdetti, gate eseguiti alla lettera ⇒ rc=0 legittimo.
- Scomposizione: placebo, bracci −a/−b/−c, M2 che morde, disasm agli atti; tre DIREZIONI (5/5, ABAB) firmate.
- L-RT1: direzione 5/5 con guardie esplicite; canonica == pin al byte (hash ricalcolati).
- Incidenti datati nei log (`lancio-scomp-t1-rc9.log`, `avvio-istruttoria.log`).

## Rilievi
1. **L-RT1 «tenuta nel tree» senza batteria né corpus.** Gate dei bracci = 7 fixture (`s183-leva-build-verdetto.out`); nessun `cargo test`/corpus-gate su 9c78dc39. `phpr-ci/CI_FEED.log` ultima riga `START 3dcd2ff3 20:30:23`, nessun DONE; 8 commit S-183 mai costruiti. S-184 comporrebbe su un tree non collaudato.
2. **Gate del mutante emendato a valle senza riscrivere il criterio.** `s183-criterio-rt1.md` p.2 (storia git: solo 9c78dc39) esige fx-cr1 ed «esito ESATTO»; `s183-leva-build.sh:33-34,154` ridefinisce fixture e classe `dtor|collect`. `ab-out/s183-leva/fxrt1-M.diff` mostra anche «dtor p1» spostato (forma `plain`, dichiarata fuori perimetro). Disasm B bl +9 / sp_refs +21 contro p.6 «−2 / stabile»: meccanismo contraddetto come in S-181, senza riscrittura.
3. **«Finestra pulita» contro la propria sentinella.** `ab-out/finestra-rt1.txt` INIZIO: `100.0 /Applications/Google`; E2 100-106 % passa perché <150 % ammette un core saturo (scomp: 14-24 %). Per t24 la sentinella è cieca: `pair-out/quiet-decl-t24.txt` top tutto 0.0 (primo campione di `top -l 1`) mentre E2 leggeva 98-135 %.
4. **Somma dei componenti presentata come cifra** («8,50-9,17 ricompone», `s183-scomp-verdetto.out`): regola 3 vieta cifre da componenti; −a (+3,67) include il trasversale (+2,07 prop-dq), contato due volte. Criterio scomp p.2 «Z byte-id altrimenti rc=7» disatteso (Z ≠ pin, rc=0; in rt1 Z cambia ancora) senza emenda.
5. **Incidente #5 = reperto non provato.** «Pin server NON riproducibile» (`WP_SESSION_183.md` §6) nasce da `cargo build --release` dell'intero workspace (`azrev-out/build-canonica.log:331`), non dalla ricetta `pin-server.sh:19` (`-p php-server --features axum-server`). #4/#5 assenti da commit e log.

## Azioni S-184
1. Apertura: CI_FEED per 3dcd2ff3; se assente/rosso, `cargo test --profile dev-release` + corpus-gate PRIMA di ogni leva. «Tenuta nel tree» solo con batteria+corpus a verbale.
2. Riscrivere criterio-rt1 p.2/p.6 con morso eseguito e disasm letto; disasm in gate (bl atteso ± tolleranza) prima della promozione composta.
3. Sentinella come gate: processo singolo >50 % ⇒ attesa; `top -l 2` nella dichiarazione.
4. Eseguire la ricetta `pin-server.sh` in target separata, confronto con b2802f08: confermare o ritirare #5 in un commit.
5. Verdetto scomp: «tre direzioni, somma indicativa» al posto di «ricompone la cifra»; emendare criterio scomp p.2.
