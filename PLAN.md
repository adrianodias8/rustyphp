# PLAN.md — the original brief (2026-09-29): fork `phpr`, profile it, decide the kernel, aim at Drupal

> Kept as written for the record. Where it says MIT, the license is PHP 3.01 (see `NOTICE.md`);
> where it says "do not delete their docs", session 5 reversed that once the fork was published
> under its own name. Status of every phase: `NOTES.md`; the decision: `DECISION_KERNEL.md`.

You are working on a fork of `francescotinti/php-rust` (`phpr`, MIT): a from-scratch PHP 8.5 runtime
in Rust. We are keeping its correctness foundation (phpt oracle, operator differential harness,
Mago-based parser bridge, cycle collector, verified builtins and extensions) and changing two things:

1. **The performance strategy** — decided by a profiler, not by taste. Possibly a new value-model kernel.
2. **The target** — Drupal and Laravel, with a worker-mode server. Not multi-threaded shared-state PHP.

The upstream project's golden rule is also ours: **the oracle (real `php`) is always right.**
Never commit a pass→fail regression on the `.phpt` corpus or on any framework suite that is currently green.

Work through the phases in order. Tonight is Phase 0 and Phase 1. Phase 2 is a written decision, not code.

---

## 0. Ground rules

- Target **PHP 8.5** semantics (upstream already does). Oracle binary: a real `php` 8.5 CLI; `PHP_ORACLE` env var points at it.
- Read upstream's `CLAUDE.md`, `README.md`, `COVERAGE.md`, `HISTORY.md` and skim `diary/` **before touching code**. Their conventions apply until we consciously replace them. Do not delete or rewrite their docs; add ours beside them.
- Keep upstream attribution intact (MIT). Add `NOTICE.md` naming the upstream project and commit.
- Add nothing to `php-types` (the value model) in Phases 0–1. That crate is the subject of the Phase 2 decision; do not prejudge it.
- Every session ends with `NOTES.md` updated: what was measured, what changed, what's next. Numbers, not adjectives.
- Never optimise before measuring. Never redesign before profiling.

---

## 1. Phase 0 — Fork, build, baseline (tonight, first half)

Goal: our fork builds clean, all upstream suites run, and we have **committed baseline numbers**.

