# DECISION_KERNEL.md — **DRAFT**

> **Status: DRAFT, written 2026-09-30 at the end of the profiling session. Not a decision yet.**
> It records what the numbers support so the next session starts from an argument instead of a
> blank page. Section 7 lists what must be checked before this draft is promoted. Nothing in
> `php-types` or any hot path has been changed.

PLAN.md §3. Evidence: [`PROFILE.md`](PROFILE.md), [`ARCHITECTURE_NOTES.md`](ARCHITECTURE_NOTES.md),
[`bench/results/`](bench/results/).

## 1. Recommendation

**Outcome A — keep upstream's value model and optimise inside it.** Not B, and not C as PLAN
describes it.

Two independent reasons, either of which would be enough:

1. **The numbers do not reach the threshold.** PLAN sets B at "≥ 40 % of hot-path time inherent to
   `Rc` + `RefCell` + heap scalars" and A at "< 25 %". Measured: 10.7 %, 19.7 %, 20.3 %, 22.3 %,
   31.9 %. Heap scalars: **0 %** — scalars are inline and the census counts zero allocations for
   integer and float arithmetic.
2. **The kernel PLAN proposes to design is, for the most part, the kernel upstream already has.**
   A rewrite would spend its budget rebuilding what exists and then still face the costs that are
   actually large, none of which live in the value representation.

## 2. The profile table

Percent of samples; `perf` cpu-clock with DWARF inline attribution, ±2 points. Full table and
method in PROFILE.md §2.

| bucket | `Zend/bench` | `arrays` | `oop` | Symfony steady | `strings` | `autoload` |
|---|---:|---:|---:|---:|---:|---:|
| `Rc` inc/dec + drop glue | 8.6 | 19.6 | 9.4 | 9.4 | 3.9 | 1.3 |
| `RefCell` borrow checks | 0.3 | 0.3 | 4.7 | 4.4 | 1.6 | 0.2 |
| allocation / deallocation | 1.9 | 12.0 | 8.2 | 6.5 | 14.3 | 2.4 |
| **sum of the three** | **10.7** | **31.9** | **22.3** | **20.3** | **19.7** | **3.9** |
| hash table ops in `PhpArray` | 2.1 | 14.2 | 2.5 | 2.3 | 3.4 | 0.1 |
| string hash / compare / copy | 0.3 | 2.7 | 5.0 | 4.8 | 44.3 | 0.4 |
| VM dispatch loop itself | 54.5 | 28.4 | 34.2 | 30.9 | 5.6 | 1.6 |
| argument passing into builtins | 0.7 | 1.0 | 0.0 | 2.2 | 2.6 | 0.1 |
| builtin bodies | 0.1 | 1.1 | 0.0 | 3.1 | 15.4 | 0.1 |
| parser / HIR / compile | 1.9 | 0.6 | 1.8 | 4.1 | 0.4 | 86.9 |
| other: operator / type-juggling bodies | 17.1 | 5.3 | 3.5 | 1.7 | 2.3 | 0.1 |
| other: VM handler bodies | 10.7 | 11.8 | 21.4 | 22.6 | 4.3 | 2.1 |
| other: GC bookkeeping + sweep | 1.3 | 1.5 | 3.6 | 2.3 | 0.5 | 0.2 |
| other: engine-internal hash maps | 0.4 | 1.4 | 5.7 | 5.3 | 1.1 | 0.6 |
| other: kernel, startup | 0.3 | 0.2 | 0.1 | 0.4 | 0.3 | 4.1 |

Slowdown against `php -n` on the same benchmarks: 7.96×, 4.81×, 9.83×, 8.64×, 3.48×, 15.25×.

## 3. Reasoning

### 3.1 Against B

PLAN's description of the replacement kernel, item by item, against what is in the tree today:

