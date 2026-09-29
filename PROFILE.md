# PROFILE.md — where phpr's time goes

PLAN.md §2. Measured 2026-09-30 on upstream `9d4ef5ba` (our commit `9552080b`, no source changes).
Raw numbers: [`bench/results/2026-09-30.md`](bench/results/2026-09-30.md),
[`bench/results/2026-09-30-compile-alloc-scaling.md`](bench/results/2026-09-30-compile-alloc-scaling.md).
Flamegraphs and folded stacks: [`bench/profiles/`](bench/profiles/).

## 0. What this profile can and cannot say

- **It does not explain "the 1.77×".** That figure is upstream's, for the WordPress PHPUnit suite
  on macOS against MySQL — a workload dominated by database I/O and builtins, which we did not
  run. What we measured, on CPU-bound PHP in Docker on Linux/arm64, is **3.5× to 15×** slower than
  `php -n`. Both can be true; they are different workloads. Nothing here contradicts upstream's
  own six-category micro table (2.1×–4.1× on macOS), but our figures are consistently higher and
  the environment difference (OS, libc, oracle build) is not separated from the engine.
- **Sampling, not counting.** `perf record -e cpu-clock --call-graph dwarf`, 2,999 Hz (9,999 Hz
  for `autoload`, 19,999 Hz for the 80 ms `symfony-boot`). The VM exposes no hardware PMU: no
  cycles, no cache or branch misses.
