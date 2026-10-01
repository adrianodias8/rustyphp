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
```

Environment switches: `PHPR_REG_LOWER`, `PHPR_UNIT_CACHE=0`, `PHPR_DUMP_OPS=1`,
`PHPR_LOG=debug|trace`; the `PHPR_*` names are upstream's and are kept.
