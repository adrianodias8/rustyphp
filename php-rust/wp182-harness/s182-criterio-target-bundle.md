# S-182 — criterio PRE-registrato: target incrementale su sparsebundle (apparato, timebox ½ sessione)

Domanda: la target di SVILUPPO su `~/Claude/phpr-target/dev-output` (APFS in `phpr-target.sparsebundle`
sul volume esterno, 150 GiB nominali) è abbastanza veloce da sostituire `~/Claude/php-rust-dev-output`
(disco interno, 14 GiB liberi, oggi potato a ogni sessione)? La canonica e la CI NON sono toccate.

- **Segno**: wall-clock di `cargo build --release` (comando ordinario, incrementale) dopo `touch` di
  `crates/php-runtime/src/coerce.rs` (edit tipico: ricompila runtime + relink fat-LTO). Freddo = informativo.
- **Bracci**: A = disco interno · B = bundle. Stesso HEAD, stesso rustc 1.98.1, tree pulito.
- **R**: freddo R=1 per braccio (A poi B); caldo R=3 interleaved A B A B A B. Giudice: mediana caldo.
- **Soglia**: B/A mediana caldo ≤ **1,25** ⇒ bundle PROMOSSA a target dev (config.toml → bundle).
  1,25 < B/A ≤ 1,50 ⇒ solo CI (dove oggi la cache muore a ogni job). > 1,50 ⇒ bocciata, si smonta.
- **Pre-gate** (script, rc≠0 = niente misura): bundle montata · lock `/private/tmp/phpr-measure.lock`
  ASSENTE · nessun phpr/php-server/cargo/ci-runner vivo · loadavg 1m < 3 · Data ≥ 10 GiB liberi.
- **Esito**: solo numeri in `target-bundle-out/s182-target-bundle-verdetto.out` (+ du delle target e
  della bundle su disco). Rumore: se |A1−A3| caldo > 15 % la corsa non è giudicabile, si ripete.
