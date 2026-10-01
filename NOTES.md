# NOTES.md — session log of the fork

Numbers, not adjectives (PLAN.md §0). Newest session first. Every figure below was measured in
this session unless it is explicitly labelled "upstream's claim".

---

# Next session — the owner's plan (given 2026-10-01), in order

1. **Drupal 11 standard install under ferro**, one-shot cli-server mode, SQLite:
   `drush site:install standard`. Fix each first fatal with an oracle-validated `.phpt`. Record
   every missing extension/function in `MISSING_FOR_DRUPAL.md`.
2. **Drupal front page byte-identical to the oracle** (strip tokens), then run it in worker mode.
   Document every piece of Drupal state that leaks between requests; do not patch Drupal — find
   what a worker needs to reset.
3. **wrk**: Drupal front page, anonymous, page cache off — ferro worker vs php-fpm+opcache vs
   FrankenPHP worker. Tune FrankenPHP (`num_threads`, `GOMAXPROCS`) until it scales, and note the
   config.
4. If a bare-metal Linux box is available, re-run the Symfony wrk suite there (needs the owner:
   no such machine is reachable from this environment).

**No engine micro-slices until step 3 has numbers.**

---

# Session 8 — 2026-10-01 — Drupal under wrk (step 3), classic-mode speed

**Step 3 numbers** (front page, anonymous, `page_cache` uninstalled; `bench/drupal/bench-wrk-drupal.sh`,
12-CPU VM shared with wrk; every arm's body identical to php-fpm's, token-stripped):

| workers | php-fpm + opcache | FrankenPHP worker | ferro worker |
|---|---:|---:|---:|
| 4 | 732.77 | 196.42 | 57.22 |
| 8 | 1415.78 | 369.15 | 114.50 |
| 12 | 1623.49 | 460.92 | 148.55 |

FrankenPHP: `num_threads` 8/16 and `GOMAXPROCS` changed nothing (~240 req/s at 4 workers);
`num_threads` = workers does not start. The ferro worker degrades within a run (105 → 55 req/s
over four 15 s runs, GC share 4 % → 20 %) even with the `recipe` reset (which now also clears
`ReverseContainer::$recordedServices`, +245 entries per request in both engines). Cause open.

**Classic (one-shot, php-fpm-like) mode — the owner's priority.** A warm one-shot request took
166 ms (oracle `php -S`, no opcache: 8–10 ms). Profile: 64 % in `Vm::run_deferred` — every class
of a defer-always unit was re-parsed, re-lowered and re-compiled on every request although the
unit cache hit. Fixes (`bench/drupal/oneshot-time.sh`, warm, same box):

| change | ms / request |
|---|---:|
| before | 166 |
| deferred declarations cached in `UNIT_CACHE` (`vm/defercache.rs`, keyed by file/line/snippet digest, valid per `unit_fp`) | 75 |
| + negative cache: a (snippet, fp) whose lowering stopped on a missing supertype goes straight to autoload | 33 |
| + per-thread regex cache (ext/pcre's is per process) and a precomputed snippet digest | 30 |
| + realpath cache for include resolution (positives only, `realpath_cache_ttl` 120 s) | 29 |

New `ferro -S … router.php --workers N` (no `--worker`): N threads, fresh `Vm` per request, caches
per thread. wrk, 8 workers (`bench/results/2026-10-01-drupal-wrk-classic-w8.md`):
php-fpm 1355 req/s, **ferro-classic 219** (was ~34 at 166 ms × 8 threads), ferro-worker 150.
What is left is the interpreter itself (`run_loop`, calls, autoload, unserialize): ~3× Zend per
request with compilation gone. The remaining gap to php-fpm is ~6×.

**Second pass (owner: "adapt from opcache; work on the dispatch loop").** Tools: a frame-pointer
build (`RUSTFLAGS="-C force-frame-pointers=yes" CARGO_TARGET_DIR=/target/fp`) with
`CG=fp bench/drupal/oneshot-profile.sh` and `bench/drupal/fold.py` (perf script folded once, then
self / inclusive / callers in seconds; DWARF unwinding of the same data timed out at 30 min).
`bench/ab-summary.py` prints `ab.sh`'s per-section geomeans.

Kept (8 alternating rounds, `oneshot-time.sh`: 2a8f4606 28.55 ms → 25.68 ms, −10 %):
- `unit_fp` hashed every global name and trait key on each of ~1,100 includes/deferred fires per
  request (2.7 %): ordered prefix digests extended in place (both lists are append-only).
- `run_linked` re-checked the 112 shared prelude functions on every link, with a byte compare of
  the file name each: the shared prefix is now skipped by pointer comparison.
- Four trait files (a trait that `use`s a trait) missed the unit cache on every request — the
  lowering autoloads mid-way, which marked them impure. Each attempt's "needs X" is now recorded
  per fingerprint, the next load autoloads X first and probes again, and the unit is published
  under its final attempt's fingerprint; the include's chain event moved after the lowering so
  hit and miss stay aligned. Warm requests: 0 unit-cache misses.
- opcache's `revalidate_freq`: include stat at most every 2 s under the server
  (`PHPR_REVALIDATE_FREQ`; 0, i.e. every include, in the CLI).

Measured and dropped:
- **Dispatch fast path** (slice 6, safe Rust): an inner loop keeping `ip`/frame in locals for the
  hottest simple ops (op-census of a Drupal request: 409k ops; `Sweep` 13.5 %, `LoadVar` 11.4 %,
  `PushConst` 7.9 %, `JumpIfFalse` 6.9 %, `StoreSlot` 5.4 %). Entered per op from the loop head:
  `empty_loop` 1.35×, `nestedloop` 1.27× (fused loop ops never take it, but pay the entry).
  Entered only from the fast ops' own arms: micro rows 1.05–1.5× worse, Drupal unchanged — runs of
  fast ops are one or two long before a heavy op. Drupal's `run_loop` self time is 16 %, ~9 ns of
  dispatch per op; the time is in handlers, not dispatch.
- **Hashed property layout** (`PropsLayout::slot_of`, a hash-vector scan): no Drupal change
  across alternating rounds, and 6–9 % slower on `magic_get_1m`/`getter_setter_fluent_1m`/
  `prop_rmw_1m` even with the probe out of line (their classes have 3 properties and never take
  it — a code-layout effect). Bisected and reverted.

wrk, 8 workers (`bench/results/2026-10-01-drupal-wrk-classic2-w8.md`): php-fpm 1354 req/s,
ferro-classic **248** (was 219).

Third pass: **single-pass `unserialize()`** (`vm/unser.rs`: allocation-free validation, then
`Zval`s straight from the bytes; `R:`/`C:` payloads keep the tree path; byte-level integer
parsing — `from_utf8` + `str::parse` was ~30 % of it) — 584 KB synthetic payload 3.54 → 2.68 ms
(PHP 1.23), `strings/unserialize_x5` 0.733; `Key::from_bytes` no longer copies a string key twice.
**`MethodIc` keyed on (receiver class, calling scope)**: resolution, private rebind and visibility
are pure in that pair, so protected/private calls are cached too (before: public,
scope-independent winners only). Drupal warm request ~25.9 → 24.2 ms; wrk 8 workers:
ferro-classic **271** req/s, php-fpm 1280 in the same run
(`bench/results/2026-10-01-drupal-wrk-classic3-w8.md`).

**Fourth pass — what overtaking php-fpm would take.** Per-feature ratios against PHP 8.5.7 (CLI,
no opcache; `bench/*.php`): method call 16.6×, property write 18.8×, `instanceof` 12.3×, function
call 9.7×, `new` 8.9×, closures 8–10×; arrays 2–5×, strings 1.4–5×. In a function, an empty
`for` iteration is 8.5 ns vs 2.1 (two ops), a call 72.6 ns vs 10.0. Drupal's self time by
subsystem (`bench/drupal/buckets.py`): dispatch 17 %, objects/props 9 %, Zval clone/drop 8 %,
arrays 7 %, alloc 7 %, strings 5 %, frames 5 %, GC emulation 4 %, hashing 4 %, unit link 4 %,
unserialize 3 %, methods 3 %, other 18 % — flat: parity needs every row ~4× smaller.
`bench/drupal/hotlines.sh` (objdump line tables; perf annotate crashes here) on a call loop:
the hot instructions are the per-op prologue's dependent loads (`frames` ptr/len → frame →
`func` → ops) and spills into `run_loop`'s >10 KB stack frame (a ~600-arm `match`).

Tried and reverted this pass:
- A `FrameStack` wrapper whose epoch lets `run_loop` cache `top`/`func` across ops: function
  calls 74 → 83 ns, empty loop 8.7 → 9.3 ns (the push/pop bookkeeping and the codegen shift cost
  more than the two loads saved).
- Skipping the `$this` GC note at frame exit when the object is already light-demoted and held
  elsewhere: method calls 121 → 101 ns and the gate stays green, but `PHPR_GC_VERIFY` on the 240
  php-src `__destruct` tests shows a new under-note (`magic_methods/bug29368_3`: an operand-stack
  temp dying in an unwind is no longer seen by the current sweep) — the WP-21 window. Reverted.

**Fifth pass — can safe Rust get there? (owner: "do things better the safe way before deciding a
rewrite or unsafe").** `bench/proto/` runs the same tiny bytecode (ferro-style fused ops, 16-byte
`Zval`) under two layouts, no `unsafe` (`CARGO_TARGET_DIR=/target/proto cargo build --release`):

