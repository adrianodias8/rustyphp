# NOTES.md — session log of the fork

Numbers, not adjectives (PLAN.md §0). Newest session first. Every figure below was measured in
this session unless it is explicitly labelled "upstream's claim".

---

# Session 1 — 2026-09-29/30 — Phase 0 (fork, build, baseline) and Phase 1 (profile)

## 0. Environment — read this before comparing any number with upstream's

| | |
|---|---|
| Host | Apple M2 Max, 12 cores, 32 GB, macOS 26.1 |
| Where everything ran | **Docker** (OrbStack 28.5.2), `linux/arm64`, kernel `6.17.8-orbstack`, 12 CPUs / 19.56 GiB visible to the VM |
| Image | `rustyphp-dev:8.5.7` — built from `docker/Dockerfile` (base `php:8.5.7-cli`, Debian 13 trixie), 1.7 GB |
| Oracle | **PHP 8.5.7** (cli, NTS), Zend Engine v4.5.7, Zend OPcache v8.5.7 built in, JIT off; official `docker-library/php` build, built 2026-06-24. `PHP_ORACLE=/usr/local/bin/php` |
| Rust | rustc 1.98.1 (48a229cea 2026-09-01), cargo 1.98.1 — the version pinned by upstream's `php-rust/rust-toolchain.toml` |
| Corpus | php-src tag `php-8.5.7` (`35eab8c0`), 21,548 `.phpt` files (upstream states 21,548) |
| phpr source | upstream `main` @ `9d4ef5ba3945545a0d7d9f53437b2d703ebd0ed9`, no source changes |
| Tools | perf 6.12.111, samply 0.13.1, inferno, Composer 2.10.1, GNU time |

**Nothing was installed on the host.** A Homebrew install was started at the beginning of the
session and stopped at the user's request before any package landed; only partial downloads in
Homebrew's cache were created, and those were deleted.

Consequences of running in Docker that matter for the numbers:

1. **Upstream measures on macOS, natively, against Homebrew's PHP.** We measure on Linux/arm64 in a
   VM against the docker-library PHP build. Ratios (phpr ÷ oracle) are comparable in kind, not
   digit for digit: different allocator environment, different libc, different oracle build flags.
2. **The container runs as root.** This changes the outcome of tests about unwritable paths
   (one Rust unit test, one DBAL test — both identified below).
3. **No hardware PMU in the VM.** Profiles use the `cpu-clock` software event (timer sampling),
   not cycles; there are no cache-miss or branch-miss counters.
4. The oracle image has no `gd`, `intl`, `tidy`, `xsl`, `bcmath` or `gmp` extension. Irrelevant for
   everything measured tonight; relevant later.

## 1. Phase 0 — status against PLAN.md §1

| PLAN item | status |
|---|---|
| 1.1 Fork to our org | **Not done — needs a decision.** PLAN says "our org" without naming it; the GitHub account belongs to three orgs. Creating a public fork is outward-facing, so it was left for the user. Local clone done: remote `upstream` (push URL disabled), branch `main` tracks `upstream/main`, work is on `next`. |
| 1.1 Toolchain, build | Done. `rust-toolchain.toml` already existed (1.98.1). |
| 1.1 Oracle 8.5.x | Done: 8.5.7. |
| 1.1 php-src at matching tag | Done: `../php-src` at `php-8.5.7`. |
| 1.2 `cargo test` | Done: 1747 / 1 / 2 as root, **1748 / 0 / 2** as non-root. |
| 1.2 Operator differential | Done: **37,835 cases, 0 mismatches**. |
| 1.2 phpt `tests` + `Zend/tests` | Done. Lists committed. |
| 1.2 phpt per extension | Done: `baseline/ext.md`. |
| 1.2 Real-world smoke test | Done, **both options run**: (a) Composer fails under phpr; (b) DBAL runs with 10 errors, 8.2× wall. |
| 1.2 Commit baseline + regression gate | Done locally. CI workflow written but **never executed** (no remote). |
| 1.3 `ARCHITECTURE_NOTES.md` | Done. |

### 1.1 Build

| measurement | value |
|---|---:|
| `cargo fetch` (cold registry) | 76 s |
| `cargo build --release --locked -p php-cli`, cold | **124.2 s** |
| `cargo build --release --locked --workspace`, after the above | 58.6 s |
| `phpr` binary | **17,002,224 bytes** (sha256 `4dea4a8fc03a6c99…`) |
| `php-server` binary | 16,999,832 bytes |
| `phpt-runner` binary | 16,922,112 bytes |
| target dir after workspace build | 831 MB |
| compiler warnings | 5 (`php-builtins` 4, `php-types` 1); 0 errors |

