# ARCHITECTURE_NOTES.md — what upstream's engine actually is

PLAN.md §1.3. Read at upstream commit `9d4ef5ba` (2026-09-29). Paths are relative to
`php-rust/crates/`. Sizes were measured, not read off comments: an out-of-tree probe crate
(`size_of`/`align_of` on `php-types`' public types, release build, `aarch64-unknown-linux-gnu`,
rustc 1.98.1) — nothing was added to `php-types`.

**The one-paragraph version.** PLAN.md §3 option B describes a kernel to *design*: 16-byte `Zval`
with inline scalars, refcounted string with cached hash, packed/hash dual-mode array with Zend-style
ordered buckets, slot-table objects, raw `Cell` refcounts. **Upstream already has almost all of
that.** What it does not have is an API boundary: `Zval` is a public enum whose variants *are*
`Rc<PhpArray>` and `Rc<RefCell<Object>>`, and ~4,600 call sites outside `php-types` pattern-match
on them. The representation is the API.

---

## 1. `Zval`

`php-types/src/zval.rs:17` — a plain Rust `enum`, 14 variants.

| | measured |
|---|---:|
| `size_of::<Zval>()` | **16** |
| `align_of::<Zval>()` | 8 |
| `size_of::<Option<Zval>>()` | 16 (niche; no extra tag) |

Pinned by compile-time assertions at `php-types/src/array.rs:88-99` (size, align, and
`Option<Zval>` = `Zval`), plus a seal that `Bool`/`Long`/`Double` payloads are `Copy`
(`array.rs:107-112`).

| variant | payload | heap? | block size when allocated |
|---|---|---|---:|
| `Undef`, `Null`, `Bool`, `Long(i64)`, `Double(f64)` | inline | **no** | — |
| `Str(ZStr)` | thin pointer to one block | yes | 32 B header + bytes |
| `Array(Rc<PhpArray>)` | `Rc` | yes | 88 B (16 `RcBox` + 72) + element buffers |
| `Ref(Rc<RefCell<Zval>>)` | `Rc<RefCell>` | yes | 40 B (16 + 24) |
| `Object(Rc<RefCell<Object>>)` | `Rc<RefCell>` | yes | 152 B (16 + 136) + props buffers |
| `Closure(Rc<Closure>)` | `Rc` | yes | 136 B |
| `Generator(Rc<RefCell<GenState>>)` | `Rc<RefCell>` | yes | 16 + 88 |
| `Resource(Rc<RefCell<Resource>>)` | `Rc<RefCell>` | yes | 16 + 144 |
| `WeakHandle(Weak<RefCell<Object>>)` | `Weak` | no new block | — |
| `ArgPlace(Rc<ArgPlace>)` | `Rc` | yes | call-window only, never escapes |

**Scalars do not allocate.** Integers, floats, bools and null are inline in the 16 bytes. Whether
the *loop counter* allocates is a VM question, answered by measurement in PROFILE.md.

`Clone` is derived (`zval.rs:15`): a scalar clone is a 16-byte copy, a heap variant clone is a
non-atomic refcount increment. Upstream has measured and **vetoed** NaN-boxing (`AGENTS.md`
"Veti permanenti") and an SSO string (`zstr.rs:14-20`, measured +1.5 % / +2.5 % *slower*).

## 2. `PhpStr` / `ZStr`

`php-types/src/zstr.rs:38` (`PhpStr` header) and `:66` (`ZStr` handle).

- **Not `Rc<[u8]>` and not `Rc<Vec<u8>>`.** A hand-rolled single allocation in the shape of
  `zend_string`: a `#[repr(C)]` 32-byte header `{ rc: Cell<usize>, hash: Cell<u64>, len, cap }`
  followed by the bytes in the *same* block. `ZStr` is an 8-byte `NonNull<PhpStr>`.
  (`size_of::<PhpStr>() == 32` asserted at `zstr.rs:48`.)
- **Refcount:** raw non-atomic `Cell<usize>`, incremented in `Clone` (`zstr.rs:166`), decremented
  in `Drop` (`:179`) with the free path outlined `#[cold]` (`:195`). This is `unsafe` code.
- **Cached hash: yes.** DJBX33A, lazily computed, 0 = "not computed", top bit forced
  (`zhash`, `zstr.rs:414`). `Hash for PhpStr` feeds the cached value (`:438`), so an array-key
  string hashes once in its lifetime.