| PLAN asks for | upstream today | evidence |
|---|---|---|
| 16-byte `Zval` with inline scalars | **has it** | `size_of::<Zval>() == 16`, measured and compile-time asserted |
| refcounted `ZStr`, cached hash | **has it** — single allocation, 32-byte header, DJBX33A cached | `zstr.rs:38`, `:414` |
| … with interned flag | no interner; literals are shared per function constant pool | `bytecode.rs:104` |
| `ZArr` packed/hash dual mode, Zend-style ordered buckets | **has it** — `Packed(Vec<Option<Zval>>)` / ordered entries + `u32` index | `array.rs:124`, `:194` |
| `ZRef` | `Rc<RefCell<Zval>>`, 40-byte block | `zval.rs:31` |
| `ZObj` with slot table | **has it** — slots aligned to a shared per-class layout, inline caches | `object.rs:714`, `bytecode.rs:200` |
| … + handlers | no vtable; internal classes are PHP prelude + host builtins | ARCHITECTURE_NOTES §4 |
| raw refcounts via `Cell<u32>` | strings: **has it** (`Cell<usize>`); arrays/objects use `Rc` | `zstr.rs:40` |
| `unsafe` confined to the crate | already so for the value model (17 sites, no `SAFETY:` comments) | ARCHITECTURE_NOTES §8 |
| **same public API for ~500 builtins** | **not available** | below |

What B would actually buy is the delta between `Rc<T>` and a hand-rolled refcount, and between
`RefCell` and unchecked access:

- `Rc`'s count is already non-atomic. Dropping the unused weak count saves 8 bytes per array and
  object block and nothing per operation.
- The `RefCell` flag costs 4–5 % on object-heavy code and ~0 elsewhere. That is the entire
  addressable `RefCell` budget, and removing it means `unsafe` aliasing in the object path.
- Upstream already measured and vetoed NaN-boxing and SSO strings (both slower) and an object
  arena.

Against that, the **blast radius**. The representation is the API — builtins and the VM
destructure the enum directly:

| pattern, outside `php-types` | sites |
|---|---:|
| `Zval::…` constructed or matched | 4,611 |
| `Zval::Array(Rc::new(…))` — the `Rc` built by hand | 347 |
| `Key::…` | 577 |
| `PhpStr::…` constructors | 678 |
| `.borrow()` / `.borrow_mut()` on values | 672 |
| `Rc::make_mut` | 25 |

Changing the payload type of `Zval::Array` or `Zval::Object` is a compile error at several
hundred sites, in code whose correctness is pinned by a 3,041-test gate, a 37,835-case
differential and framework suites. "The builtins recompile unchanged" cannot be delivered. PLAN
asks for every public item of `php-types` to be marked keep / change-signature / remove; under
outcome A that inventory is not needed and was not produced.

### 3.2 Against C as written

C says: keep `Rc` for objects, replace strings and arrays first. Strings were already replaced by
upstream (S-124), behind their API. Arrays are the one place where a representation change could
pay — `arrays` is the 31.9 % benchmark — but the measured causes of that figure are not the
layout:

- `foreach` clones every element into a new vector before iterating (PROFILE §5.4);
- every element write allocates and frees once (PROFILE §4);
- 3–4 `Zval` clones per element write.

All three are fixable with `PhpArray` and `Zval` exactly as they are. Whether a single-allocation
array would still be worth it can only be asked **after** they are fixed, with a new profile. That
question is kept open in §5, item 9.

### 3.3 For A — where the time is, and that it is reachable

Everything below is outside the value representation, and each line has a measured size.

| cost | measured | nature |
|---|---|---|
| Dispatch loop re-indexing `frames[top]`, its stack and its ops on every access | up to 20.6 % of samples | loop structure |
| Frames moved by value on call/return | 3.5–4.8 % | loop structure |
| 1 malloc+free per array element write | exact count | handler logic |
| 1 malloc+free per user function call | exact count | handler logic |
| 1 malloc + 1 realloc per `foreach`, 5.25 clones per element | exact count | handler logic |
| 9 `Zval` clones per method call | exact count | handler logic |
| `$s .= <int/float>` quadratic | 992× at N = 400k | missing fast path |
| Per-include symbol-table rebuild under SipHash | 28.7 % of autoload | algorithm + hasher |
| `host_builtin_canonical` linear scan at run time | 6.1 % of a Symfony request | should be link-time |
| Builtin resolved by name on every call | 2.2–2.6 % | should be link-time |
| Compile pipeline 3.1–3.5× slower than the oracle's, rerun per process | 1.35–1.5 ms / 1,000 lines | no cache |
| Startup floor | 15.8 ms vs 6.3 ms | prelude compiled per process |

Where execution is already inside a builtin, phpr is within 1.2–2.5× of the oracle and beats it
on `str_replace` (0.50×) and `var_export` (0.60×). The value model is not what stands between
phpr and PHP there.

