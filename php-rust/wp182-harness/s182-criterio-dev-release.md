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
