# Ferrophant — instructions for an agent working here

Read `NOTES.md` (newest session first) before touching anything; it holds
every measured number and where each session stopped.

## Rules

- **Nothing on the host.** Build, test and measure through `docker/run.sh`
  (image `rustyphp-dev:8.5.7`: oracle PHP 8.5.7, pinned Rust, perf, wrk).
  Build artefacts live on the `rustyphp-target` volume (`/target`), scratch
  on `rustyphp-scratch` (`/scratch`), never in the repository.
- **The oracle is always right.** `php` 8.5.7 in the image and php-src at
  tag `php-8.5.7` in `../php-src` decide every behaviour question:
  `diff <(php x.php) <(ferro x.php)` until identical, in both lowering modes
  (`PHPR_REG_LOWER=1` and unset).
- **Every fix ships with a `.phpt`** in `baseline/repro/`, validated on the
  oracle with php-src's `run-tests.php` before it is committed. A divergence
  you find and do not fix gets a row in `KNOWN_DIVERGENCES.md` and a failing
  test in `baseline/divergences/`.
- **Gates before a commit that touches the engine:** `cargo test --release`
  (the one known failure is a root-only filesystem test), `baseline/gate.sh`
  (zero pass→fail on the corpus), the two smoke tests when calls/includes
  changed, and `bench/ab.sh` against the previous commit's binary
  (`/target/prev`, built from a plain copy of that commit). A section above
  1.05 is investigated; `bench/path-sensitivity.sh` and a `--features
  null-lever` build tell layout and placement noise from a regression.
- **Numbers, not adjectives.** Results go in `bench/results/<date>-*.md`
  and `NOTES.md`; the commit message carries the A/B geomeans.
- **Line caps.** `php-rust/crates/php-runtime/tests/loc_cap.rs` caps every source
  file; growing one is declared in the same commit, in its allowlist entry.
- **`unsafe` is pinned.** `php-rust/crates/php-runtime/tests/unsafe_census.rs` counts it per
  file; new `unsafe` needs a raised pin and a `SAFETY:` comment in the same commit, and none is
  added to the VM for speed without the owner's decision (DECISION_KERNEL.md §8).
- **Comments in English.** The code was translated from Italian; keep it so.
- **Never push to `upstream`** (it is disabled on purpose). `origin` is
  github.com/adrianodias8/rustyphp; work on `next`.
- **`run_loop` is layout-sensitive.** Never grow an op arm in `vm/run.rs`:
  put new logic in an `#[inline(never)]` helper (its own file under `vm/`)
  and run the whole `bench/ab.sh` — a fast path added inside the `FieldIsset`
  arm cost 3–8 % on unrelated ops (NOTES.md session 8).
- **Only interleaved timings count.** The host carries background load:
  compare binaries with `bench/drupal/ab-oneshot.sh` / `bench/ab.sh`
  (alternating rounds), never two separate runs.
- OrbStack's bind mount can lag a host-side edit by up to a minute: if
  `cargo build` finishes in 0.1 s without `Compiling …`, `touch` the file
  inside the container and check the binary's timestamp before measuring.

## Commands

```bash
docker/run.sh cargo build --release -p php-cli -p phpt-runner
docker/run.sh cargo test --release --no-fail-fast
docker/run.sh ../baseline/gate.sh
docker/run.sh bash -c 'PHPR_A=/target/prev/release/ferro PHPR_B=/target/release/ferro /work/php-rust/bench/ab.sh'
PRIV=1 docker/run.sh bash -c 'OUT=/scratch/prof /work/php-rust/bench/profile.sh'
bench/worker/bench-wrk.sh          # from the host: phpr worker vs php-fpm vs FrankenPHP
bench/drupal/bench-wrk-drupal.sh   # from the host: Drupal, ARMS="fpm frankenphp ferro-worker ferro-classic"
bench/drupal/scaling.sh            # from the host: 1xN threads vs Nx1 processes (--reuse-port)
bench/edge/bench-wrk-edge.sh       # from the host: ferro-edge in front of php-fpm and ferro, Drupal anonymous
docker/run.sh bash /work/php-rust/bench/drupal/heap-profile.sh      # one worker's memory by allocation site (heaptrack)
docker/run.sh bash /work/php-rust/bench/drupal/ab-oneshot.sh        # Drupal warm request, HEAD vs tree
docker/run.sh bash /work/php-rust/bench/drupal/optime.sh build      # per-op / per-callee time census
docker/run.sh /work/php-rust/baseline/smoke-composer.sh             # and smoke-dbal.sh
```

Profiling (perf annotate crashes on these binaries): a frame-pointer +
line-table build in `/target/fp` (`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only
RUSTFLAGS="-C force-frame-pointers=yes" CARGO_TARGET_DIR=/target/fp`), recorded
with `--call-graph fp`, then `bench/drupal/fold.py` (self/inclusive/callers),
`buckets.py` (by subsystem), `within.py`, and `hotlines.sh` (source lines and
instructions, via gimli's `addr2line` installed in `/target/tools`).

Environment switches: `PHPR_REG_LOWER`, `PHPR_UNIT_CACHE=0`, `PHPR_DUMP_OPS=1`,
`PHPR_LOG=debug|trace`; the `PHPR_*` names are upstream's and are kept.
`PHPR_GC=classic` selects the note/sweep GC engine instead of the default
drop-driven destructors (`vm/gcdrop.rs`); `PHPR_REVALIDATE_FREQ` (seconds)
sets the server's include stat interval (opcache's `revalidate_freq`, 2 s
under `-S`, 0 in the CLI). Server flags: `--workers N` without `--worker` is
the classic pool (fresh Vm per request); `--reuse-port` lets several
processes share the port; `--worker boot.php --zygote` boots the script once per
single-threaded zygote process and forks a child per request (`FERRO_ZYGOTE_STATS=1`
logs the fork cost; `--max-requests N` re-boots a zygote after N children, its first
generation after 32 — a zygote is a snapshot and goes stale as the app's caches change). `ferro-edge` (crate `ferro-edge`) is a separate binary: a
cache-tag aware reverse proxy, independent of the engine.