- **The profiled binary is the release recipe plus debug info** (`CARGO_PROFILE_RELEASE_DEBUG=1`,
  separate target dir, upstream's `Cargo.toml` untouched): fat LTO, 1 codegen unit.
- **Attribution is by inlined frame.** Almost everything hot is inlined into `run_loop`, so a
  sample is assigned using the DWARF inline chain. Line tables in optimised code are approximate;
  read every percentage as **±2 points**. Rules: [`bench/lib/buckets.py`](bench/lib/buckets.py).
- **One bucket per sample**, decided by the innermost meaningful frame — except *compile*, which
  is inclusive (anything under lowering/compiling/unit-linking), because that is the quantity a
  bytecode cache removes.
- **The plan's bucket list has no row for handler bodies**, which is most of an interpreter. They
  are reported as named "other" rows instead of being folded into "dispatch".

## 1. Benchmark results (medians of 5, engines interleaved)

Whole process, wall seconds. Every phpr checksum matched the oracle's.

| benchmark | phpr | `php -n` | php + opcache | phpr ÷ `php -n` | phpr ÷ opcache | peak RSS phpr / `php -n` |
|---|---:|---:|---:|---:|---:|---:|
| `Zend/bench.php` | 1.83 | 0.23 | 0.16 | **7.96×** | 11.44× | 77.6 / 28.4 MB |
| `Zend/micro_bench.php` ¹ | 9.52 | 1.09 | 0.81 | **8.73×** | 11.75× | 75.6 / 23.9 MB |
| `bench/arrays.php` | 2.98 | 0.62 | 0.61 | **4.81×** | 4.89× | 263.6 / 202.2 MB |
| `bench/strings.php` | 6.47 | 1.86 | 1.84 | **3.48×** | 3.52× | 350.9 / 315.4 MB |
| `bench/oop.php` | 5.90 | 0.60 | 0.59 | **9.83×** | 10.00× | 75.6 / 23.9 MB |
| `bench/autoload.php` (2,000 classes) | 0.61 | 0.04 | 0.07 | **15.25×** | 8.71× | 166.1 / 40.1 MB |
| `bench/symfony-boot.php` (200 requests) | 0.08 | 0.01 | 0.02 | **8.00×** ² | 4.00× ² | 91.8 / 25.0 MB |

¹ Unmodified `micro_bench.php` does not run under phpr (`Parse error: unsupported construct
(assignment target)` on `$x = isset(Foo::$a);`). Both engines ran a copy with that one statement
neutralised; its row is not measured.
² `/usr/bin/time` resolves 10 ms, so these two ratios are coarse. The in-script timers are
better: 200 requests take **51.8 ms under phpr and 6.0 ms under `php -n` (8.64×)**; boot is
10.7 ms vs 2.1 ms; the first request 7.4 ms vs 2.4 ms.

Best and worst sections (phpr ÷ `php -n`, in-script timers):

| slowest relative to the oracle | ratio | | closest to (or faster than) the oracle | ratio |
|---|---:|---|---|---:|
| `$s .= $i` ×200k (quadratic, §5.1) | 374.7× | | `str_replace` on 1.3 MB ×20 | **0.50×** |
| `simplecall` (Zend bench) ³ | 30.0× | | `var_export` ×2 | **0.60×** |
| property write `$o->v = $i` ×1M | 19.3× | | `serialize` ×5 | 1.19× |
| autoload + instantiate 2,000 classes | 18.1× | | `htmlspecialchars` + `md5` ×200k | 1.47× |
| property read-modify-write ×1M | 17.1× | | `json_decode` (assoc) 8.7 MB ×5 | 1.48× |
| method call ×1M | 16.3× | | `json_decode` (object) ×5 | 1.80× |
| `empty($this->x)` | 16.2× | | `sprintf` ×500k | 2.34× |
| `ary3(2000)` (Zend bench) | 15.4× | | `preg_replace` on 1.3 MB ×10 | 2.37× |
| packed array build, 1M | 13.9× | | `json_encode` 8.7 MB ×5 | 2.45× |
| `new Foo()` | 13.6× | | `sort()` 1M ints | 2.56× |

³ Zend bench prints 3 decimals; a 2 ms oracle row carries ±25 % rounding error.

**The shape is consistent: where the work is inside a builtin, phpr is within 1.2–2.5× of the
oracle and twice beats it. Where the work is bytecode — calls, property access, array element
access, loops — it is 5–19× slower.**

## 2. The bucket table

Percent of samples. `symfony-boot` is the benchmarked configuration (200 requests, ~80 ms, so
boot dominates); `symfony-steady` is the same script with 5,000 requests, to expose the
per-request path.

| bucket | `Zend/bench` | `arrays` | `oop` | `symfony-steady` | `symfony-boot` | `strings` ⁴ | `autoload` |
|---|---:|---:|---:|---:|---:|---:|---:|
| `Rc` inc/dec and drop glue | 8.6 | **19.6** | 9.4 | 9.4 | 4.6 | 3.9 | 1.3 |
| `RefCell` borrow checks | 0.3 | 0.3 | 4.7 | 4.4 | 2.2 | 1.6 | 0.2 |
| allocation / deallocation | 1.9 | 12.0 | 8.2 | 6.5 | 14.6 | 14.3 | 2.4 |
| hash table ops in `PhpArray` | 2.1 | 14.2 | 2.5 | 2.3 | 1.6 | 3.4 | 0.1 |
| string hash / compare / copy | 0.3 | 2.7 | 5.0 | 4.8 | 2.7 | **44.3** | 0.4 |
| VM dispatch loop itself | **54.5** | **28.4** | **34.2** | **30.9** | 15.1 | 5.6 | 1.6 |
| argument passing into builtins | 0.7 | 1.0 | 0.0 | 2.2 | 0.8 | 2.6 | 0.1 |
| builtin bodies | 0.1 | 1.1 | 0.0 | 3.1 | 2.0 | 15.4 | 0.1 |
| parser / HIR / compile | 1.9 | 0.6 | 1.8 | 4.1 | **36.1** | 0.4 | **86.9** |
| other: operator & type-juggling bodies | 17.1 | 5.3 | 3.5 | 1.7 | 1.1 | 2.3 | 0.1 |
| other: VM handler bodies (paths, OOP, calls) | 10.7 | 11.8 | 21.4 | 22.6 | 10.3 | 4.3 | 2.1 |
| other: GC bookkeeping + destructor sweep | 1.3 | 1.5 | 3.6 | 2.3 | 1.2 | 0.5 | 0.2 |
| other: engine-internal hash maps (symbol tables) | 0.4 | 1.4 | 5.7 | 5.3 | 2.7 | 1.1 | 0.6 |
| other: kernel (non-page-fault) | 0.0 | 0.1 | 0.0 | 0.1 | 2.6 | 0.1 | 3.4 |
| other: process startup, libc, std I/O | 0.3 | 0.1 | 0.1 | 0.3 | 2.4 | 0.2 | 0.7 |
| other: unclassified | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| *samples* | *5,814* | *8,911* | *18,121* | *3,959* | *2,747* | *19,047* | *6,781* |

⁴ 32.9 points of the `strings` "string copy" row are `memcpy` inside `PhpStr::concat2`, the
copy-the-whole-string fallback — the quadratic `$s .= $i` loop of §5.1, which by the in-script timer is
2.22 s of the run's 6.40 s of CPU (34.7 %).

### The quantity PLAN.md §3 asks for

`Rc` + `RefCell` + allocation, as a share of all samples:

| | `Zend/bench` | `arrays` | `oop` | `symfony-steady` | `strings` |
|---|---:|---:|---:|---:|---:|
| `Rc` + `RefCell` + alloc | **10.7 %** | **31.9 %** | **22.3 %** | **20.3 %** | **19.8 %** |
| PLAN threshold for option A | < 25 % | | | | |
| PLAN threshold for option B | ≥ 40 % | | | | |

This is an **upper bound** on what is "attributable to the value representation itself". A large
part of the allocation and refcount traffic is caused by how the VM *uses* values, and would
survive any change of representation unchanged:

- the per-array-write, per-call and per-`foreach` allocations of §4 are engine logic;
- `foreach`'s snapshot clones every element (§5.4);
- 42 `Zval` clones to iterate 8 elements, 9 clones per method call (§4).

No benchmark reaches 40 %. Four of five are under 25 %. `arrays` is in between, and its excess is
the snapshot-and-clone behaviour, not the size or layout of `Zval`.

## 3. Reading the table

1. **Dispatch is the largest single bucket on every CPU-bound benchmark (28–54 %).** Upstream
   measured "pure dispatch costs 1.75 ns/op" and concluded the gap is in handler bodies. Our
   attribution does not contradict the conclusion about *bodies*, but the loop's own bookkeeping
   is not small:
   - **Re-indexing the frame on every access.** Frames live in `Vec<Frame>`, each frame's operand
     stack and slots in their own `Vec<Zval>`; handlers write `self.frames[top].stack.pop()`.
     The inlined `Vec::len` / `as_slice` / `as_mut_slice` / `capacity` / slice-index helpers on
     those three vectors are, as leaf frames, **20.6 %** of `Zend/bench`, 12.3 % of `oop`, 10.6 %
     of `arrays`, 7.8 % of `symfony-steady`. That is bounds checks and pointer reloads.
   - **Moving frames by value** (`ptr::read/write::<Frame>`, `with_buffers`, frame drop glue):
     3.5 % of `Zend/bench`, 4.8 % of `oop`.
   - `size_of::<Op>()` is 48 bytes; a Zend opline is 32 on 64-bit.
2. **Handler bodies are the second largest: 15–28 %** (operators + VM bodies). The property/array
   *path* machinery (`path_apply`, `path_walk`, `path_op`) is 5.4 % of `Zend/bench` and 4.6 % of
   `arrays` on its own.
3. **Refcounting and drops are 8–10 % on general code, 20 % on array-heavy code.**
   `drop_glue::<Zval>` and `Zval::clone` are 6.9 % each in `arrays`.
4. **`RefCell` is a real but minor cost: 4–5 % on object-heavy code, ~0 elsewhere.**
5. **Allocation is 6–14 % outside pure arithmetic.** mimalloc itself is cheap; the count is the
   problem (§4).
6. **On a framework request, engine-internal lookups by *name* cost about as much as
   refcounting.** `symfony-steady`: symbol-table hash maps 5.3 % + `host_builtin_canonical`
   6.1 % (§5.3) + builtin argument plumbing 2.2 % = 13.6 %.
7. **Compile is 87 % of autoloading and 36 % of a short Symfony run.** See §6.

## 4. Allocations per iteration — said loudly, as PLAN asks

**Scalars do not allocate. The loop counter does not allocate.** Integer and float arithmetic in
a `for` loop performs **0** allocations per iteration, at global scope and inside a function.

**But the PLAN's reference loop — integer arithmetic plus one array write — allocates exactly
once per iteration**: 1,183,457 allocator calls at 1,000,000 iterations, 2,183,457 at 2,000,000.
The allocation belongs to the array element *write*, not to the arithmetic.

| operation | malloc + free per operation | `Zval` clones |
|---|---:|---:|
| integer / float arithmetic, empty loop | 0 | 0–1 |
| array element read | 0 | 2 |
| **array element write `$a[$k] = $v`** (int or string key, existing key, unshared array) | **1** | 3–4 |
| array append `$a[] = $v` | 0 | 3 |
| **user function call** `f($i)` with a typed parameter and return | **1** | 2 |
| builtin call `abs($i)` | 0 | 1 |
| method call `$o->inc()` | 0 | **9** |
| property write `$o->v = $i` | 0 | 4 |
| **`foreach` over an array** | **1 + 1 realloc per loop entry** | **42 for 8 elements (5.25 per element)** |
| `'id-' . $i` | 2 | 3 |

The Zend engine performs none of the three bolded allocations: it writes array elements in place,
runs calls on its VM stack, and iterates arrays by position. (Stated from Zend's design; the
oracle's allocations were not counted tonight.)

Method: upstream's own `mem-census` feature in a separate build; difference quotient between two
iteration counts; every figure is an exact integer.

## 5. Four algorithmic cliffs the profile exposed

These are not representation costs and not dispatch costs. Each is a specific piece of logic.

### 5.1 `$s .= <non-string>` is quadratic

Appending an integer or a float to a growing string copies the whole string every iteration.

| N | oracle | phpr | ratio |
|---:|---:|---:|---:|
| 50,000 | 1.04 ms | 127.35 ms | 122× |
| 100,000 | 1.92 ms | 486.35 ms | 253× |
| 200,000 | 4.06 ms | 2,091.17 ms | 515× |
| 400,000 | 9.00 ms | 8,930.59 ms | **992×** |

phpr's time grows 3.8–4.3× per doubling of N; the oracle's 1.9–2.2×. With an explicit cast
(`$s .= (string) $i`) phpr is linear and 2.2× the oracle. The fused `.=` op extends in place
only when the right-hand operand is already a string. Building output with `.= $number` is
ordinary PHP.

### 5.2 Including a file re-hashes the symbol tables — with SipHash

`autoload`: **28.7 % of all samples are SipHash** (`core::hash::sip`, std's default
`RandomState`), in `HashMap<Vec<u8>, usize, RandomState>` built under `lower_source_impl`
(26.5 % of samples), `compile_program_impl_mode` (9.0 %), `Vm::unit_fp` (6.5 %),
`seed_stub_mask` (2.9 %), `unit_remap_elided` (2.9 %).

Cost per line, phpr:

| | ms per 1,000 lines |
|---|---:|
| one 5,003-line file | 1.35 |
| one 20,020-line file | 1.48 |
| 2,002 files of 53 lines (106,018 lines) via the autoloader | **5.4** |

Small files cost 3.6–4× more per line than a large one: each include pays a fixed charge that
scales with the number of symbols *already loaded*, not with the file. The rest of the engine
uses FxHash; these maps use the DoS-hardened default.

### 5.3 `host_builtin_canonical` scans a name list on the request path

`symfony-steady`: **6.1 %** of samples are inside `php_runtime::vm::host_builtin_canonical`
(`vm/mod.rs:14856`), whose leaf frames are a slice iterator over `&[u8]` names (5.2 %) and
`is_ascii_uppercase` (1.2 %) — a linear, case-folding scan of the host-builtin name table,
executed at run time.

### 5.4 Builtins are looked up by name on every call; `foreach` copies the array

- `Op::CallBuiltin` hashes the function name and probes the registry map on each execution
  (`vm/run.rs:3872`), then walks a chain of name comparisons in `value_builtin_call`.
  Builtin argument plumbing is 2.2–2.6 % on `symfony-steady` and `strings`.
- `foreach` by value snapshots every `(key, value)` pair into a new `Vec<(Zval, Zval)>` before the
  first iteration (`vm/arrays.rs:971`): 32 bytes and up to two refcount increments per element.
  Measured: `packed_foreach_sum_1m` 7.97×, `assoc_foreach_200k_x5` 10.22× the oracle, against
  3.86× for the same array's keyed *lookups*.

## 6. Compile cost, and what a bytecode cache would buy

Median of 30 runs. Compile = script − empty script, same engine.

| | phpr | `php -n` | php + opcache, warm file cache |
|---|---:|---:|---:|
| startup floor (empty script) | **15.82 ms** | 6.29 ms | 6.23 ms |
| startup peak RSS | **68.7 MB** | 23.5 MB | 25.4 MB |
| compile 5,003 lines | **6.77 ms** | 2.21 ms | 0.55 ms |
| compile 20,020 lines | **29.72 ms** | 8.54 ms | 2.68 ms |
| memory retained per 1,000 lines | **4.1–4.4 MB** | 0.3–0.4 MB | — |

- **Ceiling of a bytecode cache for phpr, single file:** 1.35–1.5 ms per 1,000 lines. For
  comparison the oracle's opcache saves it 0.3 ms per 1,000 lines — phpr has more to gain
  because it compiles 3.1–3.5× slower.
- **Through the autoloader the ceiling is larger:** 87 % of 576 ms ≈ 500 ms for 2,000 classes,
  ≈ 0.25 ms per class. But a third of that is the SipHash symbol-table work of §5.2, which a
  cache keyed per file would *not* remove unless unit linking is also made incremental — and
  which can be removed without any cache.
- **A cache does not touch the 15.8 ms startup floor** unless the prelude is cached too. The floor
  is 9.5 ms above the oracle's and is paid by every CLI process and, because `php-server` builds a
  fresh `Vm` per request, potentially by every request.
- **`symfony-boot`:** 36 % of an 80 ms run is compile ≈ 29 ms, for 274 classes loaded by the
  oracle (199 reported by phpr — `get_declared_classes()` differs, not investigated).

## 7. What the profile says, in one paragraph

phpr is not slow because of how a value is laid out. `Zval` is already 16 bytes with inline
scalars, and scalars do not allocate. The time is in the interpreter around the values: a
dispatch loop that re-derives its frame pointer on every access (up to 21 % of samples on its
own), handler bodies that clone and drop values more often than the operation needs (9 clones per
method call, 42 per 8-element `foreach`), one heap allocation on each array write, each function
call and each `foreach`, and name-based lookups on the hot path. On top of that sit two outright
cliffs — a quadratic string append and a per-include symbol-table rebuild under SipHash — and a
compile pipeline three times slower than the oracle's that runs again in every process. Where
execution is inside a builtin, phpr is already within 1.2–2.5× of PHP.