### 3.4 What PLAN's list under A already is

PLAN names, for outcome A: "packed-array mode inside `PhpArray`, cached string hashes, interned
literals, a bytecode cache, inline caches". Three of the five exist (packed mode, cached hashes,
property and method inline caches); literals are shared though not interned. Only the bytecode
cache is new. The work list in §5 is therefore different from PLAN's, and comes from the profile.

## 4. One page: bytecode cache

**Goal.** Stop lexing, parsing, lowering and compiling files that have not changed. Measured
ceiling: 1.35–1.5 ms per 1,000 lines for a single file; ≈ 0.25 ms per class through the autoloader
(87 % of `autoload`, 36 % of `symfony-boot`); 9.5 ms of startup floor if the prelude is included.

**What exists.** A thread-local, in-memory *unit cache* (`CachedUnit`, `vm/mod.rs:15852`):
key `UnitKey { path, mtime (s, ns), size, reg_mode }` (`:15960`), 4 ways per file, owns
`Rc<Module>`, guarded by a fingerprint of the VM state (`Vm::unit_fp`, `:7095`). It already
serves includes within a process and across requests in `php-server`.

**The constraint that shapes the design.** A compiled unit is **not position-independent**.
Lowering bakes in global slot indices and class ids, and relocation rewrites them into the VM's
global id space. That is why a hit requires an equal `unit_fp`: the hash of the whole load chain,
table sizes, and every `(class name, id)` pair. Two consequences:

- `unit_fp` walks the entire class index and re-hashes each entry with SipHash **on every
  include** (6.5 % of `autoload` on its own) — an O(loaded classes) charge per file.
- A unit compiled after load order X cannot be reused after load order Y.

**Design — three steps, each independently measurable.**

1. **Make the existing cache cheap (no format work).** Maintain `unit_fp` incrementally — fold each
   registration into a running digest when it happens instead of recomputing from the tables — and
   switch the lowering/compile symbol maps from `RandomState` to FxHash, which the rest of the
   engine already uses. Expected: most of the 28.7 % SipHash share of `autoload`, without a cache.
   Do this first: a disk cache does not remove link cost.
2. **In-memory cache, process-wide per worker, prelude included.** Promote the thread-local cache
   to the worker's lifetime (it already is, per thread), add the compiled prelude image as unit 0
   keyed by the binary's build hash, and expose hit/miss counters. Validation per include is one
   `stat()` (path, mtime, size) as today; a `validate_timestamps=0` equivalent skips it in
   production.
3. **On-disk cache.** One file per `(canonical path, mtime, size, reg_mode, fingerprint)`.
   - Location: `$PHPR_CACHE_DIR`, default `$XDG_CACHE_HOME/phpr/<build-hash>/`; file name = hash of
     the key. The build hash in the directory name invalidates everything on upgrade.
   - Header: magic, format version, build hash, target triple, flags, payload length, checksum.
     Any mismatch ⇒ ignore the file and compile. Writes are `tmp` + `rename`, so a reader never
     sees a partial file; concurrent workers may both compile and both write, last one wins.
   - Payload: the `Module` **before relocation**, plus its relocation inputs (`class_remap`,
     `static_off`, `reserved_base`, `new_locals`, `seed_delta`) exactly as `CachedUnit` holds them.
   - Required source changes, all outside `php-types`: serialisation for `Op` (~200 variants),
     `Func`, `Const`, `CompiledClass`, and the HIR fragments a `Module` embeds (`DeferredDecl`,
     `LoweredTrait`); inline caches (`PropIc`/`MethodIc`, `Rc<Cell<…>>` inside ops) become indices
     into a per-load side table and are never serialised; `ZStr` constants serialise as bytes.
   - Start context-dependent (fingerprint in the key), because that is what upstream's machinery
     has been validated for, and **measure the hit rate** on a framework boot, where load order is
     deterministic. Move to position-independent units with a separate link step only if the hit
     rate is poor.

**What it must never do.** Change observable behaviour. The cache is an optimisation of a pure
function of (source bytes, engine build, VM state); on any doubt it recompiles.

**Gate for landing.** The `.phpt` corpus run three ways — cache off, cold cache, warm cache — with
identical pass/fail lists by name; differential at 0; `bench/run.sh` recorded before and after.