Profile: upstream's release recipe unchanged — fat LTO, 1 codegen unit. Upstream quotes 148 s for
a release build on its machine.

### 1.2 Rust test battery

`cargo test --release --locked --workspace --no-fail-fast` — 136 s including compilation of 30
test binaries.

| | passed | failed | ignored |
|---|---:|---:|---:|
| as root (container default) | 1747 | 1 | 2 |
| as uid 65534 | **1748** | **0** | **2** |
| upstream's claim | 1748 | 0 | 2 |

The one failure as root is `php-runtime` `logging::tests::build_config_to_unwritable_file_errs`:
it expects creating `/nonexistent-dir-phpr-xyz/sub/phpr.log` to fail, and root can create it.
Verified by re-running the same test binary as a non-root user: passes, and the whole
`php-runtime` lib suite is then 707 / 0 / 1. **Environmental, not a regression.**

Differential tests, confirmed to have executed against the oracle (not skipped):

| test | result |
|---|---|
| `php-types` `differential_operators_vs_oracle` | **37,835 cases, 0 mismatches** (0.46 s) |
| `php-builtins` `builtins_match_oracle` | ok (0.52 s) |
| `php-runtime` `evaluator_matches_oracle` | ok (1.09 s) |

### 1.2 `.phpt` corpus — `baseline/zend-tests.md`

`phpt-runner --isolate --list-fails --list-skips`, `PHPT_TIMEOUT_SECS=10`, default engine mode,
corpus copied to a container-local volume. Wall time 229 s.

| scope | total | pass | fail | skip | pass rate of runnable |
|---|---:|---:|---:|---:|---:|
| `tests/` + `Zend/tests/` | 6172 | **3041** | 1623 | 1508 | 65.2 % |
| `Zend/tests/` only | 5305 | **2655** | 1412 | 1238 | 65.3 % |
| `tests/` only | 867 | 386 | 211 | 270 | 64.7 % |
| upstream's claim, `Zend/tests/` | 5305 | 2655 | 1412 | 1238 | 65.3 % |

**Upstream's headline corpus claim reproduces exactly**, on a different OS and architecture.

- **Determinism:** the suite was run twice; the pass, fail and skip lists are identical by name
  (0 differences in each).
- Of the 1623 failures, **48 are crashes** of the isolated worker (45 × exit status 101 = a Rust
  panic, 3 × SIGABRT) and **5 are timeouts** (`Zend/tests/gc/gc_049`, `generators/
  yield_from_deep_recursion`, `try/bug70228_7`, `try/try_finally_007`, `try/try_finally_015`).
- Skips by category: 810 compile-error (test expects a compile-time diagnostic the runner does
  not model), 280 section, 116 unsupported, 80 builtin, 80 extension, 70 parse, 62 vm-unsupported,
  6 malformed, 3 ini, 1 expectf.
- Not run: upstream gates in two modes (`PHPR_REG_LOWER=0` and `=1`). Only the default mode was
  baselined.

### 1.2 Per-extension — `baseline/ext.md`

| suite | total | pass | fail | skip | pass rate of runnable | wall |
|---|---:|---:|---:|---:|---:|---:|
| `ext/standard` | 3812 | 1638 | 925 | 1249 | 63.9 % | 247 s |
| `ext/pcre` | 165 | 81 | 46 | 38 | 63.8 % | 4 s |
| `ext/json` | 88 | 60 | 21 | 7 | 74.1 % | 3 s |
| `ext/mbstring` | 417 | 84 | 131 | 202 | 39.1 % | 12 s |
| `ext/hash` | 80 | 17 | 48 | 15 | 26.2 % | 2 s |
| `ext/ctype` | 49 | 46 | 2 | 1 | 95.8 % | 2 s |
| `ext/dom` | 867 | 1 | 1 | 865 | — | 24 s |
| `ext/simplexml` | 156 | 0 | 1 | 155 | — | 4 s |
| `ext/pdo` | 127 | 2 | 5 | 120 | — | 4 s |
| `ext/pdo_sqlite` | 85 | 29 | 42 | 14 | 40.8 % | 2 s |
| `ext/sqlite3` | 96 | 34 | 46 | 16 | 42.5 % | 3 s |
| `ext/date` | 689 | 205 | 396 | 88 | 34.1 % | 19 s |
| `ext/spl` | 787 | 195 | 513 | 79 | 27.5 % | 62 s |
| `ext/reflection` | 493 | 180 | 290 | 23 | 38.3 % | 14 s |
| `ext/session` | 259 | 43 | 28 | 188 | 60.6 % | 7 s |
| `ext/filter` | 120 | 0 | 0 | 120 | — | 4 s |
| `ext/tokenizer` | 53 | 44 | 8 | 1 | 84.6 % | 1 s |

