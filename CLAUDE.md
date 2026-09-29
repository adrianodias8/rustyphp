# php-rust-experiment

Reimplementazione moderna di PHP 8.5 in Rust, guidata dal comportamento osservabile
(oracle: i 21.548 test .phpt del sorgente ufficiale), NON porting dell'architettura Zend.

## Riferimenti

- Sorgente C originale (snapshot, NON copiato qui): `/Volumes/Extreme Pro/Claude/php-8.5.7`
- Mappa delle fonti e ordine di lettura obbligatorio: [AGENTS.md](AGENTS.md)
  (→ `php-rust/REGOLE.md` processo → `php-rust/NEXT_SESSION_WORDPRESS.md` stato)
- Metodologia: skill `legacy-port` (adattata: reimplementazione spec-driven, non traduzione)

## Convenzioni

- Lingua diario: italiano. Lingua codice e commenti: inglese.
- Branch: `main`. Conventional commits in inglese (`feat:`, `docs:`, `test:`, `chore:`).
- Processo (misura, pin, gate, rotazione): SOLO `php-rust/REGOLE.md`. Commit+push a ogni
  passo, mai con build red. File `.rs` mai nei comandi git: `git add -u` + `git commit -F`.
- Ogni file di diary dichiara "Generato con assistenza AI (Claude Fable 5)".
- Le stringhe PHP sono byte (`[u8]`), MAI `String`/UTF-8.
- Baseline .phpt committata: non deve mai regredire tra step.

## Struttura

- `php-rust/` — workspace Cargo (crates: php-types, php-runtime, php-builtins, php-cli,
  php-server, phpt-runner); harness per sessione in `php-rust/wp<N>-harness/`, verbali
  in `php-rust/sessions/`
- `diary/` — 00-reconnaissance … 04-divergences, metrics, NEXT-*.md (backlog per area)

## Comandi

- Test: `cd php-rust && cargo test --release` (SEMPRE `--release`: il profilo
  debug rigenera ~3,8G di artefatti in `php-rust-output`)
- CLI: `cargo run -p php-cli -- script.php` (binario `phpr`, php drop-in)
- Runner .phpt: `cargo run -p phpt-runner -- <dir o file .phpt>` (`--isolate`, `--list-fails`)
- Logging: `PHPR_LOG=debug|trace` (stderr), `PHPR_LOG_FILE=<path>`,
  `PHPR_LOG_CONFIG=<log4rs.yaml>` (vedi `php-runtime/src/logging.rs`)

> **Build / filesystem:** il volume esterno "Extreme Pro" NON supporta la
> compilazione incrementale di Rust (non hard-linka la cache). Gli artefatti vivono
> quindi sul volume principale: `~/Claude/php-rust-output` è la target CANONICA coi
> binari pinnati (solo `scripts/pin-phpr.sh` e build di promozione); lo sviluppo
> ordinario usa `CARGO_TARGET_DIR=$HOME/Claude/php-rust-dev-output` (dettagli in
> `php-rust/CLAUDE.md`). NON build sul volume esterno. Sorgente/corpus sul volume
> esterno sono solo letti.
> Engine: VM a bytecode unico (pipeline mago AST→HIR→bytecode→VM); il vecchio
> tree-walker `eval/` è stato eliminato. Lowering in `php-runtime/src/lower/`,
> VM in `php-runtime/src/vm/` (mod.rs ~26k righe, loop caldo in `run.rs` ~7,4k con
> cap LOC dichiarato — usare Serena; un hook BLOCCA grep/cat sui .rs via Bash).