- **Interned strings: no interner.** There is no global intern table and no "interned" flag.
  Literals are *shared* instead: the per-function constant pool holds prebuilt `ZStr`s
  (`Const::Str`, `php-runtime/src/bytecode.rs:104`), and `Op::PushConst` is a refcount bump
  (`Const::to_zval`, `bytecode.rs:118`). Two equal literals in two functions are two blocks.
- **In-place append:** `ZStr::try_append` (`zstr.rs:126`) grows the block with amortised `realloc`
  when `rc == 1`; shared strings fall back to `concat2` (copy). This is what makes `.=` O(n).
- Equality keeps `Rc`'s pointer-identity fast path by hand (`zstr.rs:222`).

## 3. `PhpArray`

`php-types/src/array.rs:382`. `size_of::<PhpArray>() == 72`.

- **Own implementation** — not `IndexMap`, not `HashMap`. Insertion-ordered, Zend-shaped.
- **Packed mode: yes** (`Repr`, `array.rs:124`):
  - `Packed(Vec<Option<Zval>>)` — 16 B per slot, key = position, no key storage, no index.
  - `Hashed { entries: Vec<Option<(Key, Zval)>>, index: KeyIndex }` — 32 B per entry in insertion
    order, plus `KeyIndex` (`:194`): an open-addressing table of `u32` positions into `entries`
    (linear probing, power-of-two, load ≤ 1/2). The key is stored once, in the entry — the same
    idea as Zend's `arData` + `uint32` hash slots.
  - Hashed arrays with ≤ 8 entries carry **no index at all** and are scanned linearly
    (`SCAN_MAX`, `array.rs:152`).
  - One-way escalation packed → hashed on a string key, a hole, a negative key, or a write into a
    tombstone (`to_hashed`, `:686`). Never converts back.
- **Tombstones** (`None`) on `unset`, like Zend's `IS_UNDEF` buckets; `next_free` never decreases
  (`:387`); compaction in `compact` (`:1027`).
- **Hashing:** int keys → Murmur3 `fmix64` of the value; string keys → `fmix64` of the cached
  DJBX33A (`Key::khash`, `:171`).
- **Copy-on-write / separation:** the value is `Zval::Array(Rc<PhpArray>)`; every write goes
  through `Rc::make_mut` (e.g. `php-runtime/src/vm/run.rs:2505, 2516`, `vm/arrays.rs:171`), which
  mutates in place when the refcount is 1 and deep-clones the `PhpArray` otherwise. 22 `make_mut`
  sites in `php-runtime`, 3 in `php-builtins`.
  `Clone for PhpArray` (`array.rs:452`) clones every element (refcount bumps) and applies
  `zend_array_dup`'s rule for references: a `Ref` element held only by this array is split into a
  plain value (`dup_element`, `:435`).
- **Arrays are not behind a `RefCell`.** Mutation needs `&mut`, obtained from `Rc::make_mut`.
- **Internal pointer:** a cursor position packed into a `u32` with the "may hold containers" GC
  flag (`cur_holds`, `:406`); `ptr_reset/next/prev/end/current/key` at `:1077-1135`. Carried by
  clone.
- **Iteration under modification — `foreach` takes a full snapshot.** `Op::IterInit` builds
  `IterState::ByVal { entries: Vec<(Zval, Zval)>, pos }` (`vm/mod.rs:2833`) by calling
  `snapshot_entries` (`vm/arrays.rs:971`), which **copies every `(key, value)` pair into a fresh
  `Vec` at loop entry**: 32 B and one or two refcount increments per element, before the first
  iteration runs. Zend instead holds a reference to the array and walks it by position. By-ref
  `foreach` snapshots only the keys (`IterState::ByRef`). See PROFILE.md for what this costs.
- `Drop` is hand-written to drain elements inline (`array.rs:504`, `unsafe`).

## 4. References, objects

**References (`&`)** — `Zval::Ref(Rc<RefCell<Zval>>)` (`zval.rs:31`): a shared mutable cell;
any number of variables/elements alias it. Invariant: the inner value is never itself a `Ref`.
A local slot that is a reference simply *holds* a `Zval::Ref`; readers `deref_clone()`
(`zval.rs:348`). Created through `zcell()` (`zval.rs:112`). 40-byte heap block per reference.

**Objects** — `Zval::Object(Rc<RefCell<Object>>)`, `Object` at `php-types/src/object.rs:19`
(128 B; 152 B heap block with `RcBox` + `RefCell` flag).

- **Property slots: yes.** `Props` (`object.rs:714`) = `slots: Vec<Option<Zval>>` aligned
  index-for-index with a per-class shared `Rc<PropsLayout>` (`:647`), plus
  `dyn_entries: Vec<(Box<[u8]>, Zval)>` for dynamic properties (linear scan). Declared-property
  access by compile-time slot index: `get_slot` / `replace_slot` (`:844`, `:877`).