Crashes/timeouts: `ext/standard` 5 crashed + 13 timed out; `ext/spl` 2 crashed + 4 timed out.

**`ext/dom`, `ext/simplexml` and `ext/filter` are not measured by this corpus at all**: 804, 150
and 98 of their tests are skipped because `phpt-runner`'s `--EXTENSIONS--` allowlist does not
include those extensions, regardless of what phpr implements. All three are on the Drupal list
(PLAN §5). Likewise `ext/pdo` (120 of 127 skipped on `--SKIPIF--`). Across all suites 1,767 skips
are `--SKIPIF-- section not modelled`; the runner has a `--run-skipif` flag that was not used.
Upstream's COVERAGE.md claims several of these extensions "100 %" by *function count*; the corpus
cannot confirm or refute that.

### 1.2 Smoke test (a) — `composer require monolog/monolog` — **FAILS under phpr**

`baseline/smoke-composer.sh`, Composer 2.10.1 run from extracted source on both engines (phpr
cannot execute `composer.phar`: `Parse error: unsupported construct (stmt:HaltCompiler)`). Empty
project, empty `COMPOSER_HOME`, empty cache for every run; order oracle, phpr, phpr, oracle.

| run | engine | exit | wall | user | sys | peak RSS | files in `vendor/` |
|---|---|---:|---:|---:|---:|---:|---:|
| 1 | oracle | 0 | 1.50 s | 0.42 s | 0.08 s | 49.7 MB | 152 |
| 2 | phpr | **1** | 1.26 s | 0.39 s | 0.03 s | 182.6 MB | 0 |
| 3 | phpr | **1** | 1.19 s | 0.38 s | 0.01 s | 190.8 MB | 0 |
| 4 | oracle | 0 | 1.35 s | 0.42 s | 0.07 s | 49.6 MB | 152 |

phpr resolves dependencies, then dies before downloading anything:

```
[TypeError] count(): Argument #1 ($value) must be of type Countable|array,
            Composer\DependencyResolver\Pool given     (src/Composer/Installer.php:556)
```

**Root cause isolated to a 12-line script:** `count($obj)` on an object implementing `Countable`
fails when the call is **unqualified inside a namespace**. `\count($obj)` and the same call at
global scope both work. Both `PHPR_REG_LOWER` modes. Committed as
`baseline/repro/count-countable-in-namespace.phpt` (fails under phpt-runner, expected output is
the oracle's). Not fixed tonight — no source changes.

The wall-time comparison PLAN asked for is therefore **not available for (a)**: the phpr times
above are time-to-failure, not time-to-completion. What was confirmed: the Monolog package
installed by the oracle *runs* under phpr with output byte-identical to the oracle's (3 log lines
through `StreamHandler` + `LineFormatter` + a processor).

Upstream's claim that Composer `require` works end to end was presumably made with an older
Composer; with 2.10.1 it does not reproduce.

### 1.2 Smoke test (b) — PHPUnit on Doctrine DBAL

`baseline/smoke-dbal.sh`: DBAL **4.5.0** (`c435cd7`), PHPUnit 11.5.56, default SQLite
configuration, dependencies installed by the oracle's Composer, order oracle, phpr, phpr, oracle.

| run | engine | wall | user | sys | peak RSS | PHPUnit result |
|---|---|---:|---:|---:|---:|---|
| 1 | oracle | 0.80 s | 0.74 s | 0.02 s | 91.0 MB | Tests 4141, Assertions 5959, **Failures 1**, Skipped 649, Incomplete 13 |
| 2 | phpr | 6.53 s | 6.39 s | 0.08 s | 386.8 MB | Tests 4146, Assertions 5931, **Errors 10, Failures 1**, Skipped 639, Incomplete 13 |
| 3 | phpr | 6.38 s | 6.28 s | 0.04 s | 386.6 MB | same as run 2, same test names |
| 4 | oracle | 0.78 s | 0.72 s | 0.03 s | 90.9 MB | same as run 1 |

