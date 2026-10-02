# bench/ — benchmarks and profiles (fork addition, PLAN.md §2)

Everything runs inside the dev container; nothing is installed on the host.

| command | what it does | output |
|---|---|---|
| `docker/run.sh /work/php-rust/bench/run.sh` | every benchmark × 4 engine configurations × `R` runs (default 5), interleaved | `bench/results/<date>.md` |
| `docker/run.sh /work/php-rust/bench/compile-cost.sh` | startup floor and compile cost, 30 runs | stdout |
| `docker/run.sh /work/php-rust/bench/alloc/count.sh` | allocations per loop iteration (builds a `mem-census` phpr in `/target/census`) | stdout (TSV) |
| `PRIV=1 docker/run.sh /work/php-rust/bench/profile.sh` | `perf` profiles of a debug-info build in `/target/prof`, flamegraphs, bucket attribution | `bench/profiles/` |
| `docker/run.sh php -n bench/concat-scaling.php` (and the same with `phpr`) | growth of `.=` with a non-string operand | stdout |
| `bench/worker/bench-wrk.sh` (from the host) | HTTP throughput under `wrk`: phpr worker mode vs nginx+php-fpm+opcache vs FrankenPHP worker mode, hello and the Symfony app, one docker network | `bench/results/<date>-wrk.md` |
| `docker/run.sh /work/php-rust/bench/worker/isolation.sh` | worker mode's request-isolation gate: one worker across N requests vs the one-shot cli-server vs the oracle | stdout |
| `docker/run.sh /work/php-rust/bench/path-sensitivity.sh [phpr] [bench] [R]` | the same script under four path names, one binary: the heap-layout band of a section (session 4: `prop_rmw_1m` 171–201 ms) | stdout |
| `docker/run.sh bash -c 'PHPR_A=… PHPR_B=… /work/php-rust/bench/ab.sh'` then `python3 bench/ab-summary.py <tsv>` | A/B of two ferro binaries (ABBA, 7 rounds); the summary prints per-section geomeans and the rows outside [0.85, 1.05] | TSV / stdout |
| `bench/edge/bench-wrk-edge.sh` (from the host) | ferro-edge (cache-tag reverse proxy) in front of php-fpm and ferro on Drupal's anonymous front page: cached, under BAN, pass-through | `bench/results/<date>-edge-wrk-w<N>.md` |
| `bench/drupal/*` | Drupal 11: install and page parity, worker mode, wrk vs php-fpm/FrankenPHP, classic-pool scaling, op-time census, profiling tools — see `bench/drupal/README.md` | `bench/results/*-drupal-*.md` |
| `bench/proto/` (`CARGO_TARGET_DIR=/target/proto cargo build --release`) | standalone safe-Rust VM layout prototype: ferro's frame layout vs a contiguous value stack (NOTES.md session 8, fifth pass) | stdout |

Rules the harness enforces:

- **Same source on both engines.** `lib/harness.php` prints a `RESULT` checksum per section;
  `lib/summarize.py` marks a section `INVALID` if phpr's checksum differs from the oracle's, so a
  wrong answer can never be reported as a fast one.
- **Inputs are staged on a container-local volume** (`/scratch`), so bind-mount I/O is not
  measured. Generated inputs (`gen/`) and `symfony-app/vendor/` are not committed;
  `symfony-app/composer.lock` is.
- **`memory_limit=-1` on every engine.** The oracle image has no `php.ini`; its 128 MB default
  aborts `arrays.php` and `strings.php`.
- **`Zend/micro_bench.php` is run from a patched copy** on both engines: phpr rejects
  `$x = isset(Foo::$a);` at parse time. That row is reported as not measured.
- **Profiles use a separate binary.** `profile.sh` sets `CARGO_PROFILE_RELEASE_DEBUG=1` through
  the environment; upstream's `Cargo.toml` (its pinned build recipe) is not edited.
- **`alloc/count.sh` is for counting only.** The census binary is instrumented; never time it.

Engines: `phpr` · `php-noopc` (`php -n -d opcache.enable_cli=0`) · `php-opc`
(`php -d opcache.enable_cli=1`, one-shot: the optimizer runs but nothing is cached between
processes) · `php-opc-warm` (`php-opc` + a primed `opcache.file_cache`).

`bench/profiles/*.folded.gz` are the inputs of the table in `PROFILE.md`; re-derive it with
`python3 bench/lib/buckets.py bench/profiles/<name>.folded.gz --md`.