- **Inline caches exist:** `PropIc` and `MethodIc` (`bytecode.rs:200`, `:268`), each an
  `Rc<Cell<(epoch, class, slot, …)>>` embedded in the op, invalidated by a global epoch.
- **No handlers vtable.** There is no `zend_object_handlers` equivalent. User-class behaviour is
  resolved through the VM's global class table (`Vm::classes`, `vm/mod.rs:3021+`;
  `CompiledClass`, `bytecode.rs:1980`). Internal classes (DOM, PDO, mysqli, SPL, Reflection,
  DateTime…) are **written in PHP** in an embedded prelude (§8) that calls host builtins, with a
  few "opaque handle" classes special-cased by name (`is_opaque_handle_class`, `object.rs:515`).
- Every object also carries `class_name: ZStr`, `info: Rc<ObjectInfo>`, `rare: Option<Box<ObjRare>>`,
  lazy-object fields and GC marks (`GcMark`, `:98`).
- PHP destructors never run from Rust `Drop`; teardown is an explicit sweep. `Drop for Object`
  (`object.rs:366`) only orders handle-id release, through a depth-bounded trampoline
  (`drop_bounded`, `:315`) so a 500k-node linked list does not overflow the native stack.

## 5. How shared values are mutated

| value | mechanism | cost on the hot path |
|---|---|---|
| array | `Rc::make_mut` on `Rc<PhpArray>` | one strong-count check (+ weak-count check) per write; deep clone when shared |
| object | `Rc<RefCell<Object>>` + `borrow()` / `borrow_mut()` | a borrow-flag read/write per access |
| reference | `Rc<RefCell<Zval>>` + `borrow()` / `borrow_mut()` | same |
| string | `ZStr::try_append` if `rc == 1`, else copy | one refcount read |

Counts outside `php-types`: `.borrow()` 491 sites, `.borrow_mut()` 181 sites (448 + 139 in
`php-runtime`, 43 + 42 in `php-builtins`). `Rc::get_mut` is used in 9 places, none on a hot value
path. No `unsafe` is used to bypass `RefCell` — the borrow checks are real and are paid.

## 6. The VM

Pipeline: `mago` parser → AST → HIR (`php-runtime/src/hir.rs`, `lower/`) → bytecode
(`compile/`) → dispatch loop (`vm/run.rs`). Single engine; the tree-walker is gone.

- **Instruction encoding:** `pub enum Op` (`bytecode.rs:339`), ~200 variants, a fat Rust enum
  with operands inline. **`size_of::<Op>() == 48`** (asserted, `compile/reg_lower.rs:1250`). A
  function's code is `Func::ops: Vec<Op>` with a parallel `lines: Vec<Line>` (`bytecode.rs:1433`).
  Not fixed-width words; no computed-goto threading.
- **Dispatch:** `Vm::run_loop` (`vm/run.rs:1488`) — a `loop` that reads `frames[top].ip`, takes
  `&func.ops[ip]` by reference (no per-tick `Op` clone), bumps `ip`, and enters one big
  `match op` (`run.rs:1555`). `run.rs` is 7,404 lines; `vm/mod.rs` is 25,968.
- **Operand kinds:** primarily a **stack machine** — values flow through the frame's operand
  stack. `enum Operand { Stack, Slot(u16), Temp(u16), Const(u16) }` (`bytecode.rs:1417`) is a
  register-style operand form retrofitted onto the *hot* ops by a lowering pass
  (`compile/reg_lower.rs`), selectable with `PHPR_REG_LOWER=0|1`; upstream's gates run both modes.
  Many fused superinstructions exist (e.g. `ConcatAssignSlot`, `bytecode.rs:373`).
- **Frame layout:** `struct Frame` (`vm/mod.rs:2545`): `func`, `module`, `ip`,
  **`slots: Vec<Zval>`** (CVs + register temps), **`stack: Vec<Zval>`** (operand stack / TMPs),
  `this`, `class`, `static_class`, `iters: Vec<IterState>`, flags, and a lazily-boxed cold
  extension (`FrameExt`, `:2615`). Frames live in `Vm::frames: Vec<Frame>`; PHP recursion does
  not use the native stack. Each frame owns two heap `Vec`s, recycled through a 64-deep freelist
  (`FramePool`, `vm/mod.rs:2800`) so a call normally allocates nothing.