## 5. One page: worker mode

**Goal.** Boot the application once per worker, then serve requests in a loop, as FrankenPHP's
worker mode does. Single-threaded per worker, N workers, no PHP state shared across threads.

**What exists.** `php-server` has two modes (ARCHITECTURE_NOTES §10):

- *cli-server* — a complete SAPI (`$_SERVER`, `$_GET`, `$_POST`, `$_COOKIE`, `$_FILES`,
  `$_REQUEST`, `php://input`), sequential on one thread. WordPress runs on it.
- *axum* — a worker-thread pool that passes **no request data** to PHP: every superglobal is
  empty. It proves the threading model and nothing else.

Both build a **fresh `Vm` per request**. Upstream tried a reused `Vm`, rejected it (WP-77.2), and
states there is no `Vm::reset()`. `Vm::request_end()` (`vm/mod.rs`) resets about 25 categories of
*ephemeral* state — output buffers, superglobals, handlers, the frame stack, GC buffers, resource
id counters — and explicitly does **not** reset function statics, static properties or the class
table, which die with the `Vm`.

**Design.**

1. **Transport.** Replace the axum pool's `WorkerHandlerMeta { path, source }` with the full
   request (method, URI, query, headers, body) and route it through the same
   `WebRequest` → `seed_web_superglobals` path the cli-server uses. This makes the pool a real
   SAPI and is needed whatever happens to worker mode. N OS threads, one `Vm` each, requests
   dispatched over a channel; a Rust panic in a worker aborts the process (upstream's existing
   fail-fast policy).
2. **Runtime API.** One new host builtin, the equivalent of `frankenphp_handle_request()`:
   ```php
   // worker.php — run once per worker
   $app = require __DIR__ . '/bootstrap.php';
   while (phpr_handle_request(function () use ($app) { $app->handle(); })) {
       gc_collect_cycles();
   }
   ```
   `phpr_handle_request(callable): bool` blocks until a request arrives, performs the per-request
   reset, seeds the superglobals, runs the callable, flushes the response, runs the request's
   shutdown steps, and returns `true` — or `false` when the worker should exit (shutdown, or
   `max_requests` reached). The `Vm`, the class table, loaded units and everything the boot script
   created stay alive across iterations.
