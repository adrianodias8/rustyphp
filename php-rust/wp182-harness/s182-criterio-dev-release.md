# S-182 — criterio PRE-registrato: profilo `dev-release` incrementale (apparato, segue target-bundle)

Domanda: `cargo build --profile dev-release` (inherits release, incremental=true, lto=false, cgu=16)
accorcia la build dopo un edit tipico rispetto a `cargo build --release` (fat LTO, cgu 1, incremental OFF)?
Il profilo release del pin NON cambia; il binario dev-release non è mai pin, braccio di misura o CI.

- **Segno**: wall-clock della build dopo `touch crates/php-runtime/src/coerce.rs`, target = bundle
  `~/Claude/phpr-target/dev-output` (promossa S-182). A = `--release` · B = `--profile dev-release`.
- **R**: freddo B R=1 (A freddo già misurato: 204,8 s) · caldo R=3 interleaved A B A B A B · giudice mediana.
- **Parità (gate, non tempo)**: `phpr` dev-release e release su `wp97-harness/micro/arith_small.php`
  (entrambi i modi PHPR_REG_LOWER) == oracle. Diverso ⇒ profilo BOCCIATO a prescindere dal tempo.
- **Soglia**: B/A ≤ 0,50 ⇒ PROMOSSO come comando ordinario di sviluppo (docs) · 0,50 < B/A ≤ 0,80 ⇒
  facoltativo (resta nel Cargo.toml, non nei docs) · > 0,80 ⇒ bocciato, profilo rimosso.
- **Pre-gate** come s182-target-bundle-ab.sh (bundle montata, lock nostro o assente, nessun job CI, loadavg < 3).
- **Esito**: solo numeri in `target-bundle-out/s182-dev-release-verdetto.out`; rumore A > 15 % ⇒ non giudicabile.

## EMENDA E1 (dichiarata 2026-09-29 19:4x, PRIMA della gamba, durante la corsa `touch`)
Il `touch` cambia l'mtime ma non il contenuto: il release ricompila comunque il crate, dev-release riconosce
l'hash invariato e riusa tutto (warm-B1 4,4 s) ⇒ la gamba `touch` è il CASO MIGLIORE di B, non l'edit tipico.
Gamba **EDIT** (giudica lei, stesse soglie): a ogni round si APPENDE a `coerce.rs` una `pub fn __s182_probe_<i>()`
nuova (contenuto diverso a ogni build di ogni braccio), A = `--release` · B = `--profile dev-release`, R=2
interleaved A B A B; ripristino con copia salvata (`cmp` al byte a fine corsa, nessun comando git sui `.rs`).
La gamba `touch` resta a verbale come limite inferiore. Esito in `target-bundle-out/s182-dev-release-edit-verdetto.out`.

## ESITO (2026-09-29 19:56, HEAD 4e5b660f, rustc 1.98.1, tree pulito, lock nostro)
Gamba `touch` (limite inferiore): freddo dev-release 114,6 s (release 204,8) · caldo A 164,1/151,2/160,0 · B 4,4/3,9/4,3 ·
mediana B/A 0,026 · rumore A 2,4 % · parità arith on/off: release=ok dev-release=ok · du release 909M · dev-release 1,3G.
Gamba **EDIT** (giudica, E1): A 151,1/147,5 · B 28,4/27,5 · **mediana B/A = 0,186** · ripristino coerce.rs al byte ⇒
**PROMOSSO**: `cargo build --profile dev-release` è il comando ordinario di sviluppo (docs). Il binario sta in
`target/dev-release/` e NON è mai pin, braccio di misura o artefatto CI: la ricetta del pin resta `--release`.
Verdetti: target-bundle-out/s182-dev-release-verdetto.out, s182-dev-release-edit-verdetto.out.