- **User-function calls:** the exact-arity "simple call" path moves arguments straight from the
  caller's operand stack into the callee's first slots (`vm/run.rs:3725-3775`), no container.
  Other shapes build a `Vec<Zval>` of arguments and go through `bind_params`.
- **Calls into builtins:** `BuiltinFn = fn(&[Zval], &mut Ctx) -> Result<Zval, PhpError>`
  (`php-runtime/src/builtin.rs:65`). Arguments are **popped (moved) off the operand stack**, not
  cloned, into a native-stack array for arity ≤ 4 and a `Vec` beyond (`vm/run.rs:3861-3913`);
  passed as a slice; the builtin returns an owned `Zval`.
  **The builtin is looked up by name on every call**: `self.registry.get(&name[..])`
  (`run.rs:3872`) hashes the function-name bytes with FxHash and probes a
  `FxHashMap<Vec<u8>, Builtin>` each time `Op::CallBuiltin` executes. There is no resolved
  function pointer in the op.
  `value_builtin_call` (`run.rs:1402`) then runs a chain of name comparisons (`var_export`,
  `print_r`, `var_dump`, `count`/`sizeof`, stream-wrapper ops…) before every builtin call.

## 7. Builtins — registration and signature

- Registry type: `pub type Registry = FxHashMap<Vec<u8>, Builtin>` (`builtin.rs:89`);
  `enum Builtin { Value(BuiltinFn), RefFirst(BuiltinRefFn) }` (`:81`).
- `php_builtins::registry()` (`php-builtins/src/lib.rs:76`) registers **535** value builtins by
  hand (`add(b"name", module::func)`); 495 functions in `php-builtins` have the
  `(&[Zval], &mut Ctx)` shape. The remaining ~480 of upstream's "1017 functions" are *host*
  builtins implemented inside the VM (`vm/host.rs`, 7,727 lines) because they need to re-enter
  the VM (callbacks, reflection, streams, PDO, sessions…), or are PHP functions in the prelude.
- `Ctx` (`builtin.rs:16`) gives a builtin the output buffer and the diagnostics sink. A pure
  builtin **cannot call back into PHP**; `__toString`/`__debugInfo` results are precomputed by
  the VM and handed in through `Ctx`.
- **Can a kernel swap be API-compatible? No, not as things stand.** Builtins do not talk to an
  abstract value API. They destructure the enum:

  | pattern, outside `php-types` | sites |
  |---|---:|
  | `Zval::…` (construct or match) | 4,611 |
  | `Zval::Array(Rc::new(…))` — builds the `Rc` by hand | 347 |
  | `Key::…` | 577 |
  | `PhpStr::…` constructors | 678 |
  | `.borrow()` / `.borrow_mut()` | 672 |
  | `Rc::make_mut` | 25 |

  Changing `Zval::Array`'s payload from `Rc<PhpArray>` to anything else is a compile error at
  hundreds of sites. `PhpStr`/`ZStr` *is* encapsulated (private fields, constructor funnel) and
  was already swapped once behind its API (S-124); `PhpArray`'s internals are private too. The
  exposed parts are the `Rc` and `RefCell` wrappers in the `Zval` variants themselves.

## 8. Where `unsafe` is used

290 occurrences of the word, 280 of them real `unsafe` blocks/fns/impls:

| crate | count | what |
|---|---:|---|
| `php-types` | 222 | FFI to system libs: `tidyio.rs` 58, `xsltio.rs` 51, `gdio.rs` 45, `zlibio.rs` 7, `netio.rs` 3, `fsown.rs` 5 (169 total) · census instrumentation `memcensus.rs` 36 · **value model: `zstr.rs` 15, `array.rs` 2** |
| `php-runtime` | 23 | `vm/host.rs` 13 (libc/FFI), `vm/mod.rs` 5, `bytecode.rs` 3, others 1–2 |
| `php-server` | 18 | global-allocator wrappers, signal/libc |
| `php-builtins` | 10 | libc (`file.rs`, `env.rs`), 1 in `mbstring.rs` |
| `php-cli` | 7 | allocator wrapper, libc |
| `phpt-runner` | 0 | |

So README's "no `unsafe`" is about the *VM's control flow* (generators/fibers on explicit frames).
The value model already contains hand-written `unsafe` refcounting (`ZStr`) and an `unsafe`
`Drop` (`PhpArray`). `zstr.rs` has 15 `unsafe` sites and 0 `// SAFETY:` comments; `array.rs` 2 and 0.

## 9. Bytecode caching