| ratio phpr ÷ oracle (means of the two runs) | |
|---|---:|
| wall | **8.17×** (6.455 s ÷ 0.79 s) |
| user CPU | **8.68×** (6.335 s ÷ 0.73 s) |
| peak RSS | **4.25×** (386.7 MB ÷ 91.0 MB) |

- The 1 failure common to both engines, `ExceptionTest::testConnectionExceptionSqLite`, expects a
  read-only database file to be unwritable — root again. Environmental.
- phpr-only: **10 errors from 2 causes** (`baseline/smoke-dbal.failures.txt`):
  - 9 × `PortabilityTest::*` — `TypeError: Doctrine\DBAL\Portability\OptimizeFlags::__invoke():
    Argument #2 ($flags) must be of type int, null given`.
  - 1 × `GenericNameParserTest::testValidInput` data set #11 — a name with non-ASCII characters:
    `ExpectedDot: Expected dot at position 9`.
- phpr runs 5 more tests and skips 10 fewer than the oracle (4146 vs 4141; 639 vs 649) — a
  data-provider or skip-condition difference, not investigated.
- Upstream's claim is "DBAL 3769 / 0 / 0", on an unstated older DBAL version. With 4.5.0 it is
  4146 / 10 errors / 1 failure. Upstream's Doctrine figure for the *ORM* suite is ~7.0× CPU; this
  DBAL run at 8.7× user CPU is in the same region. **Neither is near the 1.77× headline**, which
  is WordPress only.

### 1.2 Regression gate

- `baseline/gate.sh` re-runs `tests/` + `Zend/tests/` and fails on any pass→fail, pass→missing, or
  pass→skip of a test in `baseline/zend-tests.pass.txt` (3041 names). It reports new passes but
  never rewrites the baseline.
  **Exercised once on the untouched tree: PASS** (3041 → 3041, 0 pass→fail, 0 pass→skip, 0 new
  passes) — which is also a third identical run of the corpus.
- Upstream's `scripts/corpus-gate.sh` **could not be reused**: it hard-codes
  `/Volumes/Extreme Pro/…` and judges against a frozen fail-set in `wp109-harness/corpus-gate/`
  that is git-ignored and absent from the repository.
- `.github/workflows/phpt-regression-gate.yml` runs the gate in the same Docker image. **It has
  never run**: there is no fork remote. GitHub's hosted runners are amd64 and the baseline is
  arm64; the first run may need the baseline re-captured on the CI architecture.

## 2. Findings that contradict PLAN.md or upstream's documentation

1. **License.** PLAN.md calls upstream MIT. It is the **PHP License 3.01** (LICENSE, README,
   AGENTS.md, Cargo `license = "PHP-3.01"`). Clause 4 forbids "PHP" in the name of a derived
   product without written permission. See `NOTICE.md`. Needs an owner decision before anything
   is published.
2. **PLAN §3 option B is largely already built.** `Zval` is 16 bytes with inline scalars; strings
   are a single-allocation refcounted block with cached hash; arrays have packed/hash dual mode
   with Zend-style ordered entries; objects have slot tables. See `ARCHITECTURE_NOTES.md`.
3. **"Same public API" is not available.** The representation is the API: 4,611 `Zval::` sites
   and 347 hand-built `Zval::Array(Rc::new(…))` outside `php-types`.
4. **Composer does not run** under phpr today (smoke test a).
5. **`PHP_OS` is hard-coded to `Darwin`**; phpr reports it on Linux.
6. **`phpr -v` is not implemented.**
7. **`php-server --axum` passes no request data to PHP** — every superglobal is empty in that
   mode. Only the sequential single-threaded cli-server mode is a working SAPI.
8. **"No `unsafe`"** in upstream's README refers to VM control flow. `php-types` has 222 `unsafe`
   sites; the value model itself has 17 (`zstr.rs` 15, `array.rs` 2) with no `// SAFETY:` comments.
9. Upstream's process documents require MCP servers (Serena, Vexp) and a shell hook that are not
   in the repository, and instruct "commit and push every step". Not followed: the tools are
   unavailable, and upstream's remote is not ours to push to.