| per iteration | ferro | proto A (ferro's layout) | proto B (contiguous stack, locals) | B3 (B + unchecked, info) | PHP |
|---|---:|---:|---:|---:|---:|
| empty `for` | 8.5 ns | 4.7 | 3.7 | 3.5–4.3 | 2.1 |
| `$x = f($x)` | 74 ns | 35 | 18 | 19 | 10 |

The layout alone halves a call in safe Rust; unchecked indexing buys nothing measurable. The real
engine costs ~2× prototype A on top (GC notes and per-statement `Sweep`, frame flags/ext, the
600-arm `match`'s spills).

Drupal phases (`bench/drupal/phases.php`, warm, ms): PHP (no opcache) autoload 0.06, boot 1.2,
**handle 3.9**, send+terminate 3.7–7.5; ferro 0.27, 1.9, **18.7**, 3.1–7.9. Bootstrap is small on
both: worker mode or engine-level request scoping (the "structured state" idea of the PHP
Structured Concurrency series, hook-dev-alter.com, Sep 2026 — boot once, process-wide state
shared, request state in a per-request store that starts clean) saves ~2 ms at most here. The gap
is execution inside `handle()`, 4.8×.

Profile now (frame pointers, warm request): `run_loop` self 16 %, `Zval` drop/clone 7 %, allocator
5 %, `resolve_method_runtime` 1.8 % (half from `dispatch_instance_call`), property resolution
(`resolve_prop_access` + `PropInfo` map) ~3 %, `unserialize` 11 % inclusive (two allocations
per value: an intermediate `Ser` tree, then the `Zval`), fibers 10 % inclusive (Drupal's
renderer), SQLite schema load 2 % (per connection, as in PHP).

---

# Session 7 — 2026-10-01 — Drupal 11: install, front page, worker mode (steps 1–2)

Plan: the owner's four steps at the top of this file. Steps 1 and 2 done; step 3 next.

1. **Install** (`bench/drupal/install.sh`): `drush site:install standard` on SQLite completes under
   ferro, 14.5 s vs 2.1 s for the oracle. Same diagnostics in the log; the two databases are
   equivalent (same 39 tables and row counts; `config` identical once UUIDs are normalised; the
   rest differs only in timestamps, the cron key, the CSS/JS query string and the password salt).
   28 fixes on the way — `MISSING_FOR_DRUPAL.md` rows 1–28, each with its `.phpt`. The largest:
   **native fiber stacks** (corosensei, owner decision; `Fiber::suspend` from inside a generator
   or callback panicked), `__serialize()`-without-`__unserialize()` round trips (Symfony DI lost
   every `ChildDefinition`'s class), lazy `RecursiveIteratorIterator`, user stream wrappers
   (`file_put_contents`, chmod/touch/unlink/rename/mkdir/rmdir), static-property reference
   binding and return.
2. **Front page** (`bench/drupal/frontpage.sh`, base `/scratch/drupal-base`, see
   `bench/drupal/README.md`): `/`, `/node`, `/user/login` byte-identical to the oracle's `php -S`,
   headers included, after token stripping (4/4). Rows 29–36 (first-class callable autoload,
   preg byte offsets on multibyte subjects — Twig lexer, `strtr` with Stringable values, `$this->p`
   arguments to array callables, references through argument unpacking — Olivero preprocess,
   `LIBXML_NOEMPTYTAG` — the active menu link, duplicate `Date`).
3. **Worker mode** (`bench/drupal/drupal-worker.php`, `bench/drupal/worker-leaks.sh`): the naive
   loop fails the same way under FrankenPHP and ferro (Drupal state, not the engine). A worker
   needs `$kernel->resetContainer()` + a reset of `Renderer::$contextCollection`,
   `Html::$seenIds`, `Html::$seenIdsInit` after `terminate()` — then 10/10 responses are
   byte-identical to one-shot PHP. Table and the full leak map in `MISSING_FOR_DRUPAL.md`
   § Worker mode. Rows 37–43 found on the way (reflection of static properties, nested writes
   through `WeakMap`/`&offsetGet`, `touch()` on a directory, xmlns-first serialisation, …).
4. Corpus 3,051 → **3,100** (48 + 1 new passes, baseline advanced, 0 pass→fail). DBAL: the 10
   errors are gone (4,146 tests, 1 failure — the oracle's same 1). New divergences D-17…D-23.

Measured, not yet tuned: one-shot ferro serves the warm front page in 0.28 s vs 0.008 s for
`php -S` (every request recompiles Drupal); worker mode is what step 3 measures.

---

# Session 6 — 2026-10-01 — Ferrophant rebrand; worker recycling and the soak test

1. **Rebrand** (`4583e8f1`): binary `ferro`, builtin `ferro_handle_request`, comments in English,
   new README / CLAUDE.md / NOTICE.md, `tests/loc_cap.rs`. Gate: 3,048 = baseline, 0 pass→fail.
2. **`--max-requests N`** for `ferro -S --worker`: after N requests `ferro_handle_request()`
   returns false, the script returns and the thread restarts on a fresh `Vm`.
3. **Soak test** `bench/worker/soak.sh` — numbers in `bench/results/2026-10-01-soak.md`. No
   per-request leak (hello 8.7 M requests, Symfony 1.58 M; RSS flat after warm-up).
4. **Bug found and fixed**: the first recycling run panicked on every request after a restart —
   the GC walk epoch was per-`Vm` while unit-cache arrays outlive the `Vm`. Now thread-local.
   No `.phpt` (it needs two `Vm`s on one thread); the soak run with `MAXREQ` is the regression check.
5. Gates: `cargo test --release` 1,749 passed / 1 failed (root-only logging test); gate PASS
   (3,048, repro 14/14, divergences 12 failing + 2 skipped); isolation diffs = the three
   documented in session 5 §2.

6. **D-04 fixed**: builtin arity errors (`… expects exactly N argument(s), M given`) are raised as
   plain `Error` from dozens of sites; `PhpError::class_name()` now maps that wording to
   `ArgumentCountError` (catch and render both go through it).
7. **D-02 fixed**: `to_zstr` warns on NAN like Zend's `zval_get_string` (concat, interpolation,
   string params, `print_r`, `implode`); the float-offset path renders its message without it and
   gained PHP 8.5's "float … is not representable as an int" warning.
   The operator differential missed every warning of a compile-time-folded expression (emitted
   before its handler exists); operands now pass through `__v()`, oracle run with
   `memory_limit=-1`. Still 37,835 cases, 0 mismatches.
8. New divergences: D-15 (builtin frame missing from a trace), D-16 (float-offset diagnostics
   reported on the next statement's line — upstream does the same). The `PHPR_REG_LOWER`
   bad-value warning was still in Italian; translated. Other Italian string literals (mostly test
   assertion messages) remain; a sweep is open.
9. Corpus 3,048 → **3,051** (bug46106, argument_count_incorrect_internal,
   float_to_int/union_int_string_type_arg), baseline advanced; 0 pass→fail.

10. **Kernel decision: outcome A, decided by the owner** (DECISION_KERNEL.md, status DECIDED, §7
    caveat status, new §8 safety policy). Safety pass: `SAFETY:` comments on every value-model
    `unsafe` block; `tests/unsafe_census.rs` pins 280 `unsafe` lines in 23 files; Miri in CI.
    Miri found that `PhpStr::as_bytes` is UB under Stacked Borrows (payload read through a
    header-sized reference); Tree Borrows accepts it, and all 32 zstr/array tests pass under it.
11. CI rewritten (one `ci.yml` in the dev image: build, tests, corpus gate, Miri); first run
    green on amd64, gate identical to the arm64 baseline (3,051).

Not done: a memory ceiling. The soak shows no growth to bound; with all workers in one process an
RSS ceiling could only recycle blindly. Revisit if a real application grows.

---

# Session 5 — 2026-09-30/10-01 — fork published, divergence register, worker mode, wrk

## 1. Housekeeping

- The fork now lives at **github.com/adrianodias8/rustyphp** (public; `origin`), branches `main`
  (upstream's, unchanged) and `next`. Nothing is pushed to upstream, ever.
- The case-insensitive builtin lookup (`StrToUpper`, `COUNT`) asked for in step 2 was already
  `e48f4910` (session 4), with `builtin-names-case-insensitive{,-ns}.phpt`; it went out with the
  first push.
- **`KNOWN_DIVERGENCES.md`** (`609e3188`): every "found, not fixed" divergence of sessions 1–4,
  one row each, one minimal failing `.phpt` each under `baseline/divergences/` — all PASS on the
  oracle under php-src's `run-tests.php`, all fail or skip on phpr. `baseline/gate.sh` runs them
  and names any that starts passing (promotion to `baseline/repro/` is then deliberate). Two
  session-2 flags did not reproduce and are closed there. D-14 (`ini_set('precision')` has no
  effect) was found by this session's isolation battery.

## 2. Worker mode (`2bda4f2e`, DECISION_KERNEL.md §5)

`phpr -S host:port --worker worker.php [--workers N]` and one host builtin:

```php
$kernel = build_kernel($dir);                        // boot once
while (phpr_handle_request(function () use ($kernel) {
    $response = $kernel->handle(Request::createFromGlobals());
    $response->send();
})) { gc_collect_cycles(); }
```

The same script runs unchanged under FrankenPHP (`frankenphp_handle_request`). N OS threads, one
`Vm` each; one lightweight thread per connection parses HTTP and queues the request, the workers
pull from the queue and reply through a channel — connections outnumber workers freely, keep-alive
is served request after request, and (found on the first attempt, where a worker owned its
connection) no connection can starve another. The engine side (`vm/worker.rs`) never sees a
socket: the host installs two closures on the worker's thread.

Per request: output, headers, diagnostics and superglobals reset and reseeded; shutdown functions
run and cleared; session written and closed; error/exception handlers and the ini table restored
to the post-boot snapshot; upload temp files deleted. Kept, as documented: globals, statics,
static properties, objects, **and the object/resource id counters** — this is a subset of
`request_end()` taken against a snapshot, as §5 requires, not `request_end()`. An uncaught
throwable or `exit` ends the request, not the worker; the status is 500 only when
`display_errors` is off (php_error_cb's rule — the cli-server SAPI answers 200 with the error in
the body, like PHP's).

Gate (`bench/worker/isolation.sh`): sixteen routes through one worker across the sequence, the
same routes through the one-shot cli-server, and through the oracle's `php -S`. Worker vs
one-shot differ in exactly: `SCRIPT_NAME` (`/index.php`, FrankenPHP's convention, vs the router
script's request path), the extra closure frame in an uncaught-exception trace, and `/stateful`
(statics, a boot-time object) by design. One-shot phpr vs oracle: only D-14. The Symfony worker's
five benchmark responses are byte-identical to the oracle's.

## 3. HTTP throughput under `wrk` (step 5) — `bench/worker/bench-wrk.sh`

Three servers on one docker network, `wrk` in a fourth container, the same `vendor/` and the same
front controllers (`bench/worker/`), responses checked against php-fpm's before measuring, 3 s
warm-up, medians of 3 × 15 s. `phpr-worker` = this fork's worker mode; `fpm` = nginx 1.31 →
php-fpm 8.5.7, opcache on (`validate_timestamps=0`, JIT off), static pool; `frankenphp` =
FrankenPHP 1.12.7 (PHP 8.5.11) worker mode. The Symfony app is `bench/symfony-boot.php`'s
HttpKernel app (three routes + 60 filler routes), `wrk` cycling the benchmark's five paths; hello
is `echo "Hello, world"`. 12 vCPUs in the VM shared by everything.

**4 workers each, 32 connections, 2 wrk threads** — `bench/results/2026-09-30-wrk-w4.md`:

| | server | req/s | p50 | p99 | phpr ÷ this |
|---|---|---:|---:|---:|---:|
| hello | nginx + php-fpm | 36,007 | 0.85 ms | 1.54 ms | **4.57** |
| hello | FrankenPHP worker | 24,446 | 1.19 ms | 4.48 ms | **6.72** |
| hello | phpr worker | 164,383 | 0.16 ms | 1.74 ms | 1 |
| Symfony | nginx + php-fpm | 13,580 | 2.26 ms | 3.83 ms | **1.08** |
| Symfony | FrankenPHP worker | 23,175 | 1.26 ms | 4.22 ms | **0.63** |
| Symfony | phpr worker | 14,634 | 2.17 ms | 2.42 ms | 1 |

**8 workers each, 64 connections**, medians of 2 × 10 s — `bench/results/2026-09-30-wrk-w8.md`:

| | server | req/s | phpr ÷ this |
|---|---|---:|---:|
| hello | nginx + php-fpm | 47,799 | 4.01 |
| hello | FrankenPHP worker | 5,495 ¹ | — |
| hello | phpr worker | 191,475 | 1 |
| Symfony | nginx + php-fpm | 20,755 | **1.27** |
| Symfony | FrankenPHP worker | 23,347 | **1.13** |
| Symfony | phpr worker | 26,354 | 1 |

¹ FrankenPHP's hello arm is bimodal in this VM: 24.4–24.8 k req/s in four isolated 15 s runs and
in the 4-worker record, 10–17 k in three other harness runs, 5.5 k with 8 workers — while its
Symfony arm is stable at 22–23 k. Not understood (it is Caddy/Go under Docker Desktop's network on
macOS; the phpr and fpm arms never moved more than 4 %). Its hello ratio is quoted from the
record run; the 8-worker hello figure is not a FrankenPHP ceiling.

What the numbers say:

- **Hello** is the SAPI floor: phpr's thread-per-connection front end and in-process request
  loop answer in 24 µs per request per worker (164 k/s ÷ 4); php-fpm pays the nginx hop and
  FastCGI, FrankenPHP the Caddy→PHP hand-off.
- **Symfony** is where the interpreter shows. Per worker: phpr ≈ 273 µs/request, FrankenPHP
  ≈ 173 µs, php-fpm (with nginx) ≈ 295 µs. In-process, the same request loop is 230 µs on phpr
  against 30 µs on `php -n` (`handle_requests`, session 4 §2c) — **the 7.9× gap of the kernel path
  shrinks to 1.6× behind HTTP**, because 140 µs of every FrankenPHP request is SAPI work
  (superglobals, `Request::createFromGlobals`, `Response::send`) that phpr's leaner loop does in
  ~45 µs. With 8 workers phpr scales 1.8× and passes both (FrankenPHP does not scale here with its
  default thread configuration; its ceiling in this VM is not established).
- PLAN §4's target for worker mode was "within 1.2× of php with opcache on `bench/symfony-boot.php`":
  against php-fpm + opcache the worker is at **0.93× (4 workers) / 0.79× (8 workers)** of its
  time per request, i.e. inside the target; against FrankenPHP it is at 1.58× / 0.89×.

## 4. What was not done, and why

- **The on-disk bytecode cache (DECISION_KERNEL.md §4 step 3).** Step 1 was slice 2; step 2 — a
  process-wide in-memory cache — is what upstream's unit cache already is inside a worker: in
  worker mode nothing is lexed, parsed or compiled after boot (includes hit the thread's unit
  cache, the main unit runs once). Step 3 would serve the one-shot CLI (the 15.8 ms startup floor,
  9.5 ms of it the prelude, plus 1.35–1.5 ms per 1,000 lines) and worker boot; it needs a
  serialisation of `Module` (203 op variants with `Rc`-shared inline caches), `Func`, `Const`,
  `CompiledClass` and the HIR fragments a `Module` embeds, and it must preserve the `Rc` identities
  `run_linked` and `relocate_module_class_ids` test (`Rc::ptr_eq`, `Rc::get_mut`) or fall back
  correctly. It does not change any step-5 number, so it was sequenced after them; it is the next
  piece of work if the CLI floor matters.
- Worker mode has no `max_requests` recycling and no memory ceiling yet (§5.5); the isolation
  battery is the gate, a long-run leak test (10⁶ requests, RSS sampled) is not written.

## 5. Commits

| commit | content |
|---|---|
| `609e3188` | `KNOWN_DIVERGENCES.md`, `baseline/divergences/`, gate reporting |
| `2bda4f2e` | worker mode, `bench/worker/`, wrk in the dev image |
| (this one) | this section, D-14, `bench/results/2026-09-30-wrk-w{4,8}.md`, decision doc §5/§6 status |

---

# Session 4 — 2026-09-30 — the three namespaced-call divergences fixed; slice 4 (`foreach`)

Same environment. `/target/prev` was rebuilt from the commit each change started from
(`e38b1997`, then `e48f4910`); every A/B below is against it.

## 1. The three divergences of session 3 §5 — one root, fixed together (`e48f4910`)

All three sat in the unqualified call inside a namespace (`sprintf(…)` in `namespace App`).
Before designing anything the extent was measured: every function both engines define (762 names
in common, plus `assert`) was declared as `App\<name>(...$a) { return 'S'; }` by `eval` and then
called unqualified, one process per name (`probe.tmp/run.sh`, not committed).

| | oracle | phpr before | phpr after |
|---|---:|---:|---:|
| names whose namespaced shadow is called | 762 of 763 | **426** | 763 |

The 336 that could not be shadowed were every *host* builtin (`sprintf`, `array_map`,
`is_callable`, `json_decode`, `preg_*`, `date_*`, `curl_*`, `xml_*`, `session_*`, …), every
by-reference builtin (`sort`, `array_push`, `preg_match`, `sscanf`, `array_multisort`, the
`reset`/`end` family) and every prelude / global user function — all bound statically by
`compile::call` when the name was not a *registry* builtin. phpunit-bridge's ClockMock and DnsMock
shadow exactly these (`time`, `microtime`, `sleep`, `date`, `checkdnsrr`, `gethostbyname`, …).
The one name PHP itself refuses to declare in a namespace is `assert`; phpr accepts it (residue).

What changed:

1. **Sticky binding.** `NsIc` is keyed on the run epoch alone: the first execution of a site
   decides, as Zend's `INIT_NS_FCALL_BY_NAME` runtime-cache slot does. `fn_gen` and the
   running-module key are gone — the module key made a unit function's site re-resolve whenever it
   was called from another unit (found by the re-include probe). A unit run again in the same
   request (repeated `include` served by the unit cache, same `Module`) has the sites of its
   top-level code and closures reset (`Module::reset_ns_sites`, `Vm::units_run`): PHP compiles a
   fresh op array per include; the functions it declared keep their bindings.
2. **By-value host builtins** compile to `Op::CallNsFallback` like registry builtins, with a new
   target `NsTarget::FallbackHost(canon)` that dispatches directly with `Op::CallHostBuiltin`'s
   epilogue. The site helper now pops up to four arguments straight into a stack buffer for any
   builtin target (no `Vec`), as the two static call ops do.
3. **Everything whose static sequence depends on the callee's shape** — out-param, ref-first and
   scanf host builtins, `array_multisort`, registry `RefFirst` builtins, a spread on a host
   builtin, a global user or prelude function — goes behind a new op, `Op::NsShadowGuard`:
   `guard → user; <static sequence>; Jump end; user: <dynamic call by the namespaced name>; end:`.
   The guard runs *before* the arguments (Zend's order) and binds on its first execution. The
   arguments are compiled once per branch and evaluated on one — checked with side-effecting
   arguments. Cost: two op dispatches per such call in namespaced code, and the argument
   bytecode twice for those sites. Registered in census (`N_OPS` 201 → 202), `reg_lower`,
   jump threading and liveness like session 2's `PropConcatGate`.
4. **Case folding.** `compile::call` folds the name once (the tables are lowercase);
   `invoke_named` and `is_name_callable` look the registry up through `LcKey`;
   `ReflectionFunction` reports an internal function's lowercase name.

Verification, all byte-identical to the oracle in the default, `PHPR_REG_LOWER=1` and
`PHPR_UNIT_CACHE=0` modes: the session-3 probe (now identical on every line, including the
mixed-case one), a case probe in the global namespace and in a namespace, a guard probe (every
by-reference family with no shadow, argument side effects, spreads, shadows with by-reference
parameters, sites that already ran, fully qualified names, named arguments, methods / closures /
generators, undefined-then-declared), a re-include probe (top-level code and closures rebind, a
unit function keeps its bindings, a binding made inside a unit holds when called from main).
Committed as four `.phpt`s: `ns-shadow-builtins`, `ns-reinclude-binding`,
`builtin-names-case-insensitive`, `builtin-names-case-insensitive-ns`.

One divergence seen and left, now `KNOWN_DIVERGENCES.md` D-04: a builtin called with the wrong
number of arguments throws `Error` where PHP throws `ArgumentCountError`. `assert` being
declarable in a namespace is D-05.

Gates: `cargo test` 1749 / 1 (root-only) / 2; LOC caps declared; phpt gate **3047 → 3048**
(`Zend/tests/frameless_jmp_004`: a case-folded call to a namespaced shadow), 0 pass→fail, repro
11/11; baseline lists advanced with `run-baseline.sh` — `ext/spl` +2 passes (`bug31185`,
`bug36287`), `ext/standard` `serialization_objects_016` skip→pass and `image/bug13213` skip→fail
(`GetImageSize` now resolves, the JPEG comment case behind it does not); Composer and DBAL
unchanged.

A/B `e38b1997` → `e48f4910`, 7 rounds, 123 sections — `bench/results/2026-09-30-ab-ns-binding.tsv`:

| benchmark | sections | geomean B/A | min | max |
|---|---:|---:|---:|---:|
| `Zend/bench.php` | 18 | 1.003 | 0.973 | 1.143 |
| `Zend/micro_bench.php` | 35 | 1.005 | 0.957 | 1.110 |
| arrays | 20 | 0.997 | 0.968 | 1.039 |
| strings | 21 | 0.999 | 0.971 | 1.019 |
| oop | 21 | 0.991 | 0.955 | 1.009 |
| autoload | 3 | 1.002 | 0.966 | 1.037 |
| symfony-boot | 3 | 0.997 | 0.953 | 1.045 |

- Symfony `handle_requests` 46.2 → 45.9 ms (0.995): the guard and the host-builtin route cost
  nothing measurable on the request path.
- `ary2(50000)` 7 → 8 ms is the 1 ms timer. `$x = $str[0]` 1.110 in this run, 1.038 in a 9-round
  rerun, 0.992 against the null-lever build. `int_func()` (`strlen("hello")` in the global
  namespace, a path this change does not touch): **1.058, 1.064, then 0.977** in three prev-vs-new
  runs and 0.925 in the slice-4 A/B where it is the *A* side that had not changed — the machine's
  state moves that section by ±6 % between runs.
- The layout band, measured as upstream does: the same tree built with `--features null-lever`
  against itself moves `micro_bench` sections by **0.962 … 1.032** (geomean 0.998). A micro delta
  inside that band is not a finding.

## 2. Slice 4 — `foreach` over a held array (`a08311f0`)

`snapshot_entries` copied every `(key, value)` pair of the array into a `Vec` before the first
iteration — one allocation and two clones per element, and a third clone at bind time.
`IterState::ByVal` now holds one `Rc` of the array and walks it by position
(`PhpArray::positions` / `entry_at`, skipping tombstones); the key is materialised only when the
loop binds one. The semantics are PHP's own refcount semantics: the loop's reference keeps the
array unchanged, a write to the source through the body separates it, a reference element is
still read live. `yield from` over an array keeps its snapshot.

Regression test `foreach-held-array.phpt` (the session's probe: source modified / unset /
appended / replaced in the body, also through a reference; reference elements and the
lingering-reference gotcha; tombstones; destructuring; the value target aliasing the array;
property and static sources; hash escalation mid-loop) — identical to the oracle before and after,
in both lowering modes.

A/B `e48f4910` → `a08311f0`, 7 rounds — `bench/results/2026-09-30-ab-foreach.tsv`:

| benchmark | sections | geomean B/A | min | max |
|---|---:|---:|---:|---:|
| `Zend/bench.php` | 18 | 0.998 | 0.947 | 1.039 |
| `Zend/micro_bench.php` | 35 | 0.986 | 0.925 | 1.031 |
| arrays | 20 | **0.958** | **0.554** | 1.052 |
| strings | 21 | 0.997 | 0.976 | 1.050 |
| oop | 21 | 0.994 | 0.980 | 1.003 |
| autoload | 3 | 0.986 | 0.940 | 1.022 |
| symfony-boot | 3 | 0.992 | 0.953 | 1.037 |

- `packed_foreach_sum_1m` 35.3 → 30.5 ms (0.863), `packed_foreach_kv_1m` 37.2 → 32.1 ms (0.864),
  `assoc_foreach_200k_x5` 0.945, `nested_cow_copy_modify_100k` 0.949,
  `nested_pass_by_value_100k` 66.1 → 36.6 ms (**0.554**: the inner arrays were cloned into the
  snapshot).
- Above 1.05: `assoc_isset_miss_200k_x5` 1.052, spread B 21.9 %, no `foreach` in the section.
- `packed_foreach_sum_1m` is now ~30 ns per element against the oracle's ~4.5 ms per million: the
  loop body is `IterNext`, `LoadSlot`, `Binary(Add)`, `StoreSlot`, `Jump` — what is left is
  dispatch (slice 6), not the iterator.

Gates: `cargo test` 1749 / 1 / 2; LOC cap `vm/run.rs` +8 declared; phpt gate PASS (3048,
0 pass→fail), repro 12/12; DBAL unchanged.

## 2b. Slice 5 — the per-call and per-array-write allocation (`25a5e4a3`)

PROFILE.md §4 counted three allocations Zend does not make: `foreach` (gone with slice 4), the
array element write, the call to a function with a typed parameter.

- **Typed calls.** `simple_call` — the admission of the allocation-free stack→slot paths
  (`Op::Call`, `methodcall_fast`, `bind_params`) — excluded any function with a parameter hint,
  because `enter_callee` did the coercion after building the frame from an argument `Vec`. Now a
  hinted body starts with `Op::CoerceParams`, which coerces / checks the passed arguments in the
  callee frame with the caller's line and strict_types — what Zend's `RECV` opcodes do — and
  hints no longer disqualify. The op sits *before* `CheckArity`: PHP type-checks each passed
  argument before it finds the missing one (two corpus tests caught the first order:
  `function_arguments/argument_count_incorrect_userland_strict`, `pipe_operator/type_mismatch`).
  By-reference, variadic and generator functions keep `enter_callee`'s coercion.
- **Array writes.** `$a[k] = v` popped its key through `pop_keys` — `Vec::split_off` of one
  element — on every write; the single-key form pops the key directly. Same for `$a[k] op= v`
  and `$a[k]++`. The append-with-prefix form `$a[k][] = v` keeps the `Vec`: its one key is a
  prefix, and the first attempt treated it as the leaf — the Symfony section's checksum
  (`RESULT DIFFERS` in `bench/ab.sh`) caught it before anything else did.

`bench/alloc/count.sh`, fresh `mem-census` build:

| operation | before | after |
|---|---:|---:|
| array element write | 1 malloc + free | **0** |
| typed function call | 1 | **0** |
| `foreach` (slice 4) | 1 + 1 realloc, 5.25 clones/element | **0**, 2.25 clones/element |
| concat | 2 | 2 (the result string) |

Regression test `typed-call-direct-path.phpt` (`EXPECTF`). The wider probe is byte-identical
between `a08311f0` and this commit; the diffs it shows against the oracle are pre-existing and
are `KNOWN_DIVERGENCES.md` D-06 … D-12.

A/B `a08311f0` → `25a5e4a3`, 7 rounds — `bench/results/2026-09-30-ab-direct-calls.tsv`:

| benchmark | sections | geomean B/A | min | max |
|---|---:|---:|---:|---:|
| `Zend/bench.php` | 18 | 1.002 | 0.947 | 1.250 |
| `Zend/micro_bench.php` | 35 | 1.002 | 0.942 | 1.035 |
| arrays | 20 | 0.992 | 0.948 | 1.017 |
| strings | 21 | 0.994 | 0.955 | 1.017 |
| oop | 21 | 1.014 | 0.883 | 1.121 |
| autoload | 3 | 1.010 | 1.008 | 1.015 |
| symfony-boot | 3 | 0.998 | 0.995 | 1.003 |

- `function_call_1m` (a typed `add1(int $x): int`) 169.6 → 149.8 ms (**0.883**);
  `packed_index_write_1m` 0.948. Symfony `handle_requests` 0.995 — its typed methods evidently do
  not reach the inline-cache fast path often enough to show.
- `strcat(200000)` 8 → 10 ms is the 1 ms timer.
- **Three oop sections read +6–12 % in every A/B of this slice** — `prop_rmw_1m` 1.121,
  `static_method_call_1m` 1.118, `prop_write_1m` 1.062 — with spreads under 5 %. They execute no
  typed call and no array write. Bisecting the change (six builds: each edit alone, the call arm
  inline or out of line, the new op unused) reproduced the same three sections whenever the
  *executed* bytecode changed and never when only `run_loop`'s code changed; the loop itself, in
  a standalone script, runs in 127 ms on both binaries. Then the unchanged binary alone: the same
  `oop.php` under four path names gives `prop_rmw_1m` **168, 168, 188, 190 ms**
  (`bench/path-sensitivity.sh`, committed), and sweeping a `Counter` object's heap address with
  0–96 padding objects gives 116 ms everywhere but two positions at 124–136 ms — on *both*
  binaries. So: the section has a slow placement (+12–17 %) that depends on where the object,
  the frame buffers or the operand stack land, and this binary's allocation pattern (fewer
  allocations before the section) lands in it under the harness's path. Not a code regression;
  not understood either (store-to-load aliasing between the object's storage and the frame is
  the suspect — needs hardware counters, i.e. bare metal). **Open item** — and a warning for
  every future A/B on `oop.php`: a delta on these sections is a placement effect until shown
  otherwise.

Gates: `cargo test` 1749 / 1 / 2; LOC caps declared; phpt gate PASS (3048, 0 pass→fail),
repro 13/13; DBAL unchanged.

## 2c. Re-profile after slices 2–5 (`DECISION_KERNEL.md` §6 row 9)

`bench/profile.sh` with `OUT=bench/profiles/after-slice-5` (new option; the session-1 set stays
where PROFILE.md cites it), same method as session 1. Percent of samples; shares, so a bucket
grows when another shrinks (strings: the quadratic `.=` memcpy is gone).

| | `Zend/bench` | arrays | oop | symfony-steady | symfony-boot | strings | autoload |
|---|---:|---:|---:|---:|---:|---:|---:|
| VM dispatch loop, session 1 → now | 54.5 → 52.2 | 28.4 → 30.5 | 34.2 → 32.8 | 30.9 → 34.2 | 15.1 → 19.0 | 5.6 → 8.0 | 1.6 → 2.7 |
| `Rc` + `RefCell` + alloc, session 1 → now | 10.7 → 10.9 | 31.9 → 31.2 | 22.3 → 23.4 | 20.3 → 23.7 | 21.4 → 16.9 | 19.7 → 28.5 | 3.9 → 7.5 |
| parser / HIR / compile | 1.9 → 1.0 | 0.6 → 0.6 | 1.8 → 1.6 | 4.1 → 3.2 | 36.1 → 36.0 | 0.4 → 0.4 | 86.9 → 72.8 |
| name resolution (`host_builtin_canonical`, `is_name_callable`) | — | — | — | 9.6 → 0 | 3.0 → 0 | — | — |
| SipHash | — | — | — | — | — | — | 28.7 → 0 |

The session-1 figures are read from the bucket files; the session-1 table in this file has its
two Symfony columns swapped (`symfony-steady` there shows boot's numbers and vice versa).

- The PLAN §3 thresholds are unchanged by four slices: `Rc` + `RefCell` + allocation is 11–31 %,
  never near the 40 % that would argue for a new value model. What the slices removed was
  algorithmic (re-indexing, re-resolution, snapshots, argument vectors); what is left is the
  dispatch loop (row 6) and, for short runs, compilation (row 7).
- `autoload` is still 73 % lowering + compilation, now genuinely per line rather than per
  loaded class: the bytecode cache (row 7) is the next lever for boot-heavy workloads.
- `symfony-steady`: dispatch 34 %, handler bodies 17 %, `Rc` 12 %, `RefCell` 5 %, allocation
  7 % — the request path is now a plain interpreter-overhead profile.

## 3. Commits

| commit | content |
|---|---|
| `e48f4910` | the three fixes, four `.phpt`s, baseline lists advanced, `2026-09-30-ab-ns-binding.tsv` |
| `a08311f0` | slice 4, `foreach-held-array.phpt`, `2026-09-30-ab-foreach.tsv` |
| `79a1c9cd` | session-4 notes up to slice 4, `DECISION_KERNEL.md` §6 row 4 |
| `25a5e4a3` | slice 5, `typed-call-direct-path.phpt`, `2026-09-30-ab-direct-calls.tsv`, `bench/path-sensitivity.sh` |
| `a8cb8496` | §2b, `DECISION_KERNEL.md` §6 row 5 |
| (this one) | §2c, `bench/profiles/after-slice-5/`, `bench/profile.sh` `OUT=` |

Nothing was pushed.

## 4. What is next

1. The placement effect of §2b: find what aliases (object storage, frame slots, operand
   stack), on bare metal with counters if the VM cannot tell; it is worth up to 17 % on property
   access and it confounds every `oop.php` A/B.
2. Slice 6, the dispatch loop — now the whole of the `foreach` gap and most of the request path.
3. The entries of `KNOWN_DIVERGENCES.md`, each with its failing test.
4. Still the owner's: fork location, license/name, the verdict on the DRAFT decision.

---

# Session 3 — 2026-09-30 — slices 2 and 3: the per-include index rebuild, the namespaced call fallback

Same environment as sessions 1 and 2 (Docker `linux/arm64`, oracle PHP 8.5.7, rustc 1.98.1). Three
binaries were kept side by side: `/target/base` (upstream `9d4ef5ba`, unmodified), `/target/prev`
(the commit each slice started from) and `/target/release` (the working tree).

## 1. Slice 2 — every include re-indexed the whole loaded image (`26f30a38`)

PROFILE.md §5.2 blamed SipHash (28.7 % of `autoload` samples). Replacing the hasher was the small
part. Re-profiling after the swap showed what the hashing was *for*: each include unit was lowered
against a freshly built index of every class already loaded.

| step | `autoload_instantiate_cold`, single runs |
|---|---:|
| unmodified | 565–589 ms |
| FxHash in the lowering/compile symbol maps, incremental `unit_fp`, one reordered test | 405–438 ms |
| + cached seed class index, per-position registration cache | 231–240 ms |

Profile of the intermediate state (FxHash only): hashing still 26.6 % (11.1 % SipHash left in
`unit_fp`'s `DefaultHasher`, 15.5 % FxHash); the class index build ~18 % of the run, `unit_fp`,
`seed_stub_mask` and `unit_remap_elided` ~7.5 % each — all proportional to the number of classes
already loaded, per include. Parsing the included file itself: 3 %.

What changed (`vm/mod.rs`, `lower/mod.rs`):

- `Vm::unit_fp`: the registered-classes and aliases digests are maintained at the three insert
  sites (`class_index_insert`, `seed_alias_push`). The digest is an XOR of per-entry hashes, so the
  incremental value is bit-identical to the former full walk.
- `Vm::seed_class_index`: built once, handed to the lowering as an `Rc` (`ClassIndex` = read-only
  base + the unit's own layer), extended in place when the only registrations since the last
  include are appended seed classes. An alias, a conditional name or any other registration
  forces a full rebuild through the same `build_seed_class_index` function.
- `Vm::seed_registered`: per-position registration cache for the stub mask and the elided remap.
- The lowering and compile symbol maps use FxHash, like the rest of the engine.

Regression test: `baseline/repro/include-storm-seed-index.phpt` — conditional classes, a
conditional declared only on a later re-include, `class_alias`, duplicate conditional
declarations in two branches, `eval` against the image, autoloaded files in between. Identical to
the oracle with the unit cache on and off and in both `PHPR_REG_LOWER` modes.

A/B, **unmodified vs `26f30a38`** (so it includes session 2's fixes), 7 rounds, 123 sections —
`bench/results/2026-09-30-ab-seed-index.tsv`:

| benchmark | sections | geomean B/A | min | max |
|---|---:|---:|---:|---:|
| `Zend/bench.php` | 18 | 0.996 | 0.952 | 1.018 |
| `Zend/micro_bench.php` | 35 | 0.992 | 0.862 | 1.051 |
| arrays | 20 | 0.990 | 0.969 | 1.007 |
| strings | 21 | 0.769 | 0.004 | 1.048 |
| oop | 21 | 0.988 | 0.940 | 1.023 |
| autoload | 3 | **0.683** | **0.400** | 0.996 |
| symfony-boot | 3 | **0.893** | 0.809 | 0.995 |

- `autoload_instantiate_cold`: 546.7 → 218.6 ms (**0.400×**, spreads 24.0 % / 2.2 %).
- Symfony `boot_autoload_and_kernel` 10.2 → 8.2 ms (0.809), `first_request` 6.9 → 6.1 ms (0.885).
- One section above 1.05: micro_bench `$x = self::$x` 198 → 208 ms (1.051, spread A 21.2 %).

## 2. Slice 3 — the namespaced function fallback was resolved on every call (`7a5f9d49`)

`count($a)` written unqualified inside `namespace App` compiles to `Op::CallNsFallback`
(`App\count`, then global `count`), because a namespaced function declared later must be able to
shadow the builtin. That is nearly every builtin call in Symfony. Each execution ran:

1. `is_name_callable("App\count")` — three table lookups, then a **linear, case-folding scan of
   the ~400 host-builtin names** and three further host tables; always a miss;
2. `is_name_callable("count")` — the same, up to the first hit;
3. `invoke_named("count")` — the resolution again, from the top.

Share of `symfony-steady` samples (5,000 requests), inclusive:

| | unmodified | hashed lookup only | + site cache |
|---|---:|---:|---:|
| `host_builtin_canonical` | 6.14 % | 0.56 % | 0.00 % |
| `is_name_callable` | 9.60 % | 3.60 % | 0.00 % |
| resolution left at the call site (`NsIc::get` 0.16 %, `call_ns_fallback_site` self 0.08 %) | — | — | ~0.3 % |

The hashed lookup alone (`FxHashMap` over the name table, query folded through `LcKey`) moved the
samples and **not the wall time** (no measurable change on the whole run): the cost was the
repeated resolution, of which the scan was only the most visible part. Hence the cache.

- `NsIc` (`bytecode.rs`): a cell on the op, the analogue of Zend's `INIT_NS_FCALL_BY_NAME`
  runtime-cache slot, with the contract of upstream's `PropIc` (shared between op clones,
  invisible to structural equality). Key: run epoch, `Vm::fn_gen` (bumped at both
  `linked_functions` insert sites) and the identity of the running module. Targets: the namespaced
  function, the global one, or — for a registry value builtin — its entry point, called through
  `value_builtin_call` with no lookup at all. An undefined function is never cached.
- **Layout.** With the hit path written inside the `run_loop` arm, `oop.php` — which is not
  namespaced and never executes this op — drifted +1.7–4.5 % on 12 of 21 sections (geomean 1.017).
  Moved to `Vm::call_ns_fallback_site` (`calls.rs`, `#[inline(never)]`), leaving the arm the size
  it had: oop 1.005, and 1.003 in a 9-round rerun. Upstream's warning that `run_loop` is
  layout-sensitive holds for a change this small; `run.rs` grew by one line.

Semantic probe (eval / include / conditional declarations before and after a site has run,
undefined-then-defined, by-ref arguments, host builtins, closures and methods): **byte-identical
between the unmodified engine, `26f30a38` and `7a5f9d49`**, in both lowering modes. Regression
test: `baseline/repro/ns-fallback-site-cache.phpt`.

A/B, **`26f30a38` vs `7a5f9d49`**, 7 rounds, 123 sections —
`bench/results/2026-09-30-ab-ns-site-cache.tsv`:

| benchmark | sections | geomean B/A | min | max |
|---|---:|---:|---:|---:|
| `Zend/bench.php` | 18 | 1.000 | 0.982 | 1.017 |
| `Zend/micro_bench.php` | 35 | 1.001 | 0.963 | 1.028 |
| arrays | 20 | 1.001 | 0.993 | 1.015 |
| strings | 21 | 0.977 | 0.916 | 1.035 |
| oop | 21 | 1.005 | 0.980 | 1.036 |
| autoload | 3 | 0.987 | 0.976 | 1.009 |
| symfony-boot | 3 | **0.937** | **0.881** | 0.970 |

- Symfony `handle_requests` (200 requests): 49.0 → 43.2 ms (**0.881**, spreads 2.3 % / 2.8 %); a
  second run with 9 rounds gave 49.0 → 43.3 ms (0.883, spreads 1.8 % / 1.9 %).
- No section above 1.05. Worst: `prop_rmw_1m` 1.036 (spread B 28.1 %), `preg_match_small_500k`
  1.035 (spreads 18.9 % / 10.6 %), `getter_setter_fluent_1m` 1.033. In the 9-round rerun (oop and
  symfony-boot only) `prop_rmw_1m` was 1.002 and `getter_setter_fluent_1m` 0.999, and two *other*
  sections led (`arrow_fn_1m` 1.032, `closure_use_1m` 1.025). Noise.
- The three strings sections below 0.95 sit inside their own spreads (12–21 %); not claimed.
- Host load average was 7.3 when the run started (the laptop had just woken); 0.7 inside the VM.

## 3. Where the engine stands against the oracle

`bench/run.sh`, 5 runs, engines interleaved, on `7a5f9d49` — `bench/results/2026-09-30-slices-2-3.md`.
"Session 1" is `bench/results/2026-09-30.md` (unmodified engine); the oracle was re-measured in
both, and each ratio uses the oracle of its own run. `php -n` = no opcache.

| section (in-script timer) | phpr, session 1 | phpr now | `php -n` now | phpr ÷ `php -n`, session 1 | now |
|---|---:|---:|---:|---:|---:|
| `autoload_instantiate_cold` (2,000 classes) | 576.4 ms | 224.5 ms | 31.9 ms | 18.07× | **7.03×** |
| Symfony `boot_autoload_and_kernel` | 10.7 ms | 8.4 ms | 2.3 ms | 5.00× | **3.60×** |
| Symfony `first_request` | 7.4 ms | 6.5 ms | 2.6 ms | 3.06× | **2.52×** |
| Symfony `handle_requests` (200) | 51.8 ms | 46.2 ms | 5.9 ms | 8.64× | **7.88×** |

| whole process, wall | phpr, session 1 | phpr now | phpr ÷ `php -n`, session 1 | now |
|---|---:|---:|---:|---:|
| `Zend/bench.php` | 1.83 s | 1.80 s | 7.96× | 7.83× |
| `Zend/micro_bench.php` | 9.52 s | 9.32 s | 8.73× | 8.63× |
| arrays | 2.98 s | 2.90 s | 4.81× | 4.75× |
| strings | 6.47 s | 4.17 s | 3.48× | 2.28× |
| oop | 5.90 s | 6.10 s | 9.83× | 9.84× |
| autoload | 0.61 s | 0.26 s | 15.25× | 6.50× |
| symfony-boot | 0.08 s | 0.07 s | 8.00× | 7.00× |

- The strings row is session 2's `.=` fix, not this session's work.
- oop: both engines were ~3 % slower in this run than in session 1's (oracle 0.60 → 0.62 s); the
  ratio is unchanged, which agrees with the A/B (1.005, 1.003).
- `handle_requests` reads 46.2 ms here and 43.2 ms in the A/B: different runs, 5 rounds against
  7. The A/B is the instrument for deltas; this table is for the distance to the oracle.
- A steady Symfony request is still 7.9× the oracle. The re-profile of the final tree
  (`/scratch/prof-s3`, not committed): dispatch loop 32.8 %, VM handler bodies 17.6 %, `Rc` 10.6 %,
  allocation 8.0 %, `RefCell` 5.3 %. The namespace fallback is gone from it; what is left that is
  name-shaped is builtin argument plumbing (lookup by name and pre-call checks, 2.2 %) and the
  engine's symbol maps (2.8 %). The remaining distance is slices 4–6, not more lookups.
- The results file's "Problems" line about `isset(Foo::$x)` is stale: `bench/run.sh` still patches
  that statement out of `micro_bench.php`, although phpr has parsed it since session 2.

## 4. Gates (final tree, `7a5f9d49`)

| gate | result |
|---|---|
| `cargo test --release --no-fail-fast` (root) | **1749 passed / 1 failed / 2 ignored** — the failure is the root-only unwritable-path test |
| upstream's LOC-cap test (`loc_dente`) | caps raised and declared: slice 2 `vm/mod.rs`, `lower/mod.rs`, `lower/class.rs`; slice 3 `bytecode.rs` +69, `vm/mod.rs` +15, `compile/expr.rs` +2, `vm/run.rs` +1 |
| `baseline/gate.sh` | **PASS**: 3047 → 3047, pass→fail 0, pass→skip 0; repro tests 7/7 |
| repro tests on the oracle (`run-tests.php`) | both new `.phpt`s PASS |
| smoke (a) Composer `require monolog/monolog` | resolution identical, `vendor/` byte-identical (152 files) |
| smoke (b) PHPUnit on DBAL 4.5.0 | unchanged: 4146 tests, 5931 assertions, 10 errors, 1 failure, 639 skipped; 6.29 s / 6.25 s wall (6.53 / 6.38 on the unmodified engine, session 1) |

## 5. Found on the way, not fixed

1. **An already-bound call site rebinds when the namespaced function is declared later.** In
   `namespace App`, `function a() { return ucfirst("x"); }`, call `a()`, then
   `eval('namespace App; function ucfirst(…) {…}')` (or `include` a file declaring it), call `a()`
   again: PHP still calls the global `ucfirst` — its runtime-cache slot keeps the first binding —
   and phpr calls `App\ucfirst`. Same for a site inside a loop. Pre-existing (phpr re-resolved on
   every call); the site cache preserves it by invalidating on `fn_gen`. `NsIc` now makes the PHP
   behaviour a small change (do not invalidate a filled site). To check first: whether phpr's unit
   cache reuses one compiled unit across repeated `include`s of the same file, where PHP compiles
   a fresh op array — and so a fresh cache — each time.
2. **Builtin function names are case-sensitive.** `StrToUpper("q")`, `COUNT([1])` → "Call to
   undefined function", in the global namespace and in a namespace; a host builtin (`Preg_Quote`)
   and a user function called in another case resolve (checked in a namespace). The value
   registry is looked up with the name as written.
3. **`sprintf` cannot be shadowed in a namespace.** With `App\sprintf` declared before the first
   call, `sprintf(…)` inside `namespace App` still runs the builtin (PHP runs `App\sprintf`);
   `strlen` and `count` shadow correctly. `sprintf` is compiled to its own op before the fallback
   is considered.
4. The mid-work autoload profile had overwritten `bench/profiles/autoload.*` in `26f30a38`;
   `7a5f9d49` restores the session-1 files that PROFILE.md cites. `bench/profile.sh` writes into
   the repository unconditionally — profile a modified engine into `/scratch` instead.

## 6. Commits

| commit | content |
|---|---|
| `26f30a38` | slice 2, `include-storm-seed-index.phpt`, `2026-09-30-ab-seed-index.tsv` |
| `7a5f9d49` | slice 3, `ns-fallback-site-cache.phpt`, `2026-09-30-ab-ns-site-cache.tsv`, session-1 autoload profile restored |
| (this one) | this section, `DECISION_KERNEL.md` §6 rows 2–3, `bench/results/2026-09-30-slices-2-3.md` |

Nothing was pushed.

## 7. What is next

1. Slice 4 of `DECISION_KERNEL.md` §6: `foreach` by position over a held `Rc` clone instead of the
   snapshot (`packed_foreach_sum_1m`, `assoc_foreach_200k_x5` are the rows to move). Check it
   against upstream's `PERF_MAP.md` and vetoed-lever list before starting.
2. The three divergences in §5 above are small and each needs only its `.phpt`; 5.2 (case-folding
   the registry lookup) is the one real code could hit.
3. Still the owner's: where the fork lives, the license/name question, and the verdict on the
   DRAFT decision (session 1, §5).

---

# Session 2 — 2026-09-30 — three fixes: `count()` dynamic path, quiet static-property fetch, in-place `.=`

Same environment as session 1 (Docker `linux/arm64`, oracle PHP 8.5.7, rustc 1.98.1). Every
comparison below is against the engine built from `1ddea856` (upstream `9d4ef5ba`, unmodified),
built into `/target/base` from a git worktree so the two binaries could be run interleaved.

## 1. What was fixed

### 1.1 `count($countable)` through any dynamic call — one fix, wider than the bug found

The session-1 repro (`count()` unqualified inside a namespace) turned out to be one entry into a
path shared by every *dynamic* builtin call: `$f = 'count'; $f($x)`, `call_user_func('count', …)`,
`array_map('count', …)`, `count(...)`, and the run-time namespace fallback. That path
(`Vm::invoke_named`, `vm/calls.rs`) ran the builtin directly and skipped the pre-call handling the
compiled `Op::CallBuiltin` performs (`Countable` dispatch, lazy-object realisation,
`__debugInfo`, user stream wrappers). It now calls the same function (`value_builtin_call`).

- Composer 2.10.1 `require monolog/monolog` under phpr: **works end to end**. Two runs: 2.06 s and
  1.98 s wall (oracle 1.71 s / 1.40 s), user CPU 0.42 s / 0.41 s (oracle 0.43 s / 0.42 s), peak
  RSS 140 MB (oracle 49 MB). Resolution identical; `vendor/` byte-identical (152 files).
- Regression tests: `baseline/repro/count-countable-in-namespace.phpt`,
  `count-countable-dynamic-call.phpt`.

### 1.2 `isset()` / `empty()` / `??` on static properties — a quiet fetch, not just a parser fix

`isset(Foo::$a)` was rejected at *parse* time (`unsupported construct (assignment target)`), so
`Zend/micro_bench.php` never ran. Underneath: phpr had no `BP_VAR_IS` fetch for static
properties, so `Foo::$nope ?? 'd'`, `empty(Foo::$nope)`, `isset(self::$nope)`, and the private /
uninitialised-typed cases all threw where PHP silently answers unset. Upstream had restricted
the construct to `self::`/`parent::`/`static::` for exactly that reason.

Fix: a `quiet: bool` on the three static-get ops (`StaticPropGet`, `StaticPropGetDynamic`,
`StaticPropGetDynName`) backed by `Vm::ensure_static_is`, used by `isset`, `empty` and the `??`
left operand; the lowering accepts any class expression as a test place; bare `empty(C::$p)`
compiles to the quiet read + `!`. Class resolution is not quietened: `isset(NoSuch::$p)` still
throws `Class "NoSuch" not found` after an autoload attempt, as in PHP.

- 31 probe scripts (`/scratch/repro/isset`, named/dynamic class, visibility from inside and from
  a subclass, typed uninitialised, indexed and property-chained forms, `??=`, autoload, class not
  found, integer class name) all byte-identical to the oracle.
- `Zend/micro_bench.php` runs unmodified; corpus: **+6 passes** (3041 → 3047), and 4 tests moved from
  *skip* to *fail* (`closures/closure_041/043/044/046`: they used to stop at the rejected
  `isset(A::$priv)`; they now stop at a different pre-existing gap, `isset($this)` in a closure —
  "unsupported $this property write"). New corpus totals 3047 / 1625 / 1500; `Zend/tests/` alone
  2660 / 1414 / 1231. Passes: (`Zend/tests/isset/bug80030`,
  `tests/classes/static_properties_undeclared_isset`, `closures/closure_060`,
  `magic_methods/bug51822`, `class_toString_concat_with_itself`,
  `class_toString_concat_non_interned_with_itself` — the last three from 1.3).
- Regression tests: `isset-static-property.phpt`, `isset-static-property-class-not-found.phpt`.

### 1.3 `.=` in place for every left-hand form and operand kind — and `__toString()`

Measured before touching anything (`bench/concat-lhs-scaling.php`, N = 400,000 appends of an
8-byte string, unmodified engine vs oracle):

| target of `.=` | oracle | phpr before | phpr after | before ÷ after |
|---|---:|---:|---:|---:|
| local variable | 4.10 ms | 12.05 ms | 16.73 ms ¹ | — |
| by-ref parameter | 3.99 ms | **12,774 ms** | 18.09 ms | 706× |
| `global` | 4.37 ms | **12,804 ms** | 18.54 ms | 691× |
| `static` variable | 4.13 ms | **12,736 ms** | 18.34 ms | 695× |
| object property `$o->s` | 5.52 ms | **12,878 ms** | 30.33 ms | 425× |
| static property `C::$s` | 4.79 ms | **12,800 ms** | 21.13 ms | 606× |
| array element `$a['k']` | 5.43 ms | **12,854 ms** | 41.79 ms | 308× |
| `$o->parts['a']` | 6.34 ms | **12,955 ms** | 32.35 ms | 400× |
| closure `use (&$s)` | 8.70 ms | **12,796 ms** | 55.39 ms | 231× |
| local, integer operand (`.= $i`) | 9.00 ms | **8,931 ms** | 15.45 ms | 578× |
| local, float operand (`.= 1.5`) | 15.89 ms | **5,078 ms** | 101.29 ms | 50× |

¹ single runs; the local case is inside noise here and is measured properly in the A/B below.

Every bold cell was quadratic (×4 per doubling of N). The in-place fast path existed only for a
plain local slot with a string operand. Now (`vm/mod.rs::concat_in_place`): a slot held directly
or behind a reference, a static property, an array element (`AssignOpPath` leaf), a property
(`Op::PropConcatGate`, a new op in front of the unchanged read-modify-write sequence — miss falls
through), and `$o->prop[k]` (inside the existing `field_rmw_fast` perimeter). Operands: string,
int (rendered into a stack buffer), float, bool, null. Refused, so the old path runs: shared
strings (copy-on-write), arrays, objects, hooks, readonly, lazy objects, typed references.

Found and fixed alongside, because it is the same operator: **`$s .= $stringable` never called
`__toString()`** on any target form — a warning and the class name instead of the string. Plain
`.` emits a `Stringify` step per operand; the compound forms did not. Object operands of a compound
concat now go through `__toString()` (`concat_object_operands`, left operand first).

- Semantic battery `bench/concat-semantics.php` (COW, aliases, references, foreach by-ref, typed
  and readonly properties, magic, `__toString`, arrays as operand, expression values, string used
  as a hash key after in-place growth, 2,000-iteration loop with snapshots): identical to the
  oracle except two pre-existing divergences not related to `.=` (below). Also byte-identical
  between the unmodified and the patched engine on the pre-`__toString` subset, in both
  `PHPR_REG_LOWER` modes.
- Regression test: `concat-assign-in-place.phpt` (validated on the oracle with php-src's own
  `run-tests.php`, like every file in `baseline/repro/`).

## 2. Gates

| gate | result |
|---|---|
| `cargo test --release --workspace` (root) | **1749 passed / 1 failed / 2 ignored** — the failure is the root-only unwritable-path test; two new tests added (`long_digits_matches_to_zstr`, `concat_in_place_only_when_unique_and_pure`) |
| `php-runtime` lib suite as uid 65534, fresh container | 709 / 0 / 1 |
| differential | 37,835 cases, 0 mismatches (`php-types`); `php-builtins`, `php-runtime` differentials ok |
| feature matrix (`cargo check` per feature) | `op-census`, `gc-census`, `mem-census`, `census-instrumentation`, `ic-stats` ok; `zval-census` fails **identically on the unmodified tree** (references `php_types::memcensus` without its feature) |
| `baseline/gate.sh` | **PASS**: 3041 → 3047, pass→fail 0, pass→skip 0, 6 new passes; repro tests 5/5 |
| upstream's LOC-cap test (`loc_dente`) | caps raised and declared in the allowlist, upstream's convention: `vm/mod.rs` +222, `vm/run.rs` +154, `vm/arrays.rs` +33, `bytecode.rs` +22, `compile/expr.rs` +15, `lower/expr.rs` +1 |
| upstream's op-census table test | `PropConcatGate` appended (N_OPS 200 → 201); the distance-from-bottom assertions amended in block, as upstream did for `PropDimGetConst` |

### A/B, unmodified vs patched, 7 rounds interleaved (ABBA), 123 sections — `bench/results/2026-09-30-ab-fixes.tsv`

| benchmark | sections | geomean B/A | min | max |
|---|---:|---:|---:|---:|
| `Zend/bench.php` | 18 | 0.978 | 0.889 | 1.018 |
| `Zend/micro_bench.php` | 35 | 0.979 | 0.842 | 1.045 |
| arrays | 20 | 0.991 | 0.954 | 1.017 |
| strings | 21 | **0.769** | **0.004** | 1.051 |
| oop | 21 | 0.992 | 0.976 | 1.010 |
| autoload | 3 | 0.959 | 0.927 | 1.002 |
| symfony-boot | 3 | 1.002 | 0.993 | 1.015 |

- `concat_append_int_200k`: 2,089.6 ms → 8.1 ms (**0.004×**).
- `empty(self::$x)`: 273 → 230 ms (0.84); `self::$x = 0` / `Foo::$x = 0`: 0.90 (noise band ~15 %).
- The one section above 1.05: `concat_append_1m` (local `.= 'x'`) 34.9 → 36.7 ms, **1.051**, spreads
  3.6 % / 7.7 % — ≈1.8 ns per append, seen in five A/B runs (1.051, 1.053, 1.049, 1.039, 1.051). Declared.
  Every other section is within its round-to-round spread. No RESULT checksum differs.
- loadavg was 3.1 at the start of the final A/B (other host processes); spreads are wider than in
  session 1.

## 3. Found on the way, not fixed

Moved to [`KNOWN_DIVERGENCES.md`](KNOWN_DIVERGENCES.md) (session 5), one failing `.phpt` each:
D-01 (uninitialised typed static read), D-02 (`NAN` to string), D-03 (redeclaring a builtin).
Two flags of this session did not reproduce and are closed there (the diagnostic line inside call
arguments; `Op::StaticPropRef` relocation — now guarded by
`baseline/repro/static-prop-ref-across-include.phpt`). Two were performance, not behaviour, and
stay here:

1. **`.=` on a float operand is 6.6× the oracle** (linear now, 101 ms vs 15 ms at 400k): the float
   formatting (`double_to_precision`) allocates and is slow; `bench/concat-scaling.php` row
   `append_float_ms`.
2. **Indexed static properties are read-modify-written through a temp copy**
   (`static_prop_rmw`): `C::$arr['k'] .= x` separates the whole array each time. Not measured.

## 4. Commits

| commit | content |
|---|---|
| (this one) | the three fixes, their `.phpt`s and unit tests, `bench/ab.sh` + `bench/lib/ab-summary.py`, `bench/concat-lhs-scaling.php`, `bench/concat-semantics.php`, gate runs `baseline/repro/*.phpt`, baseline lists advanced (3047), results `bench/results/2026-09-30-fixes.md`, `2026-09-30-ab-fixes.tsv` |

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

## 2. Phase 1 — status against PLAN.md §2

| PLAN item | status |
|---|---|
| 2.1 `bench/run.sh`, 3 engine configurations, 5 runs, median wall / user / RSS | Done, plus a 4th configuration (opcache with a warm file cache). |
| 2.1 `Zend/bench.php` | Done. |
| 2.1 `Zend/micro_bench.php` | Done **from a patched copy**: phpr cannot parse the original. 36 of 37 rows measured. |
| 2.1 `arrays.php`, `strings.php`, `oop.php`, `autoload.php`, `symfony-boot.php` | Done. All phpr checksums match the oracle. |
| 2.1 Results committed with SHA and oracle version | Done: `bench/results/2026-09-30.md`. |
| 2.2 Build with debug info, profile 4 benchmarks, save flamegraphs | Done for 7 profiles (the 4 asked + `symfony-steady`, `strings`, `autoload`). `perf`, not `samply`. |
| 2.2 Bucket table in `PROFILE.md` | Done. |
| 2.2 Allocations per iteration | Done: exact counts for 15 loop bodies. |
| 2.2 Compile cost on a 5,000-line file | Done, plus a 20,000-line file and the empty-script floor. |
| 3 `DECISION_KERNEL.md` | **Drafted, marked DRAFT.** Recommends outcome A. |
| `php-types` and hot paths | **Not modified.** `git diff 9d4ef5ba -- php-rust/crates` is empty. |

### Builds made for Phase 1

| binary | how | build time | size |
|---|---|---:|---:|
| measured `phpr` | `cargo build --release --locked --workspace` | 58.6 s (after the 124.2 s `-p php-cli` build) | 17,002,400 B, sha256 `f690bd4bb4d0346c` |
| profiled `phpr` | same + `CARGO_PROFILE_RELEASE_DEBUG=1`, `/target/prof` | 2 m 18 s | 90,793,752 B |
| census `phpr` | `--features mem-census`, `/target/census` | 2 m 08 s | 20,147,936 B |

The `-p php-cli` build of Phase 0 produced a 17,002,224-byte binary; the workspace build relinked
it to 17,002,400 bytes. **All benchmarks used the 17,002,400-byte binary.**

### Measurement conditions

5 runs per (benchmark, engine), engines interleaved within each round. loadavg 0.45 at start,
1.27 at end. `memory_limit=-1` on every engine. Inputs staged on a container-local volume.
`/usr/bin/time -v` for whole-process wall/user/RSS (10 ms resolution); `hrtime()` for sections.

Oracle configurations: `php -n -d opcache.enable_cli=0` · `php -d opcache.enable_cli=1`
(one-shot; optimizer on, nothing cached between processes) · the same with a primed
`opcache.file_cache`. JIT off throughout. Symfony components installed: http-kernel 7.4.20,
http-foundation 7.4.20, routing 7.4.20, event-dispatcher 7.4.17 (13 packages, 517 PHP files).
Autoload fixture: 2,002 files, 106,018 lines.

### Benchmark numbers — all of them

#### Whole-process summary

wall / user in seconds, RSS = peak resident set in MiB (`/usr/bin/time -v`). Whole-process figures include each engine's startup and, for the harness benchmarks, fixture construction outside the timed sections.

| benchmark | phpr wall | phpr user | phpr RSS | php-noopc wall | php-noopc user | php-noopc RSS | php-opc wall | php-opc user | php-opc RSS | php-opc-warm wall | php-opc-warm user | php-opc-warm RSS | phpr/noopc wall | phpr/opc wall | phpr/opc-warm wall | phpr/noopc user | phpr/noopc RSS |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| zend_bench | 1.830 | 1.820 | 77.6 | 0.230 | 0.230 | 28.4 | 0.160 | 0.160 | 30.8 | 0.160 | 0.160 | 30.5 | 7.96× | 11.44× | 11.44× | 7.91× | 2.73× |
| zend_micro_bench | 9.520 | 9.500 | 75.6 | 1.090 | 1.090 | 23.9 | 0.810 | 0.810 | 26.3 | 0.810 | 0.800 | 25.7 | 8.73× | 11.75× | 11.75× | 8.72× | 3.17× |
| arrays | 2.980 | 2.970 | 263.6 | 0.620 | 0.570 | 202.2 | 0.610 | 0.560 | 205.2 | 0.610 | 0.560 | 204.8 | 4.81× | 4.89× | 4.89× | 5.21× | 1.30× |
| strings | 6.470 | 6.400 | 350.9 | 1.860 | 1.740 | 315.4 | 1.840 | 1.760 | 318.4 | 1.830 | 1.730 | 318.2 | 3.48× | 3.52× | 3.54× | 3.68× | 1.11× |
| oop | 5.900 | 5.890 | 75.6 | 0.600 | 0.600 | 23.9 | 0.590 | 0.590 | 26.2 | 0.600 | 0.590 | 25.8 | 9.83× | 10.00× | 9.83× | 9.82× | 3.16× |
| autoload | 0.610 | 0.590 | 166.1 | 0.040 | 0.030 | 40.1 | 0.070 | 0.050 | 56.8 | 0.030 | 0.020 | 51.8 | 15.25× | 8.71× | 20.33× | 19.67× | 4.14× |
| symfony-boot | 0.080 | 0.080 | 91.8 | 0.010 | 0.010 | 25.0 | 0.020 | 0.020 | 29.0 | 0.010 | 0.000 | 27.7 | 8.00× | 4.00× | 8.00× | 8.00× | 3.68× |

#### Problems

- `Zend/micro_bench.php` **unmodified** does not run under phpr: `PHP Parse error: unsupported construct (assignment target) on line 147`. The table below uses a copy with that one statement neutralised.
- `zend_micro_bench` / `isset(Foo::$x)`: statement patched out on both engines: phpr rejects `isset(Class::$static)` at parse time.

#### Per-section detail

##### `zend_bench` — per-section times (seconds, median of 5)

| section | phpr | php-noopc | php-opc | php-opc-warm | phpr / noopc | phpr / opc |
|---|---:|---:|---:|---:|---:|---:|
| simple | 0.0190 | 0.0050 | 0.0040 | 0.0040 | 3.80× | 4.75× |
| simplecall | 0.0600 | 0.0020 | 0.0020 | 0.0020 | 30.00× | 30.00× |
| simpleucall | 0.0760 | 0.0070 | 0.0020 | 0.0020 | 10.86× | 38.00× |
| simpleudcall | 0.0750 | 0.0090 | 0.0020 | 0.0020 | 8.33× | 37.50× |
| mandel | 0.3160 | 0.0480 | 0.0210 | 0.0210 | 6.58× | 15.05× |
| mandel2 | 0.4110 | 0.0470 | 0.0280 | 0.0280 | 8.74× | 14.68× |
| ackermann(7) | 0.0520 | 0.0100 | 0.0080 | 0.0080 | 5.20× | 6.50× |
| ary(50000) | 0.0080 | 0.0020 | 0.0010 | 0.0010 | 4.00× | 8.00× |
| ary2(50000) | 0.0080 | 0.0010 | 0.0010 | 0.0010 | 8.00× | 8.00× |
| ary3(2000) | 0.2620 | 0.0170 | 0.0180 | 0.0180 | 15.41× | 14.56× |
| fibo(30) | 0.1900 | 0.0320 | 0.0260 | 0.0260 | 5.94× | 7.31× |
| hash1(50000) | 0.0160 | 0.0030 | 0.0030 | 0.0030 | 5.33× | 5.33× |
| hash2(500) | 0.0370 | 0.0040 | 0.0040 | 0.0040 | 9.25× | 9.25× |
| heapsort(20000) | 0.0990 | 0.0110 | 0.0100 | 0.0100 | 9.00× | 9.90× |
| matrix(20) | 0.0700 | 0.0100 | 0.0090 | 0.0090 | 7.00× | 7.78× |
| nestedloop(12) | 0.0530 | 0.0100 | 0.0100 | 0.0100 | 5.30× | 5.30× |
| sieve(30) | 0.0470 | 0.0060 | 0.0050 | 0.0050 | 7.83× | 9.40× |
| strcat(200000) | 0.0080 | 0.0020 | 0.0020 | 0.0020 | 4.00× | 4.00× |
| Total | 1.8110 | 0.2280 | 0.1560 | 0.1560 | 7.94× | 11.61× |

##### `zend_micro_bench` — per-section times (seconds, median of 5)

| section | phpr | php-noopc | php-opc | php-opc-warm | phpr / noopc | phpr / opc |
|---|---:|---:|---:|---:|---:|---:|
| empty_loop | 0.0480 | 0.0110 | 0.0110 | 0.0110 | 4.36× | 4.36× |
| func() | 0.3190 | 0.0360 | 0.0110 | 0.0110 | 8.86× | 29.00× |
| undef_func() | 0.3170 | 0.0400 | 0.0110 | 0.0110 | 7.92× | 28.82× |
| int_func() | 0.1770 | 0.0190 | 0.0110 | 0.0110 | 9.32× | 16.09× |
| $x = self::$x | 0.2060 | 0.0250 | 0.0250 | 0.0250 | 8.24× | 8.24× |
| self::$x = 0 | 0.2270 | 0.0230 | 0.0220 | 0.0220 | 9.87× | 10.32× |
| isset(self::$x) | 0.2780 | 0.0260 | 0.0220 | 0.0210 | 10.69× | 12.64× |
| empty(self::$x) | 0.2810 | 0.0300 | 0.0250 | 0.0250 | 9.37× | 11.24× |
| $x = Foo::$x | 0.2000 | 0.0260 | 0.0260 | 0.0260 | 7.69× | 7.69× |
| Foo::$x = 0 | 0.2250 | 0.0230 | 0.0230 | 0.0230 | 9.78× | 9.78× |
| isset(Foo::$x) | not measured | — | — | — | — | — |
| empty(Foo::$x) | 0.2420 | 0.0280 | 0.0240 | 0.0230 | 8.64× | 10.08× |
| self::f() | 0.3460 | 0.0480 | 0.0110 | 0.0120 | 7.21× | 31.45× |
| Foo::f() | 0.3460 | 0.0430 | 0.0120 | 0.0110 | 8.05× | 28.83× |
| $x = $this->x | 0.1170 | 0.0220 | 0.0230 | 0.0230 | 5.32× | 5.09× |
| $this->x = 0 | 0.2790 | 0.0240 | 0.0230 | 0.0230 | 11.62× | 12.13× |
| $this->x += 2 | 0.4580 | 0.0350 | 0.0350 | 0.0340 | 13.09× | 13.09× |
| ++$this->x | 0.2450 | 0.0310 | 0.0310 | 0.0310 | 7.90× | 7.90× |
| --$this->x | 0.2420 | 0.0320 | 0.0310 | 0.0310 | 7.56× | 7.81× |
| $this->x++ | 0.2420 | 0.0310 | 0.0310 | 0.0310 | 7.81× | 7.81× |
| $this->x-- | 0.2410 | 0.0320 | 0.0310 | 0.0310 | 7.53× | 7.77× |
| isset($this->x) | 0.1620 | 0.0320 | 0.0280 | 0.0280 | 5.06× | 5.79× |
| empty($this->x) | 0.5840 | 0.0360 | 0.0310 | 0.0310 | 16.22× | 18.84× |
| $this->f() | 0.4170 | 0.0440 | 0.0440 | 0.0430 | 9.48× | 9.48× |
| $x = Foo::TEST | 0.3930 | 0.0310 | 0.0310 | 0.0300 | 12.68× | 12.68× |
| new Foo() | 1.1990 | 0.0880 | 0.0890 | 0.0890 | 13.63× | 13.47× |
| $x = TEST | 0.1690 | 0.0180 | 0.0180 | 0.0180 | 9.39× | 9.39× |
| $x = $_GET | 0.1210 | 0.0320 | 0.0310 | 0.0320 | 3.78× | 3.90× |
| $x = $GLOBALS['v'] | 0.1100 | 0.0270 | 0.0270 | 0.0270 | 4.07× | 4.07× |
| $x = $hash['v'] | 0.2010 | 0.0410 | 0.0110 | 0.0110 | 4.90× | 18.27× |
| $x = $str[0] | 0.2430 | 0.0310 | 0.0110 | 0.0110 | 7.84× | 22.09× |
| $x = $a ?: null | 0.1860 | 0.0270 | 0.0110 | 0.0110 | 6.89× | 16.91× |
| $x = $f ?: tmp | 0.2060 | 0.0280 | 0.0110 | 0.0110 | 7.36× | 18.73× |
| $x = $f ? $f : $a | 0.1800 | 0.0280 | 0.0110 | 0.0110 | 6.43× | 16.36× |
| $x = $f ? $f : tmp | 0.1650 | 0.0300 | 0.0110 | 0.0110 | 5.50× | 15.00× |
| Total | 9.4920 | 1.0880 | 0.8100 | 0.8090 | 8.72× | 11.72× |

##### `arrays` — per-section times (seconds, median of 5)

| section | phpr | php-noopc | php-opc | php-opc-warm | phpr / noopc | phpr / opc |
|---|---:|---:|---:|---:|---:|---:|
| packed_build_1m | 0.1200 | 0.0087 | 0.0090 | 0.0088 | 13.85× | 13.27× |
| packed_foreach_sum_1m | 0.0383 | 0.0048 | 0.0045 | 0.0049 | 7.97× | 8.41× |
| packed_foreach_kv_1m | 0.0380 | 0.0070 | 0.0072 | 0.0070 | 5.45× | 5.26× |
| packed_index_read_1m | 0.0609 | 0.0071 | 0.0063 | 0.0064 | 8.52× | 9.62× |
| packed_index_write_1m | 0.1248 | 0.0137 | 0.0130 | 0.0129 | 9.13× | 9.57× |
| packed_foreach_byref_1m | 0.0666 | 0.0175 | 0.0175 | 0.0174 | 3.81× | 3.81× |
| assoc_build_200k | 0.0399 | 0.0090 | 0.0084 | 0.0085 | 4.45× | 4.75× |
| assoc_lookup_200k_x5 | 0.1408 | 0.0365 | 0.0340 | 0.0341 | 3.86× | 4.15× |
| assoc_isset_miss_200k_x5 | 0.1634 | 0.0338 | 0.0325 | 0.0322 | 4.84× | 5.02× |
| assoc_foreach_200k_x5 | 0.1028 | 0.0101 | 0.0097 | 0.0098 | 10.22× | 10.60× |
| assoc_unset_reinsert_200k | 0.0478 | 0.0105 | 0.0101 | 0.0099 | 4.55× | 4.75× |
| nested_build_rows_100k | 0.0818 | 0.0149 | 0.0147 | 0.0144 | 5.50× | 5.58× |
| nested_cow_copy_modify_100k | 0.0824 | 0.0284 | 0.0282 | 0.0280 | 2.90× | 2.92× |
| nested_pass_by_value_100k | 0.0684 | 0.0182 | 0.0177 | 0.0174 | 3.76× | 3.86× |
| array_map_closure_1m | 0.1023 | 0.0225 | 0.0232 | 0.0239 | 4.54× | 4.41× |
| array_filter_closure_1m | 0.1412 | 0.0243 | 0.0248 | 0.0240 | 5.82× | 5.70× |
| usort_closure_200k | 0.4316 | 0.0990 | 0.0979 | 0.0984 | 4.36× | 4.41× |
| sort_builtin_1m | 0.3931 | 0.1533 | 0.1549 | 0.1528 | 2.56× | 2.54× |
| array_merge_slice_keys_x50 | 0.5361 | 0.0702 | 0.0712 | 0.0712 | 7.64× | 7.53× |
| in_array_array_search_2k_x2k | 0.0103 | 0.0037 | 0.0039 | 0.0037 | 2.74× | 2.64× |

##### `strings` — per-section times (seconds, median of 5)

Info: json_bytes=8691213 · serialize_bytes=12650102

| section | phpr | php-noopc | php-opc | php-opc-warm | phpr / noopc | phpr / opc |
|---|---:|---:|---:|---:|---:|---:|
| concat_append_1m | 0.0368 | 0.0100 | 0.0089 | 0.0090 | 3.67× | 4.12× |
| concat_append_int_200k | 2.2188 | 0.0059 | 0.0058 | 0.0061 | 374.74× | 380.52× |
| concat_binary_temp_1m | 0.1357 | 0.0375 | 0.0331 | 0.0327 | 3.62× | 4.10× |
| interpolation_1m | 0.1639 | 0.0439 | 0.0404 | 0.0402 | 3.73× | 4.05× |
| sprintf_500k | 0.2543 | 0.1088 | 0.1077 | 0.1074 | 2.34× | 2.36× |
| strlen_substr_strpos_1m | 0.3571 | 0.0419 | 0.0376 | 0.0380 | 8.53× | 9.49× |
| str_replace_1_3mb_x20 | 0.0190 | 0.0377 | 0.0377 | 0.0376 | 0.50× | 0.51× |
| strtr_array_1_3mb_x10 | 0.1098 | 0.0209 | 0.0208 | 0.0211 | 5.24× | 5.28× |
| preg_replace_1_3mb_x10 | 0.0633 | 0.0267 | 0.0263 | 0.0264 | 2.37× | 2.41× |
| preg_match_small_500k | 0.3115 | 0.0635 | 0.0573 | 0.0573 | 4.91× | 5.44× |
| preg_replace_callback_1_3mb_x3 | 0.0232 | 0.0093 | 0.0086 | 0.0085 | 2.50× | 2.69× |
| preg_split_1_3mb_x5 | 0.1178 | 0.0258 | 0.0257 | 0.0254 | 4.57× | 4.59× |
| explode_implode_1_3mb_x20 | 0.2199 | 0.0743 | 0.0744 | 0.0738 | 2.96× | 2.96× |
| case_trim_ucwords_500k | 0.2453 | 0.0571 | 0.0552 | 0.0546 | 4.30× | 4.44× |
| htmlspecialchars_md5_200k | 0.1449 | 0.0984 | 0.0992 | 0.0976 | 1.47× | 1.46× |
| json_encode_5mb_x5 | 0.4015 | 0.1638 | 0.1631 | 0.1630 | 2.45× | 2.46× |
| json_decode_assoc_5mb_x5 | 0.3602 | 0.2435 | 0.2316 | 0.2330 | 1.48× | 1.56× |
| json_decode_object_5mb_x5 | 0.4465 | 0.2478 | 0.2500 | 0.2502 | 1.80× | 1.79× |
| serialize_x5 | 0.2022 | 0.1701 | 0.1648 | 0.1663 | 1.19× | 1.23× |
| unserialize_x5 | 0.2904 | 0.1207 | 0.1213 | 0.1215 | 2.40× | 2.39× |
| var_export_x2 | 0.1015 | 0.1695 | 0.1699 | 0.1679 | 0.60× | 0.60× |

##### `oop` — per-section times (seconds, median of 5)

| section | phpr | php-noopc | php-opc | php-opc-warm | phpr / noopc | phpr / opc |
|---|---:|---:|---:|---:|---:|---:|
| function_call_1m | 0.1774 | 0.0157 | 0.0148 | 0.0149 | 11.29× | 11.99× |
| method_call_1m | 0.2528 | 0.0155 | 0.0155 | 0.0158 | 16.30× | 16.34× |
| method_call_args_ret_1m | 0.2527 | 0.0211 | 0.0208 | 0.0207 | 12.00× | 12.12× |
| parent_call_1m | 0.4030 | 0.0375 | 0.0366 | 0.0366 | 10.75× | 11.01× |
| static_method_call_1m | 0.1462 | 0.0167 | 0.0149 | 0.0150 | 8.77× | 9.82× |
| prop_read_1m | 0.0452 | 0.0061 | 0.0050 | 0.0051 | 7.37× | 8.98× |
| prop_write_1m | 0.1466 | 0.0076 | 0.0078 | 0.0079 | 19.28× | 18.88× |
| prop_rmw_1m | 0.1757 | 0.0103 | 0.0103 | 0.0104 | 17.07× | 17.04× |
| getter_setter_fluent_1m | 0.3895 | 0.0346 | 0.0338 | 0.0342 | 11.26× | 11.54× |
| magic_get_1m | 0.7635 | 0.0797 | 0.0796 | 0.0808 | 9.58× | 9.59× |
| magic_set_500k | 0.1895 | 0.0178 | 0.0177 | 0.0179 | 10.64× | 10.68× |
| magic_call_500k | 0.1122 | 0.0188 | 0.0190 | 0.0189 | 5.96× | 5.92× |
| interface_dispatch_3way_1m | 0.2438 | 0.0384 | 0.0367 | 0.0361 | 6.35× | 6.65× |
| instanceof_1m | 0.1815 | 0.0150 | 0.0134 | 0.0135 | 12.11× | 13.54× |
| new_object_1m | 0.7744 | 0.0797 | 0.0782 | 0.0782 | 9.71× | 9.90× |
| closure_bound_this_1m | 0.2252 | 0.0195 | 0.0194 | 0.0192 | 11.57× | 11.60× |
| closure_use_1m | 0.1820 | 0.0198 | 0.0197 | 0.0199 | 9.17× | 9.25× |
| arrow_fn_1m | 0.1798 | 0.0200 | 0.0198 | 0.0198 | 9.00× | 9.09× |
| first_class_callable_1m | 0.2531 | 0.0233 | 0.0227 | 0.0227 | 10.88× | 11.15× |
| exception_throw_catch_100k | 0.1733 | 0.0212 | 0.0212 | 0.0212 | 8.20× | 8.17× |
| exception_unwind_depth8_finally_100k | 0.6279 | 0.0827 | 0.0795 | 0.0789 | 7.59× | 7.90× |

##### `autoload` — per-section times (seconds, median of 5)

Info: declared_classes=2000

| section | phpr | php-noopc | php-opc | php-opc-warm | phpr / noopc | phpr / opc |
|---|---:|---:|---:|---:|---:|---:|
| require_composer_autoloader | 0.0007 | 0.0002 | 0.0005 | 0.0001 | 2.85× | 1.29× |
| autoload_instantiate_cold | 0.5764 | 0.0319 | 0.0623 | 0.0252 | 18.07× | 9.25× |
| instantiate_warm | 0.0074 | 0.0010 | 0.0021 | 0.0018 | 7.70× | 3.49× |

##### `symfony-boot` — per-section times (seconds, median of 5)

Info: requests=200 · declared_classes=274

| section | phpr | php-noopc | php-opc | php-opc-warm | phpr / noopc | phpr / opc |
|---|---:|---:|---:|---:|---:|---:|
| boot_autoload_and_kernel | 0.0107 | 0.0021 | 0.0050 | 0.0009 | 5.00× | 2.13× |
| first_request | 0.0074 | 0.0024 | 0.0064 | 0.0009 | 3.06× | 1.16× |
| handle_requests | 0.0518 | 0.0060 | 0.0056 | 0.0055 | 8.64× | 9.18× |


### Compile cost, allocations, concat scaling — all of them

#### 1. Compile cost — `bench/compile-cost.sh`

Median of **30** runs per cell, interleaved, one warm-up round discarded. Wall time measured
around `fork`+`wait4`, CPU and peak RSS from the child's `rusage`. `empty.php` is `<?php` alone.
`big.php` / `big20k.php` are generated declarations (functions and classes with real bodies) that
are **never called**, so everything above the empty-file floor is lex + parse + compile.

| script | engine | wall ms | user ms | sys ms | peak RSS kB | wall min–max ms |
|---|---|---:|---:|---:|---:|---:|
| `empty.php` (1 line) | phpr | 15.82 | 12.90 | 2.99 | 70,386 | 14.77–17.57 |
| | phpr, `PHPR_UNIT_CACHE=0` | 15.43 | 12.90 | 2.99 | 70,378 | 14.61–17.33 |
| | php, no opcache | 6.29 | 4.17 | 2.00 | 24,072 | 5.96–7.75 |
| | php, opcache | 6.47 | 3.87 | 2.74 | 26,056 | 5.99–7.72 |
| | php, opcache + warm file cache | 6.23 | 4.01 | 2.02 | 26,032 | 5.79–7.96 |
| `big.php` (5,003 lines, 141,468 B) | phpr | 22.59 | 19.37 | 3.00 | 90,864 | 21.37–24.07 |
| | phpr, `PHPR_UNIT_CACHE=0` | 22.26 | 19.18 | 3.01 | 90,866 | 21.25–23.76 |
| | php, no opcache | 8.50 | 6.11 | 2.46 | 25,548 | 8.20–9.35 |
| | php, opcache | 10.49 | 7.91 | 2.92 | 29,998 | 10.03–12.21 |
| | php, opcache + warm file cache | 6.78 | 4.58 | 2.50 | 27,688 | 6.54–8.37 |
| `big20k.php` (20,020 lines, 567,986 B) | phpr | 45.54 | 41.28 | 4.03 | 158,464 | 44.66–47.26 |
| | phpr, `PHPR_UNIT_CACHE=0` | 45.58 | 41.48 | 4.03 | 158,468 | 44.67–65.69 |
| | php, no opcache | 14.83 | 11.80 | 2.98 | 32,976 | 14.28–16.31 |
| | php, opcache | 22.56 | 18.43 | 4.00 | 43,052 | 21.71–23.97 |
| | php, opcache + warm file cache | 8.91 | 5.19 | 3.84 | 32,564 | 8.49–10.70 |

Derived (script − `empty.php`, same engine):

| | phpr | php no opcache | php opcache (one-shot) | php opcache, warm |
|---|---:|---:|---:|---:|
| **startup floor** | **15.82 ms** | 6.29 ms | 6.47 ms | 6.23 ms |
| compile 5,003 lines | **6.77 ms** | 2.21 ms | 4.02 ms | 0.55 ms |
| compile 20,020 lines | **29.72 ms** | 8.54 ms | 16.09 ms | 2.68 ms |
| per 1,000 lines (5k / 20k file) | 1.35 / 1.48 ms | 0.44 / 0.43 ms | 0.80 / 0.80 ms | 0.11 / 0.13 ms |
| memory per 1,000 lines (5k / 20k file) | 4,093 / 4,400 kB | 295 / 445 kB | — | — |

- phpr's startup floor is 9.5 ms above the oracle's and 46 MB heavier. phpr compiles its embedded
  PHP prelude (~9,300 lines) in every process; the oracle's internal classes are C.
- phpr compiles at about **one third of the oracle's speed** (3.1–3.5× the time per line) and
  retains **10–14× the memory per line**.
- One-shot CLI opcache is *slower* than no opcache (the optimizer runs, nothing is reused). Only
  the warm file cache shows what a persistent cache gives: 0.11–0.13 ms per 1,000 lines.
- `PHPR_UNIT_CACHE=0` changes nothing here: the unit cache is in-process and a CLI run has one
  process.

#### 2. Allocations per loop iteration — `bench/alloc/count.sh`

phpr built with upstream's own `mem-census` feature (separate target dir; its global-allocator
wrapper counts every call). Each variant run at N = 200,000 and N = 400,000; the figure is
Δcount ÷ ΔN, so startup cancels. All results are exact integers. Output of every variant matches
the oracle. The census binary is used for counting only, never for timing.

| loop body | allocs / iter | frees / iter | reallocs / iter | `Zval` clones / iter |
|---|---:|---:|---:|---:|
| empty `for` (global scope) | 0 | 0 | 0 | 0 |
| integer arithmetic (global scope) | 0 | 0 | 0 | 1 |
| integer arithmetic (in a function) | 0 | 0 | 0 | 1 |
| float arithmetic | 0 | 0 | 0 | 0 |
| array element read `$s += $a[$i & 7]` | 0 | 0 | 0 | 2 |
| **array element write `$a[$i & 7] = $s`** (global scope) | **1** | 1 | 0 | 4 |
| **array element write** (in a function) | **1** | 1 | 0 | 4 |
| **string-key write `$a['k1'] = $i`** | **1** | 1 | 0 | 3 |
| array append `$a[] = $i` | 0 | 0 | 0 | 3 |
| **`foreach` over an 8-element array** (per loop entry) | **1** | 1 | 1 | **42** |
| **user function call `$s += add1($i)`** | **1** | 1 | 0 | 2 |
| builtin call `$s += abs($i)` | 0 | 0 | 0 | 1 |
| property write `$o->v = $i` | 0 | 0 | 0 | 4 |
| method call `$o->inc()` | 0 | 0 | 0 | 9 |
| concat + strlen `$x = 'id-' . $i; strlen($x)` | 2 | 2 | 0 | 3 |

The PLAN's reference loop (integer arithmetic + one array write, `bench/alloc/alloc-loop.php`):
1,183,457 allocator calls at N = 1,000,000 and 2,183,457 at N = 2,000,000 →
**exactly 1.000 allocation and 1.000 free per iteration.**

#### 3. `.=` with a non-string operand is quadratic — `bench/concat-scaling.php`

One run per engine, times in ms.

| N | result bytes | php `.= $i` | phpr `.= $i` | php `.= (string)$i` | phpr `.= (string)$i` | php `.= 1.5` | phpr `.= 1.5` |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 50,000 | 238,890 | 1.04 | 127.35 | 1.01 | 2.56 | 1.97 | 85.13 |
| 100,000 | 488,890 | 1.92 | 486.35 | 1.90 | 4.75 | 3.76 | 325.23 |
| 200,000 | 1,088,890 | 4.06 | 2,091.17 | 4.20 | 9.65 | 7.93 | 1,273.92 |
| 400,000 | 2,288,890 | 9.00 | 8,930.59 | 8.70 | 19.04 | 15.89 | 5,078.40 |

| growth per doubling of N | php `.= $i` | phpr `.= $i` | phpr `.= (string)$i` |
|---|---:|---:|---:|
| 50k → 100k | 1.85× | 3.82× | 1.86× |
| 100k → 200k | 2.11× | 4.30× | 2.03× |
| 200k → 400k | 2.22× | 4.27× | 1.97× |

At N = 400,000 phpr is **992× slower** than the oracle for `$s .= $i` and 2.2× slower for
`$s .= (string) $i`. The in-place append fast path applies only when the right-hand side is
already a string; an integer or float operand falls back to allocating a new string and copying
the whole accumulated buffer every iteration. In the first dry run of `bench/strings.php`, the
1,000,000-iteration version of this loop took 61.8 s under phpr against 34 ms on the oracle,
which is why the committed benchmark uses 200,000.

### Profile — bucket table (percent of samples)

`perf record -e cpu-clock --call-graph dwarf`, 2,999 Hz (`autoload` 9,999 Hz, `symfony-boot`
19,999 Hz). Attribution by DWARF inline chain, ±2 points. Rules in `bench/lib/buckets.py`.

| bucket | `Zend/bench` | `arrays` | `oop` | `symfony-boot` | `symfony-steady` | `strings` | `autoload` |
|---|---:|---:|---:|---:|---:|---:|---:|
| `Rc` inc/dec + drop glue | 8.55 | 19.56 | 9.41 | 4.59 | 9.42 | 3.86 | 1.30 |
| `RefCell` borrow checks | 0.26 | 0.27 | 4.69 | 2.22 | 4.40 | 1.61 | 0.18 |
| allocation / deallocation | 1.86 | 12.04 | 8.15 | 14.56 | 6.47 | 14.25 | 2.40 |
| hash table ops in `PhpArray` | 2.08 | 14.22 | 2.45 | 1.57 | 2.30 | 3.40 | 0.10 |
| string hash / compare / copy | 0.31 | 2.68 | 4.98 | 2.66 | 4.75 | 44.31 | 0.37 |
| VM dispatch loop itself | 54.45 | 28.36 | 34.19 | 15.14 | 30.89 | 5.61 | 1.59 |
| argument passing into builtins | 0.65 | 1.03 | 0.01 | 0.84 | 2.22 | 2.61 | 0.06 |
| builtin bodies | 0.14 | 1.08 | 0.02 | 2.04 | 3.11 | 15.36 | 0.10 |
| parser / HIR / compile (inclusive) | 1.93 | 0.63 | 1.82 | 36.08 | 4.14 | 0.37 | 86.88 |
| other: operator / type-juggling bodies | 17.08 | 5.31 | 3.53 | 1.09 | 1.72 | 2.34 | 0.06 |
| other: VM handler bodies | 10.65 | 11.78 | 21.41 | 10.30 | 22.61 | 4.32 | 2.12 |
| other: GC bookkeeping + sweep | 1.34 | 1.46 | 3.59 | 1.20 | 2.27 | 0.53 | 0.16 |
| other: engine-internal hash maps | 0.38 | 1.40 | 5.67 | 2.73 | 5.33 | 1.11 | 0.56 |
| other: kernel (non-page-fault) | 0.03 | 0.07 | 0.02 | 2.62 | 0.13 | 0.13 | 3.39 |
| other: startup, libc, std I/O | 0.29 | 0.11 | 0.07 | 2.37 | 0.25 | 0.18 | 0.72 |
| other: unclassified | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |
| **`Rc` + `RefCell` + alloc** | **10.67** | **31.87** | **22.25** | **21.37** | **20.29** | **19.72** | **3.88** |
| samples | 5,814 | 8,911 | 18,121 | 2,747 | 3,959 | 19,047 | 6,781 |

Sub-totals cut across buckets (percent of all samples):

| | `Zend/bench` | `arrays` | `oop` | `symfony-steady` | `symfony-boot` | `strings` | `autoload` |
|---|---:|---:|---:|---:|---:|---:|---:|
| `Vec` accessors on frames / operand stack / ops, as leaf frames | 20.6 | 10.6 | 12.3 | 7.8 | 4.0 | 3.9 | 1.4 |
| frame moved by value, frame setup/teardown | 3.5 | 2.4 | 4.8 | 2.2 | 0.8 | 0.0 | 0.3 |
| SipHash (`RandomState`) | 0.1 | 0.0 | 0.5 | 0.4 | 2.1 | 0.3 | **28.7** |
| under `host_builtin_canonical` | 0.0 | 0.0 | 0.0 | **6.1** | 3.0 | 0.0 | 0.4 |
| builtin registry lookup by name | 0.2 | 0.3 | 0.0 | 0.5 | 0.4 | 0.5 | 0.1 |
| under `snapshot_entries` (`foreach`) | 0.1 | 1.3 | 0.0 | 0.4 | 0.3 | 0.0 | 0.0 |
| `memcpy` inside `PhpStr::concat2` | — | — | — | — | — | 32.9 | — |

`autoload`: SipHash callers, percent of all samples — `lower_source_impl` 26.47,
`compile_program_impl_mode` 9.03, `Vm::unit_fp` 6.49, `seed_stub_mask` 2.91,
`unit_remap_elided` 2.85, `lower_unit` 2.39.

Two classifier bugs were found and fixed during the session, both of which had produced wrong
tables: Rust v0 symbols were not demangled by `perf` (98.5 % "unclassified" on the first
attempt — fixed by adding `rustfilt` to the image), and unresolved `[libc.so.6]` `memcpy` was being
counted as process startup (36.6 % of `strings`). The committed tables are from after both fixes.

## 3. Findings that contradict PLAN.md or upstream's documentation

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
10. **The 1.77× headline does not describe CPU-bound PHP.** It is upstream's WordPress-suite
    figure. On our benchmarks phpr is 3.48×–15.25× `php -n`, and 8.2× on the DBAL suite. Upstream's
    own micro table says 2.1×–4.1× on macOS; ours are higher and the environment difference is
    not separated from the engine.
11. **`Zend/micro_bench.php` does not run**: `isset(Class::$static)` is rejected at parse time.
12. **`$s .= $int` is quadratic** (992× the oracle at N = 400,000).
13. **Scalars do not allocate, but an array write, a function call and a `foreach` each do**
    (exactly 1 malloc + free).
14. **PLAN's list of work under outcome A is mostly done already** (packed arrays, cached string
    hashes, inline caches exist). The profile points at different work: `DECISION_KERNEL.md` §6.

## 4. What changed in the repository

Three commits on branch `next`, on top of upstream `9d4ef5ba`. **No upstream file was modified
except `.gitignore`** (7 lines appended). Nothing was pushed anywhere.

| commit | content |
|---|---|
| `9552080b` | Phase 0: `baseline/`, `docker/`, `NOTICE.md`, `ARCHITECTURE_NOTES.md`, `NOTES.md`, `PLAN.md`, CI workflow |
| `be072c52` | Phase 1: `bench/` (scripts, results, profiles), `PROFILE.md`, `rustfilt` added to the image |
| (this one) | `DECISION_KERNEL.md` (DRAFT), this file completed |

Outside the repository: `../php-src` cloned at `php-8.5.7`; Docker image `rustyphp-dev:8.5.7`
(1.7 GB) and volumes `rustyphp-target`, `rustyphp-cargo-registry`, `rustyphp-scratch`; two memory
notes for future sessions. On the host, nothing was installed.

## 5. What is next

Decisions only the owner can make — none blocks reading the results:

1. **Where the fork lives.** Which GitHub org/account, and whether it is public. Nothing has been
   pushed; `git remote add origin … && git push -u origin next main` is all that is missing.
2. **The license.** PHP License 3.01 clause 4 constrains the product name. See `NOTICE.md`.
3. **Promote, amend or reject the DRAFT decision** (outcome A). `DECISION_KERNEL.md` §7 lists what
   would strengthen or overturn it; the cheapest check is one benchmark run on bare metal.

Next session, in order, if the draft stands:

1. Fix the two correctness bugs with their `.phpt`s (`count()` on `Countable` in a namespace;
   `isset(Class::$static)`), then re-run the Composer smoke test to get the wall-time comparison
   that could not be taken tonight.
2. Slice 1 of `DECISION_KERNEL.md` §6 (`.=` with a non-string operand) — smallest change, largest
   ratio, and a good first exercise of the full gate (tests, differential, `.phpt` gate,
   `bench/run.sh` before/after).
3. Slices 2 and 3 (symbol-table hashing, link-time builtin resolution).
4. Check each planned slice against upstream's `PERF_MAP.md` and vetoed-lever list first.

Open items carried over:

- The CI workflow has never run; the baseline is arm64 and hosted runners are amd64.
- The baseline was taken as root, in the default engine mode only (`PHPR_REG_LOWER` unset).
  Upstream gates both modes.
- `ext/dom`, `ext/simplexml`, `ext/filter` and `ext/pdo` are effectively unmeasured by the corpus.
- DBAL: phpr runs 5 more tests and skips 10 fewer than the oracle; `get_declared_classes()` reports
  199 classes where the oracle reports 274 in `symfony-boot`. Neither was investigated.
- The oracle's allocation counts were not measured; the comparison in `PROFILE.md` §4 rests on
  Zend's design, not on a count.
- Whether to run the container as a non-root user by default (it changes two test outcomes).