- **Between runs (on disk): none.** No serialization dependency exists in any manifest
  (`serde`, `bincode`, `rkyv`, `postcard`: zero hits). `Func` holds `Rc`-based inline caches and
  `ZStr` constants, so it is not serializable as-is. Every `phpr script.php` process lexes,
  parses, lowers and compiles the script **and the embedded prelude**.
- **The prelude** is ~9,300 lines of PHP embedded with `include_str!`
  (`php-runtime/src/lower/mod.rs:749-768`; `prelude/{core,spl,reflection,date,pdo,sqlite3,dom,
  session,tidy}.php` plus `prelude_{ns,mysqli,bcmath,gd,gmp,fileinfo}.php`, ≈ 405 KB of source).
  It defines the internal classes. Its per-process cost is the startup floor measured in
  PROFILE.md. A stub-elision mechanism exists (`PHPR_STUB_ELISION`, `vm/mod.rs:16160`).
- **In memory, within a process: yes** — the *unit cache* (`CachedUnit`, `vm/mod.rs:15852`):
  thread-local, keyed by path + a fingerprint of the VM's class/function tables (`Vm::unit_fp`,
  `:7095`), owns `Rc<Module>`, ways-evicted. Kill switch `PHPR_UNIT_CACHE=0` (`:16168`). Used for
  `include`d units, and for the main script in the server.

## 10. `php-server`

Two modes (`php-server/src/main.rs:1-12`):

1. **`--cli-server` (the default, and the only complete one)** — reuses `ferro -S`
   (`php-cli/src/server.rs`): a work-alike of PHP's built-in dev server. **Sequential, one
   thread**, `Connection: close` on every response. It builds a `WebRequest`, publishes it
   through a thread-local (`php_types::sapi::set_web_request`, `server.rs:647`), and
   `Vm::request_start` seeds the superglobals from it (`vm/mod.rs:3717`,
   `vm/websapi.rs:391-491`). Populated: **`$_SERVER`, `$_GET`, `$_POST`, `$_COOKIE`, `$_FILES`
   (multipart parsed, oracle-pinned shape), `$_REQUEST`, and `php://input`**
   (`sapi::request_body`, `php-types/src/sapi.rs:76`). This is the mode WordPress runs on.
2. **`--axum` (feature `axum-server`, experimental)** — Axum front end → mpsc → N OS worker
   threads (`worker_pool.rs`). **It does not pass the request to PHP.** `WorkerHandlerMeta`
   (`worker_pool.rs:136`) carries only the script path and source; `set_web_request` is never
   called from `php-server`; `vm.request_start(None, &[])` (`:935`). No method, URI, query, headers
   or body reach the script, so every superglobal is empty. Its gates test "hello" and a
   stateful counter.

**Request lifecycle — a fresh `Vm` per request, in both modes.** `execute_request`
(`worker_pool.rs:759`): new `RetainSet` → acquire main module (unit-cache hit or compile) →
`vm_new` → `request_start` → `run` → `request_shutdown` → capture output → `request_end` → `Vm`
dropped. A resident/reused `Vm` was tried and explicitly rejected upstream (WP-77.2); there is no
`Vm::reset()`. What survives between requests is the thread-local unit cache, i.e. compiled
bytecode only — never class tables, statics, or objects.

There is no worker mode in the FrankenPHP sense (boot the application once, loop on requests
inside PHP). PLAN.md §3's worker-mode design therefore starts from the cli-server SAPI layer plus
a new "handle request" entry point; the axum pool is a thread pool around one-shot `Vm`s.

## 11. Things found while reading that are not in upstream's docs

- **`PHP_OS` is hard-coded.** `phpr` reports `PHP_OS = Darwin` on Linux. Anything branching on the
  OS family will take the macOS path in our container.
- **`ferro -v` is not implemented** (`Could not open input file: -v`). Tools that probe the PHP
  version by running the binary with `-v` will fail.
- **The axum server is not a usable SAPI yet** (§10).
- **Upstream's corpus gate cannot be reused as-is.** `scripts/corpus-gate.sh` hard-codes
  `/Volumes/Extreme Pro/…` paths and compares against a frozen fail-set
  (upstream's `wp109-harness/corpus-gate/*.fails`, git-ignored and never published). Ours is
  `baseline/gate.sh`.
- **Upstream's process rules assume tools we do not have.** `php-rust/CLAUDE.md` mandates the
  Serena and Vexp MCP servers and relies on a local hook that blocks reading `.rs` files from the
  shell; neither the servers nor the hook are part of the repository. It also says to commit
  *and push* after every step — upstream's remote is not ours to push to; this fork's push URL
  for `upstream` is disabled.