### 1.1 Fork and toolchain
- [ ] Fork `francescotinti/php-rust` to our org; clone; add `upstream` remote. Branch `main` tracks upstream; we work on `next`.
- [ ] Confirm `rust-toolchain.toml` (add one pinned to current stable if missing). `cargo build --release -p php-cli` succeeds. Note build time and binary size in `NOTES.md`.
- [ ] Install/locate the oracle: `php -v` must be 8.5.x. Record the exact version in `NOTES.md`.
- [ ] Clone `php-src` at the matching `PHP-8.5` tag into `../php-src` (or wherever upstream's scripts expect it).

### 1.2 Run everything upstream claims is green — and record it
- [ ] `cargo test` (all crates). Record pass/fail counts.
- [ ] `PHP_ORACLE=$(which php) cargo test -p php-types --test differential` — the 37,835-case operator differential. Must be 0 mismatches. Record.
- [ ] `cargo run --release -p phpt-runner -- ../php-src/tests ../php-src/Zend/tests` — record pass/fail/skip and the percentage. Save the failing-test list to `baseline/zend-tests.fails.txt` and the passing list to `baseline/zend-tests.pass.txt`.
- [ ] Run `phpt-runner` on `../php-src/ext/standard/tests` and each `ext/*/tests` for extensions upstream implements (`pcre json mbstring hash ctype dom simplexml pdo pdo_sqlite sqlite3 date spl reflection session filter tokenizer`). Record per-extension numbers in `baseline/ext.md`.
- [ ] Reproduce **one** of their real-world claims end to end as a smoke test, in this priority: (a) Composer `require monolog/monolog` under `phpr`; (b) PHPUnit on Doctrine DBAL. Record wall time under `phpr` vs oracle for the same run.
- [ ] Commit `baseline/` and `NOTES.md`. Add a CI job that fails if any test in `baseline/zend-tests.pass.txt` now fails (regression gate). If upstream already has this gate, reuse it and point it at our baseline files.

### 1.3 Read the code (write down what you find)
Write `ARCHITECTURE_NOTES.md` answering, with file/line references:
- [ ] How is `Zval` represented? Size in bytes (`std::mem::size_of` in a test). Which variants heap-allocate?
- [ ] How is `PhpArray` implemented? Is there a packed (dense int key) mode? Is it an insertion-ordered hash of our own, or `IndexMap`/`HashMap`? How are copy-on-write and "separation" done? How is the internal pointer / iteration-under-modification handled?
- [ ] How is `PhpStr` represented? Interned strings? Cached hash? Is it `Rc<[u8]>`, `Rc<Vec<u8>>`, something else?
- [ ] How are references (`&`) modeled? Objects — property slots vs dynamic table? Handlers vtable?
- [ ] How do mutable operations on shared arrays/objects work — `RefCell`, `Cell`, `get_mut` on `Rc`, unsafe?
- [ ] The VM dispatch: instruction encoding (enum? fixed-width?), operand kinds, how CVs/TMPs are stored, frame layout, how calls into builtins pass arguments (by value? slice of Zval? cloned?).
- [ ] How builtins are registered and what signature they have (this decides whether a kernel swap can be API-compatible).
- [ ] Where is `unsafe` used today, if anywhere? (`rg 'unsafe' php-rust/crates`)
- [ ] Is there any bytecode caching between runs? Between requests in `php-server`?
- [ ] `php-server`: request lifecycle — fresh runtime per request, or resident? Which SAPI globals (`$_SERVER`, `$_GET`, `$_POST`, `$_COOKIE`, `$_FILES`, `php://input`) are populated and how?

---

## 2. Phase 1 — Profile (tonight, second half)

Goal: a **profile that says where the 1.77x goes**. No code changes to hot paths yet, only instrumentation and benchmarks.

### 2.1 Benchmark set (`bench/`)
Create `bench/run.sh` that runs each of the following under both `phpr` and the oracle (`php -n -d opcache.enable_cli=0` **and** `php -d opcache.enable_cli=1`), 5 runs each, reporting median wall time, user CPU, and peak RSS (`/usr/bin/time -v`):
- [ ] `php-src/Zend/bench.php` and `php-src/Zend/micro_bench.php` (classic engine microbenchmarks — each sub-test reports its own time; capture them all).
- [ ] `bench/arrays.php`: build/iterate/modify 1M-element packed arrays; 200k string-keyed arrays; nested arrays with COW copies; `array_map/filter/usort` with closures.
- [ ] `bench/strings.php`: concatenation loops, `sprintf`, `str_replace`, `preg_replace`, `implode/explode`, `json_encode/decode` of a 5 MB structure, `serialize/unserialize`.
- [ ] `bench/oop.php`: 1M method calls, property reads/writes, `__get` magic, interface dispatch, closures with bound `$this`, exceptions thrown/caught 100k times.
- [ ] `bench/autoload.php`: Composer autoload + instantiate 2,000 classes from a generated package (autoloading + `include` cost).
- [ ] `bench/symfony-boot.php`: boot a minimal Symfony HttpKernel app and handle 200 requests in-process (upstream says http-kernel is green). This is the closest proxy to a Drupal/Laravel request until those run.
- [ ] Commit results as `bench/results/<date>.md` with the `phpr` git SHA and oracle version.

### 2.2 Profiling
- [ ] Build with `[profile.release] debug = 1` (symbols) and run `samply` or `perf record -g` on: `Zend/bench.php`, `bench/arrays.php`, `bench/oop.php`, `bench/symfony-boot.php`. Save flamegraphs (`perf script | inferno-flamegraph`) to `bench/profiles/`.
- [ ] For each profile, attribute time to buckets and fill in the table in `PROFILE.md`:
  - `Rc` increment/decrement and drop glue
  - `RefCell` borrow/borrow_mut checks
  - allocation/deallocation (`malloc`/`free`, `Vec` growth, `Box`)
  - hash table operations (insert/lookup/iteration) in `PhpArray`
  - string hashing/comparison/copying
  - VM dispatch loop itself (the `match`, frame push/pop)
  - argument passing into builtins (cloning `Zval`s)
  - the builtin bodies themselves
  - parser/HIR/compile (per-run compile cost with no bytecode cache)
  - other (name it)
- [ ] Count allocations per iteration for a simple `for` loop with integer arithmetic and one array write. If scalars or the loop counter allocate, say so loudly in `PROFILE.md`.
- [ ] Measure compile cost separately: time `phpr` on a 5,000-line file that does nothing vs the oracle with and without opcache. This quantifies what a bytecode cache would buy.

---

## 3. Phase 2 — The kernel decision (written, next session unless tonight finishes early)

Produce `DECISION_KERNEL.md`. It must contain the profile table, and one of these three outcomes, with reasoning tied to the numbers:

**A. Keep upstream's value model, optimise inside it.** Choose this if <25% of hot-path time is in `Rc`/`RefCell`/allocation attributable to the value representation itself, and the bulk is hash table, dispatch, string ops or compile cost. Then the plan is targeted: packed-array mode inside `PhpArray`, cached string hashes, interned literals, a bytecode cache, inline caches — all behind the existing API.

**B. New kernel behind the existing API.** Choose this if ≥40% of hot-path time is inherent to `Rc`+`RefCell`+heap scalars. Then design (do not yet implement) a replacement `php-types` with: 16-byte `Zval` with inline scalars; refcounted `ZStr` with interned flag and cached hash; `ZArr` with packed/hash dual mode and Zend-style ordered buckets; `ZRef`; `ZObj` with slot table + handlers; raw refcounts via `Cell<u32>`; `unsafe` confined to this crate with `// SAFETY:` on every block; and — critically — **the same public API upstream's ~500 builtins already call**, so they recompile unchanged. List every public item in today's `php-types` and mark it keep / change-signature / remove. Estimate the blast radius (how many builtin call sites break).

**C. Hybrid.** The realistic outcome if the profile is mixed: keep `Rc` for objects (rare, long-lived) and replace strings and arrays (hot, numerous) first, measured one at a time.

Also required in the decision doc:
- [ ] A one-page **bytecode cache design** regardless of A/B/C (serialize compiled units keyed by path+mtime+size; in-memory cache in `php-server`). This is almost certainly the largest single win vs `php` without opcache and it's independent of the kernel.
- [ ] A one-page **worker-mode design** for `php-server`: boot a script once, loop handling requests via a `Runtime` API (FrankenPHP-style `frankenphp_handle_request()` equivalent), with per-request reset of superglobals, output buffers, `$GLOBALS`, static-cache implications documented. Single-threaded per worker, N workers. No shared mutable PHP state across threads — we are explicitly *not* pursuing upstream's multi-threaded PHP direction.

---

## 4. Phase 3 — Implement the decision, gated

- [ ] Whatever A/B/C says, implement it in slices, each slice landing only with: `cargo test` green, differential at 0 mismatches, `.phpt` regression gate green, and `bench/run.sh` showing no slowdown on any benchmark (record every run in `bench/results/`).
- [ ] Bytecode cache first (it's independent and wins immediately).
- [ ] Then the kernel work per the decision.
- [ ] Then worker mode in `php-server`.
- [ ] Target at end of phase: within 1.2x of `php` with opcache on `bench/symfony-boot.php`, and faster than `php -n` (no opcache) on everything. Record.

---

## 5. Phase 4 — Drupal

Exit: `drush site:install standard` succeeds against SQLite, then MySQL; the Drupal front page renders byte-identically to the oracle (diff the HTML with cache-busting tokens stripped); `core/phpunit.xml.dist` `Unit` suite passes at a reported percentage.

Extensions Drupal needs beyond what upstream has (verify each against `ferro -m` first):
| Extension | Approach |
|---|---|
| `intl` | Bind ICU (`rust_icu` or direct FFI). `Collator`, `NumberFormatter`, `IntlDateFormatter`, `Normalizer`, `Transliterator`, `Locale`, `MessageFormatter`. Match ICU version behaviour of the oracle's build. |
| `gd` | Bind libgd. Drupal's default image toolkit. Alternative later: pure-Rust subset with `image`. |
| `pdo_mysql`, `pdo_pgsql` | On upstream's native PDO layer, reusing their mysqli wire-protocol code for MySQL. Exact `SQLSTATE`, `ATTR_EMULATE_PREPARES` semantics, `MYSQL_ATTR_*`. Postgres via `postgres` crate (sync). |
| `session` | Files handler, `SessionHandlerInterface`, cookie params, `session_regenerate_id`, strict mode. Drupal has its own handler but the extension must exist. |
| `filter` | `filter_var` and friends with every flag. |
| `tokenizer` | `token_get_all`/`PhpToken` over Mago tokens with PHP's token names/ids. Drupal's annotation parser and Twig use it. |
| `iconv`, `zlib`, `fileinfo`, `bcmath` | Native / small bindings. |
| `xml`, `xmlreader`, `xmlwriter`, `libxml` | Extend upstream's Rust DOM, **or** add a libxml2-bound variant behind a feature flag if Drupal's tests reveal quirk mismatches. Decide by test failures, not up front. |
| `apcu` | Per-process KV first (Drupal's `apcu` cache backend is optional but common). |

Also expected: heavy exercise of the stream layer (`file_put_contents` with locks, `stream_wrapper_register` — Drupal registers `public://`, `private://`, `temporary://` wrappers), `Serializable`/`__serialize` on service container dumps, Reflection on attributes, `Symfony\Component\DependencyInjection` PHP-dumped containers (large generated files — a good compile-cost benchmark).

Order of attack: run Drupal's install with `PHP_RUST_TRACE` off and collect the first fatal; reproduce it as a minimal `.phpt` against the oracle; fix; repeat. This is upstream's loop and it works.

---

## 6. Phase 5 — Laravel (parallel with Drupal once Phase 3 lands)

Exit: fresh `laravel new` app serves `/` via `php-server` in worker mode; `php artisan test` on the skeleton passes; Laravel's own framework test suite runs with a reported percentage.

Mostly the same extensions as Drupal plus `bcmath`, `sodium` (bind libsodium), `redis` optional. Laravel leans harder on `proc_open`/`symfony/process` (upstream has `proc_open`), `pcntl` for queue workers, and `mbstring` grapheme functions.

---

## 7. Phase 6 — Speed, properly

Only with Phases 3–5 green and benchmarks recorded:
- Inline caches for property/method lookup; specialised int/int and string-concat opcodes; superinstructions from profile data.
- Drupal front page and Laravel welcome page under `wrk` in worker mode vs `php-fpm`+opcache and FrankenPHP worker mode. This is the headline benchmark; everything before was a proxy.
- Cranelift tier only if the VM dispatch bucket in the profile still dominates after the above.

---

## 8. Conventions (copy into our section of CLAUDE.md)

- Upstream's `CLAUDE.md` rules stand unless overridden here.
- Every commit: `cargo test` green; differential at 0; `.phpt` regression gate green; if it touches `php-types`, `php-runtime` VM, or builtins' arg handling, also run `bench/run.sh` and paste the delta into the commit message.
- New builtins are registered only after being verified against the oracle with a `.phpt` (upstream rule; keep it).
- Never use `String`/`str` for PHP values. Never use `HashMap` for PHP arrays.
- `unsafe` only in `php-types` and only if `DECISION_KERNEL.md` chose B or C; every block has `// SAFETY:`.
- Profile before optimising; measure after. A speedup claim without a `bench/results/` entry is not a speedup.
- Do not pursue multi-threaded shared-state PHP. Worker mode is single-threaded per worker.
- Keep the fork mergeable with upstream for as long as the kernel decision allows: rebase `next` onto upstream `main` weekly until the kernel diverges, then cherry-pick their builtin/extension fixes.