3. **Per-request reset — what is reset, what is not.**

   | state | per request | note |
   |---|---|---|
   | `$_SERVER`, `$_GET`, `$_POST`, `$_COOKIE`, `$_FILES`, `$_REQUEST`, `php://input` | reseeded | |
   | output buffers, response headers/code, `headers_sent` state | reset | flush before reset (upstream's rule) |
   | `error_get_last`, error/exception handler stacks set *during* the request | reset to the post-boot snapshot | handlers installed at boot must survive |
   | shutdown functions registered during the request | run, then cleared | |
   | session | closed and written at request end | |
   | uploaded temp files | deleted | |
   | `ini_set` during the request | restored to the post-boot snapshot | |
   | **`$GLOBALS` / global variables** | **kept** | FrankenPHP semantics; the app owns them |
   | **function `static` variables, static properties** | **kept** | see below |
   | **class/function/constant tables, loaded units** | **kept** | the point of worker mode |
   | objects created at boot | kept | |
   | object ids, resource ids | **not** reset | they must stay unique across the worker's life |

   The last row contradicts today's `request_end()`, which resets `next_object_id` to 1. With live
   boot-time objects that would hand out duplicate ids. Worker mode therefore needs a reset that is
   a *subset* of `request_end()`, taken against a snapshot made when `phpr_handle_request` is first
   called — not `request_end()` itself.
4. **Static-cache implications — to document for application authors.** Statics and static
   properties persist across requests. Code that memoises per-request data in a `static` (the
   current user, the request locale, "already initialised" flags) will leak it into the next
   request. This is the same contract as FrankenPHP worker mode, Swoole and RoadRunner; Laravel
   Octane and Symfony (`kernel.reset`) already provide reset hooks for it. Drupal assumes fresh
   process state on every request and will need explicit reset points — how many is unknown until
   it runs. The engine cannot detect misuse; it can offer `max_requests` recycling as a safety net.
5. **Memory.** A long-lived `Vm` makes the cycle collector matter: `request_shutdown` runs
   `break_request_cycles`, which assumes everything dies. Worker mode must instead collect cycles
   (`gc_collect_cycles()` between requests) and recycle the worker after `max_requests` or above a
   memory ceiling.
6. **Explicitly out of scope.** Shared mutable PHP state across threads; upstream's multi-threaded
   direction; async I/O inside a request.

**Gate for landing.** A request-isolation battery: N sequential requests through one worker must
produce responses byte-identical to N fresh-process runs for a stateless script, and must differ in
exactly the documented ways for a script using statics. Target from PLAN §4: within 1.2× of `php`
with opcache on `bench/symfony-boot.php`. Today the per-request path is 8.64× `php -n`; worker
mode removes boot and compile from the request but not that ratio.

## 6. Proposed order of work (each slice gated: tests, differential, `.phpt` gate, benchmarks)

Ordered by measured size ÷ estimated risk. None touches `php-types`' public API.

| # | slice | measured ceiling | touches |
|---:|---|---|---|
| 1 | ~~`.=` with int/float operand: convert, then take the in-place path~~ **done, session 2** — and for every target form (property, array element, static, reference), which were all quadratic too | 992× → 1.7× on that pattern; 231–706× on the others | five handlers + one gate op |
| 2 | Symbol maps under FxHash; incremental `unit_fp` | up to 28.7 % of autoload | lowering, unit linking |
| 3 | Resolve builtins and `host_builtin_canonical` at compile/link time | 6.1 % + ~2 % of a request | compiler, call ops |
| 4 | `foreach` by position over a held `Rc` clone instead of a snapshot | 1 alloc + 5 clones/element; `foreach` 8–10× → ? | iterator state |
| 5 | Remove the per-array-write and per-call allocation | 1 malloc+free each | path machinery, call binder |
| 6 | Dispatch loop: current frame held outside the `Vec`, cached stack/ops slices | up to 20.6 % | `run_loop` |
| 7 | Bytecode cache, steps 2 and 3 of §4 | 36 % of a short Symfony run | new module |
| 8 | Worker mode | removes boot per request | `php-server` |
| 9 | **Re-profile.** Only then ask whether arrays need a new representation (C-lite) | — | — |

Two correctness bugs found on the way blocked the target frameworks; **both fixed in session 2**
with their `.phpt`s: `count()` on a `Countable` through any dynamic call (it broke Composer
2.10.1, which now runs end to end), and the missing quiet static-property fetch behind
`isset(Class::$static)`.

Item 6 is the largest and the riskiest: upstream records that `run_loop` is layout-sensitive
("icache-bound"), with null-lever noise bands of several ns/iter, and its conventions allow no new
`unsafe` outside `php-types`. It must be done in safe Rust and measured A/B against the unchanged
binary, not judged by reading.

## 7. Why this is still a draft

1. **One environment.** Everything was measured in a Linux/arm64 VM under Docker on a laptop,
   against the docker-library PHP build. Upstream measures natively on macOS against Homebrew's
   PHP and reports 2.1–4.1× on micro categories where we see 3.5–15×. The *ratios* between buckets
   should transfer; the absolute slowdowns may not. Needs one confirmation run on bare metal.
2. **No hardware counters.** Timer sampling only. Claims about bounds checks and pointer reloads
   (§3.3, first row) are inferred from inlined-frame attribution, not from a disassembly. Upstream
   insists on disassembly before believing a mechanism; so should we, before slice 6.
3. **No target workload.** Drupal and Laravel do not run yet; Symfony HttpKernel with three routes
   is the closest proxy. WordPress — the workload behind upstream's 1.77× — was not run. The
   decision should be re-read once a real Drupal request can be profiled.
4. **Attribution is ±2 points and rule-based.** The 31.9 % on `arrays` sits between PLAN's two
   thresholds. The argument for A there rests on §3.2 (the causes are algorithmic), which is a
   reading of the census and the code, and will only be proven by doing slices 4 and 5 and
   re-measuring.
5. **Upstream has a long measured history we have only skimmed** (`PERF_MAP.md`, 182 session
   files, a list of vetoed levers). Some slices in §6 may already have been tried and rejected for
   reasons recorded there. Each must be checked against that record before work starts.
6. **The thresholds are PLAN's.** They were written before it was known that the value model is
   already compact. If the owner's intent behind B was "control the kernel ourselves" rather than
   "fix a measured cost", that is a product decision this document cannot make.
