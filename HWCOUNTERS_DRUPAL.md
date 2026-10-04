# HWCOUNTERS_DRUPAL.md — instructions and cache misses of one Drupal request, ferro vs php

Question (owner, 2026-10-04): the Docker VM exposes no PMU, so use cachegrind instead of
hardware counters. Is ferro's Drupal gap **instruction bloat or memory stalls**, and which
handlers execute the most instructions in excess of Zend?

**Answer: instruction bloat.**
- Ferro executes **4.67× the instructions** of php (181.2M vs 38.8M per warm front-page request).
- Its wall time is **3.99×** php's on the same setup, so ferro runs at a slightly *higher* IPC
  (≈ 2.6 vs ≈ 2.2 at the M2 Max's ~3.5 GHz) than Zend.
- Its miss and mispredict rates per instruction are about the same as php's. The exception is
  LL misses in the 16 MB model, and the measured IPC shows those cannot be costing much on the
  real chip.
- Removing every memory stall would not close the gap. Executing php's instruction count at
  ferro's own CPI would put ferro at ≈ 4.3 ms against php's 5.05 ms.

The top five handlers by excess instructions vs Zend are listed in §5:

| # | handler family | excess per request |
|---|---|---:|
| 1 | object property access | +19.3M |
| 2 | user call + return | +13.0M |
| 3 | include / per-request linking | +10.1M |
| 4 | arrays | +8.5M |
| 5 | operand load/store ops | +6.7M |

In addition, the dispatch loop itself costs +10.6M (29 instructions per op).

## 1. Method

All of this runs in the dev image (`docker/Dockerfile` gained `valgrind` 3.24 plus the `-dev`
headers below). Nothing runs on the host.

| | ferro | php |
|---|---|---|
| server | `ferro -S`, classic (a fresh Vm per request) | `php -S` (cli-server SAPI of the CLI binary) |
| config | release build | `-n`, `opcache.enable=1 enable_cli=1 jit=off validate_timestamps=0`, file cache warm |
| site | fresh copy of `/scratch/drupal-base` | fresh copy of `/scratch/drupal-base` |
| binary for attribution | `/target/cg/release/ferro`: the release profile + `debug=line-tables-only` | `/scratch/php-dbg/bin/php`: the official tarball (`/usr/src/php.tar.xz`) rebuilt with the docker-library CFLAGS/LDFLAGS + `-g` (`bench/drupal/php-dbg.sh`); gd/sodium static |
| twin check (Ir/request) | 181.22M (line tables) vs **181.37M** (`/target/release/ferro`) | 38.80M (`-g`) vs **38.82M** (official stripped binary) |

The twins count the same instructions as the shipped binaries to within 0.1 %, so all
attribution below uses the twins.

- **Per-request numbers by subtraction** (`bench/drupal/cachegrind.sh`):
  1. 5 native warm-up requests (Drupal caches and the opcache file cache).
  2. Run A: one cachegrind run of the server serving 1 + 3 requests.
  3. Run B: the same plus **20 measured requests**.
  4. Per request = (B − A) / 20, which removes start-up, warm-up and shutdown exactly.
- **Cache model** = the host's Apple M2 Max P-core: I1 192 KB 6-way, D1 128 KB 8-way, LL 16 MB
  16-way (its L2), 128-byte lines. Branch simulation is on.
  - Cachegrind has no prefetcher and no model of the 48 MB SLC behind the L2.
  - Its indirect-branch predictor is a plain last-target BTB, which penalises a single shared
    dispatch jump (ferro's `match op`) far more than real hardware does.
- **Per handler.** A callgrind pass (`TOOL=callgrind`, `--dump-instr=yes`, 10 measured requests,
  same subtraction) records instructions per instruction address. `bench/drupal/cg-handlers.py`
  resolves every address of `run_loop` / `execute_ex` through its DWARF inline chain (gimli
  `addr2line -i`) to the outermost frame in `vm/run.rs` / `zend_vm_execute.h`:
  - ferro: that line is charged to its `Op::X` arm;
  - php: it is charged to the handler function, helper or `HYBRID_CASE` label holding it. With
    the hybrid VM, handlers are labels inside `execute_ex` and the `RETURN` tail is
    `zend_leave_helper_SPEC_LABEL`. Handlers compiled out of line are added under their own
    name.
- **Comparable pieces.** `bench/drupal/cg-buckets.py` maps every handler region and every other
  function, by an ordered rule table kept in the script, to one subsystem: calls, property
  access, builtin calls, hash lookup, allocator, and so on.
- **Exclusive costs only.** Callgrind's inclusive costs are unusable here:
  - the hybrid VM's jumps between handlers produce call edges larger than the whole run;
  - so do `run_loop`'s recursive re-entries (`run_loop'2 → run_loop'2`).

  So out-of-line callees (allocator, `drop_glue`, `bcmp`, …) stay in their own bucket and are
  not charged to the handler that called them. A handler family's excess in §5 is therefore a
  lower bound.

## 2. Counters per request

| event | ferro | php | ferro / php | ferro per 1k Ir | php per 1k Ir |
|---|---:|---:|---:|---:|---:|
| instructions (Ir) | 181,223,817 | 38,799,286 | **4.67×** | — | — |
| data reads (Dr) | 52,430,551 | 11,545,279 | 4.54× | 289 | 298 |
| data writes (Dw) | 28,228,947 | 5,682,041 | 4.97× | 156 | 146 |
| I1 misses | 496,525 | 87,552 | 5.67× | 2.74 | 2.26 |
| D1 read misses | 993,921 | 180,626 | 5.50× | 5.48 | 4.66 |
| D1 write misses | 193,381 | 47,321 | 4.09× | 1.07 | 1.22 |
| LL read misses (data) | 118,395 | 149 | 795× | 0.65 | 0.004 |
| LL write misses | 26,841 | 29 | 926× | 0.15 | 0.001 |
| LL instruction misses | 4,537 | 15 | 302× | 0.03 | 0.000 |
| conditional branches (Bc) | 25,518,607 | 5,423,848 | 4.70× | 141 | 140 |
| conditional mispredicts (Bcm) | 2,088,735 | 530,466 | 3.94× | 11.5 | 13.7 |
| indirect branches (Bi) | 1,305,309 | 633,153 | 2.06× | 7.2 | 16.3 |
| indirect mispredicts (Bim) | 608,887 | 205,589 | 2.96× | 3.4 | 5.3 |

Ratios:
- instructions **4.67×**;
- L1D misses (read+write) **5.21×**;
- L1I misses **5.67×**;
- LL misses ≈ **780×** (149.8k vs 0.19k);
- all mispredicts **3.66×**.

Wall time, native, same two servers on the same warm sites, 8 interleaved rounds × 25 requests
(`bench/drupal/cg-walltime.sh`, medians, two runs):

| run | ferro -S | php -S + opcache | ratio |
|---|---:|---:|---:|
| 1 | 20.33 ms | 5.06 ms | 4.01× |
| 2 | 20.01 ms | 5.04 ms | 3.97× |

## 3. Verdict: instruction bloat, not memory stalls

1. **Time ratio (3.99×) < instruction ratio (4.67×).** Ferro retires ≈ 9.0 G instructions/s,
   php ≈ 7.7 G/s, so ferro's CPI is ≈ 0.85× php's.
   - If ferro's extra time were memory stalls, its CPI would be higher than php's, not lower.
   - At php's CPI, ferro's 181M instructions would take ≈ 23.6 ms. At its own CPI, php's 38.8M
     would take ≈ 4.3 ms.
2. **Per-instruction miss and mispredict rates are php-like:**

   | per 1k instructions | ferro | php |
   |---|---:|---:|
   | L1D misses | 6.6 | 5.9 |
   | L1I misses | 2.7 | 2.3 |
   | mispredicts | 14.9 | 19.0 |

   Ferro misses ~5× more because it executes ~5× more instructions over ~5× more data
   (Dr 4.5×, Dw 5.0×), not because its accesses are worse.
3. **The one memory item php does not have is LL (16 MB) misses**, ≈ 150k per request.
   - At a DRAM latency of ~100 ns that would be ~15 ms, three quarters of the request, which
     the measured IPC rules out.
   - On the real chip most of them are served by the 48 MB SLC or caught by prefetchers
     (sequential `memcpy`/`bcmp` streams).
   - They are a secondary item: ferro's working set per request is larger than 16 MB, while
     php's fits in it. Where they come from is in §4.
4. **Branch mispredicts track instructions (3.7×), not stalls.** Cachegrind's indirect model
   overstates ferro's (one shared `match op` jump vs Zend's per-handler dispatch).

`handle()` this session (phase bench, N=180, ROUNDS=6, engine unchanged since 0c696eaa):

| engine | bootstrap | handle | total |
|---|---:|---:|---:|
| php-fpm | 1.12 ms | 3.79 ms | 5.11 ms |
| ferro | 2.19 ms | **17.67 ms** | 20.17 ms |
| php -S (no opcache) | 1.11 ms | 3.75 ms | 5.23 ms |

Therefore, to approach 2× php the instruction count has to come down. Making ferro's existing
instructions cheaper (layout, prefetching, cache footprint) can at best recover a fraction of
the LL item.

## 4. Per function (cachegrind, per request)

Top 20 functions by instructions. The ferro `run_loop` row includes every handler inlined into
it; §5 splits it.

**ferro**

| # | function | Ir | D1mr | D1mw | DLmr | Bcm | Bim | % Ir |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| | **total** | 181,223,817 | 993,921 | 193,381 | 118,395 | 2,088,735 | 608,887 | 100 |
| 1 | `Vm::run_loop` | 37,134,959 | 168,783 | 42,288 | 19,304 | 257,237 | 394,110 | 20.5 |
| 2 | `Vm::run_linked` | 6,324,932 | 158,021 | 7,106 | 23,949 | 24,139 | 0 | 3.5 |
| 3 | `mi_free` | 5,709,443 | 8,547 | 11,401 | 76 | 15,563 | 0 | 3.2 |
| 4 | `HashMap<Box<[u8]>, PropInfo>::get` | 5,244,144 | 20,305 | 809 | 2,541 | 39,946 | 0 | 2.9 |
| 5 | `bcmp` | 5,218,039 | 27,678 | 0 | 4,718 | 58,314 | 0 | 2.9 |
| 6 | `drop_glue::<Zval>` | 4,500,696 | 6,937 | 0 | 339 | 226,334 | 0 | 2.5 |
| 7 | `PropsLayout::slot_of` | 4,456,315 | 8,413 | 94 | 554 | 51,140 | 0 | 2.5 |
| 8 | `mi_theap_malloc_aligned` | 4,096,511 | 3,779 | 0 | 0 | 15,488 | 0 | 2.3 |
| 9 | `<Zval as Clone>::clone` | 3,634,984 | 17,195 | 4,162 | 347 | 9,555 | 135,213 | 2.0 |
| 10 | `memcpy` | 3,062,726 | 51,878 | 55,638 | 20,421 | 139,317 | 0 | 1.7 |
| 11 | `KeyIndex::lookup` | 2,725,614 | 17,614 | 65 | 104 | 64,060 | 0 | 1.5 |
| 12 | `_mi_page_malloc_zero` | 2,654,119 | 163,523 | 0 | 8,821 | 2,714 | 0 | 1.5 |
| 13 | `Vm::us_value` | 2,648,224 | 962 | 2,333 | 0 | 21,698 | 14,359 | 1.5 |
| 14 | `Vm::call_ns_fallback_site` | 2,514,328 | 4,242 | 95 | 85 | 41,637 | 5,610 | 1.4 |
| 15 | `Vm::gc_note_slow` | 2,159,691 | 10,751 | 104 | 44 | 37,973 | 0 | 1.2 |
| 16 | `oop::resolve_method_runtime` | 1,910,515 | 26,198 | 311 | 3,840 | 32,916 | 0 | 1.1 |
| 17 | `oop::resolve_prop_access` | 1,748,250 | 10,269 | 18 | 1,302 | 15,908 | 0 | 1.0 |
| 18 | `???` | 1,703,899 | 2,094 | 0 | 9 | 0 | 1,239 | 0.9 |
| 19 | `Vm::field_isset_op` | 1,661,595 | 8 | 98 | 0 | 13,809 | 0 | 0.9 |
| 20 | `Vm::us_key` | 1,564,872 | 1,432 | 189 | 0 | 12,667 | 0 | 0.9 |

**php**

| # | function | Ir | D1mr | D1mw | DLmr | Bcm | Bim | % Ir |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| | **total** | 38,799,286 | 180,626 | 47,321 | 149 | 530,466 | 205,589 | 100 |
| 1 | `php_var_unserialize_internal` | 4,687,621 | 5,370 | 928 | 0 | 26,956 | 18,674 | 12.1 |
| 2 | `execute_ex` | 4,600,294 | 40,140 | 1,499 | 21 | 99,003 | 121,909 | 11.9 |
| 3 | `???` (system libsqlite3, stripped) | 2,318,926 | 7,142 | 1,047 | 5 | 19,440 | 32,688 | 6.0 |
| 4 | `accel_init_interned_string_for_php` | 1,308,391 | 5,086 | 88 | 9 | 20,299 | 0 | 3.4 |
| 5 | `sqlite3Parser` | 984,497 | 2,416 | 66 | 0 | 10,883 | 6,466 | 2.5 |
| 6 | `_emalloc` | 955,804 | 6,672 | 0 | 0 | 7,079 | 0 | 2.5 |
| 7 | `_efree` | 802,527 | 998 | 3,798 | 0 | 4,319 | 0 | 2.1 |
| 8 | `memcpy` | 753,995 | 9,382 | 11,271 | 46 | 28,750 | 0 | 1.9 |
| 9 | `zend_hash_find` | 605,691 | 13,743 | 64 | 7 | 14,492 | 0 | 1.6 |
| 10 | `zend_hash_func` | 598,221 | 56 | 0 | 0 | 10,491 | 0 | 1.5 |
| 11 | `zend_array_destroy` | 487,852 | 6,585 | 3 | 0 | 15,234 | 0 | 1.3 |
| 12 | `zend_hash_lookup` | 482,327 | 567 | 1,981 | 0 | 4,912 | 0 | 1.2 |
| 13 | `sqlite3GetToken` | 459,733 | 135 | 0 | 0 | 4,056 | 10,404 | 1.2 |
| 14 | `bcmp` | 448,906 | 1,198 | 0 | 0 | 10,811 | 0 | 1.2 |
| 15 | `ZEND_ISSET_ISEMPTY_DIM_OBJ_SPEC_TMPVAR_CV_HANDLER` | 429,040 | 377 | 0 | 0 | 5,141 | 0 | 1.1 |
| 16 | `pthread_mutex_lock` | 410,236 | 236 | 10 | 0 | 683 | 0 | 1.1 |
| 17 | `parse_iv2` | 394,638 | 24 | 28 | 0 | 4,266 | 0 | 1.0 |
| 18 | `_int_malloc` | 364,682 | 754 | 997 | 0 | 8,655 | 0 | 0.9 |
| 19 | `__pthread_mutex_unlock_usercnt` | 362,396 | 2 | 0 | 0 | 1,165 | 0 | 0.9 |
| 20 | `free` | 296,174 | 237 | 2 | 0 | 632 | 0 | 0.8 |

Shape:
- php's profile is headed by real work: unserialize of cache rows, SQLite, opcache string
  interning, the VM.
- Ferro's is headed by machinery:

  | item | Ir per request |
  |---|---:|
  | the dispatch loop | 37M |
  | per-request unit linking (`run_linked`) | 6.3M |
  | allocator (`mi_*`) | 18.8M |
  | property-name lookups (`PropInfo` map + `slot_of`) | 9.7M |
  | byte compares (`bcmp`) | 5.2M |
  | out-of-line `Zval` drop/clone | 8.1M |

Calls per request (callgrind call counts):

| function | ferro | php equivalent |
|---|---:|---|
| `drop_glue::<Zval>` / `rc_dtor_func` | 495.8k | 17.2k (Zend's refcount decrement is an inline macro; only the free at zero is a call) |
| `bcmp` | 353.8k | 33.7k |
| `Zval::clone` | 271.7k | inline `Z_TRY_ADDREF` |
| allocations (`mi_malloc_aligned` / `_emalloc` + `_emalloc_N` + glibc `malloc`) | 205.9k | ≈ 55k visible |
| `PropsLayout::slot_of` | 158.8k | — |
| `HashMap<_, PropInfo>::get` | 114.9k | — |
| `gc_note_slow` | 108.3k | `gc_possible_root` 5.9k |
| `KeyIndex::lookup` / `zend_hash_find`+`_lookup`+`_find_known_hash` | 95.9k | 47.8k |
| `unit_slot_pos` (name scan when a unit is included from inside a function) | 31.2k | — |

Where ferro's L1D and LL misses are, per request:

| site | D1mr | LL misses | what it is |
|---|---:|---:|---|
| `_mi_page_malloc_zero` | 163.5k | 8.8k | 0.8 L1D misses per allocation: mimalloc hands out cold blocks |
| `run_linked` | 158.0k | 24.0k | ~110k D1mr at `vm/mod.rs:6699`: the linear `saved.functions.iter().find(name_eq_ignore_case)` redeclare scan per linked function; 37k D1mr / 19.6k LL in `Rc` refcount bumps on cold unit data (`rc.rs:2026`) |
| `run_loop` op fetch (`match op`, `run.rs:1662`) | 92.8k | 11.8k | `Op` is 48 bytes (`zend_op` 32); ~365k dispatches |
| `memcpy` | 107.5k (r+w) | 37.6k (r+w) | string and buffer copies |
| `bcmp` | 27.7k | 4.7k | name compares |

## 5. Comparable pieces and the handlers with the most excess instructions

Instructions per request by subsystem (callgrind, exclusive costs, the interpreter split by
handler). The rules live in `bench/drupal/cg-buckets.py`; "% of the gap" is of
ferro − php = 143.5M.

| piece | ferro Ir | php Ir | ferro / php | excess | % of the gap |
|---|---:|---:|---:|---:|---:|
| property access (fetch/assign/isset on objects) | 20,487,165 | 1,151,287 | 17.8× | 19,335,878 | 13.5 |
| memory allocator | 18,820,736 | 3,919,926 | 4.8× | 14,900,810 | 10.4 |
| user calls + return (frame setup, args, params, leave) | 16,061,742 | 3,091,787 | 5.2× | 12,969,955 | 9.0 |
| value copy/drop, refcounting, object ids | 12,900,445 | 427,797 | 30.2× | 12,472,648 | 8.7 |
| strings + mem primitives (compare, copy, search, concat, convert) | 14,523,995 | 2,431,088 | 6.0× | 12,092,907 | 8.4 |
| dispatch (fetch next op, frame/ip bookkeeping) | 10,635,490 | (in handlers) | — | 10,635,490 | 7.4 |
| include, autoload, class linking per request | 13,236,856 | 3,092,305 | 4.3× | 10,144,551 | 7.1 |
| arrays (dim fetch/assign, iteration, insert, copy, destroy) | 11,861,930 | 3,329,555 | 3.6× | 8,532,375 | 5.9 |
| hash lookup (arrays, symbol/class/function/constant tables) | 10,188,476 | 2,470,255 | 4.1× | 7,718,221 | 5.4 |
| operand loads/stores (no Zend equivalent) | 6,772,740 | 43,896 | 154× | 6,728,844 | 4.7 |
| other (long tail) | 8,000,922 | 1,311,801 | 6.1× | 6,689,121 | 4.7 |
| builtin calls (resolution, arg passing; bodies excluded) | 5,254,463 | 538,237 | 9.8× | 4,716,226 | 3.3 |
| cycle GC bookkeeping | 4,932,269 | 218,885 | 22.5× | 4,713,384 | 3.3 |
| compare/arith/branch ops | 4,496,355 | 546,512 | 8.2× | 3,949,843 | 2.8 |
| builtin bodies (string, regex, hash, output …) | 4,974,981 | 1,847,598 | 2.7× | 3,127,383 | 2.2 |
| interpreter, unattributed (inlined code with no run.rs / zend_vm_execute.h frame) | 2,914,694 | 63,658 | — | 2,851,036 | 2.0 |
| isset/empty on paths (`$this->a[$k]`, `$a[$k][…]`) | 3,180,918 | 1,198,696 | 2.7× | 1,982,222 | 1.4 |
| unserialize (cache rows) | 6,538,545 | 4,846,539 | 1.3× | 1,692,006 | 1.2 |
| other interpreter ops (assign, misc) | 660,307 | 537,069 | 1.2× | 123,238 | 0.1 |
| SQLite (bundled 3.x in ferro vs the system library) | 5,781,223 | 7,679,894 | 0.8× | −1,898,671 | −1.3 |
| **total** | **182,224,252** | **38,746,778** | **4.70×** | **143,477,474** | 100 |

The pieces the owner named, side by side:

| piece | ferro | php | per execution |
|---|---|---|---|
| method call + return | `Frame::with_buffers` 2.17M, `Op::Ret` 2.15M, `resolve_method_runtime` 1.86M, `recycle_frame` 1.67M, `methodcall_fast` 1.14M, `coerce_param_hints` 0.86M, `enter_callee` 0.81M, `drop_glue::<Frame>` 0.72M … = 16.1M | `zend_leave_helper` 0.62M, `INIT_METHOD_CALL_*` 0.39M, `zend_call_function` 0.21M, `object_init_ex` 0.16M, `DO_UCALL` 0.14M, recv type checks 0.12M … = 3.1M | ≈ 940 vs ≈ 180 Ir per call (17.0k returns per request on both) |
| return alone | `Op::Ret` 126 Ir/exec + `recycle_frame` + frame drop | `zend_leave_helper` 17 Ir/exec + `RETURN` handlers | |
| property fetch | `PropInfo` map get 6.56M + `slot_of` 5.59M + `resolve_prop_access` 1.35M + `magic_applies_resolved` 0.95M + `lazy_prop_access` 0.87M + `deref_object` 0.74M + `ThisPropGet` arm 0.60M … = 20.5M | `FETCH_OBJ_R/W` 0.35M + `ASSIGN_OBJ` 0.15M + `zend_std_write/read/has_property` 0.30M + `rebuild_object_properties` 0.10M … = 1.15M | 274k name lookups (159k `slot_of`, 115k `PropInfo`) for ≈ 17k property ops ≈ 16 per op; Zend's monomorphic sites read the cached (class, offset) from the op's runtime cache and hash nothing |
| builtin call | `call_ns_fallback_site` 2.19M + `ns_site_target` 1.28M + string coercion, `Builtin` map, arg decay … = 5.25M | `FRAMELESS_ICALL_*` 0.23M, `DO_FCALL_BY_NAME` 0.11M, `INIT_NS_FCALL_BY_NAME` 0.07M, `TYPE_CHECK` … = 0.54M | ≈ 530 vs ≈ 55 Ir per call (9.9k per request); php resolves the namespaced fallback once into the runtime cache and many sites are frameless |
| hash lookup | `KeyIndex::lookup` 3.45M, `SipHasher13::write` 1.57M (std `HashMap`s on the request path), `ci_hash` 1.17M, other `HashMap::get` 1.14M … = 10.2M | `zend_hash_find` 0.74M, `zend_hash_func` 0.60M, `zend_hash_lookup` 0.49M, `index_lookup` 0.24M, `find_known_hash` 0.22M … = 2.47M | Zend hashes a string once (cached in `zend_string`), and opcache interns names so most compares are pointer compares |

### Top 5 handlers by excess instructions vs Zend

Each row is a handler family: the interpreter self cost of the ferro `Op::X` arms vs the Zend
handlers that do the same job, plus the out-of-line helpers dedicated to them. Shared runtime
costs they trigger (allocator, `Zval` drop/clone, `bcmp`, hash lookup) are not included, so
these are lower bounds.

1. **Object property access: `ThisPropGet`, `PropSetPop`, `PropGetSlot`, `FieldAssign`,
   `PropIsset`, vs `FETCH_OBJ_*`, `ASSIGN_OBJ*`, `ISSET_ISEMPTY_PROP_OBJ`. +19.3M (17.8×).**
   - Every access resolves the property by name: an FxHashMap `PropInfo` lookup for
     visibility and hooks, then a `PropsLayout::slot_of` scan, with `bcmp` on the bytes.
   - This happens ≈ 16 times per property op.
   - Zend's runtime-cache slot makes a monomorphic hit a pointer compare plus an offset load.
2. **User call + return: `MethodCall*`, `Ret`, `CoerceParams`, vs `INIT_METHOD_CALL`,
   `DO_UCALL`, `RECV*`, `RETURN` + `zend_leave_helper`. +13.0M (5.2×).**
   - ≈ 940 vs ≈ 180 instructions per call: frame buffers, method resolution at run time
     (`resolve_method_runtime` 27.9k calls), param coercion, frame recycle and drop.
3. **Include and per-request linking: `Include` → `run_linked`, `DeclareDeferred`, autoload, vs
   `INCLUDE_OR_EVAL` + opcache `persistent_compile_file`. +10.1M (4.3×).**
   - ≈ 17.7k instructions per include (747 per request) vs ≈ 4.1k.
   - Contributors:
     - linear name scans for redeclare checks (`vm/mod.rs:6676`/`:6699`: `:6699` is the
       request's second-largest L1D-miss line, 110.6k, after mimalloc's `alloc.c:49`);
     - `Rc` bumps on cold unit data;
     - `unit_slot_pos` (31k name scans for includes from inside functions);
     - user-wrapper/stream checks.
4. **Arrays: `FetchDim`, `AssignPath`, `IterNext`/`IterInit`, `ArrayInit`/`Push`, path keys, vs
   `FETCH_DIM_*`, `ASSIGN_DIM*`, `FE_*`, `INIT_ARRAY`/`ADD_ARRAY_ELEMENT`. +8.5M (3.6×).**
   - Top items:

     | item | Ir per request |
     |---|---:|
     | `PhpArray::insert` | 1.52M |
     | `pop_field_keys` (keys collected into a `Vec` per path op) | 1.38M |
     | array drop | 0.90M |
     | `IterNext` arm | 0.84M |
     | `coerce_key_silent` | 0.70M |
5. **Operand load/store ops: `LoadVar`, `PushConst`, `StoreSlot`, `LoadVarPushConst`, `Pop`,
   `This`, `Dup`. +6.7M.**
   - Zend has no such handlers: CV and CONST operands are embedded in `zend_op` (see
     ZEND_VM_NOTES.md).
   - These ops are only the ferro side, ≈ 150k executions at 40–60 instructions each,
     dispatch excluded.

Not a handler but larger than four of them:
- **The dispatch loop itself, +10.6M: 29 instructions per op over ≈ 365k dispatches.** Per op,
  `run_loop` does all of this:
  - checks `self.frames.len() > MAX_CALL_DEPTH`;
  - recomputes `top`;
  - reads `frames[top].ip` and `.func` with bounds checks;
  - bounds-checks `func.ops[ip]`;
  - writes `ip + 1` back to the frame (`run.rs:1606–1660`).

  Zend's `ZEND_VM_NEXT_OPCODE` is `opline++` plus an indirect jump, and is counted inside each
  handler on the php side.
- **Runtime-wide excess, charged to no handler here:**

  | item | excess Ir | detail |
  |---|---:|---|
  | allocator | +14.9M | 206k allocations vs ≈ 55k; ≈ 40 instructions per `mi_malloc_aligned` plus `_mi_page_malloc_zero`'s cold-block misses |
  | out-of-line `Zval` drop/clone | +12.5M | Zend inlines both |
  | strings/mem | +12.1M | 354k `bcmp` calls, mostly name compares that interned-pointer equality avoids in Zend |
  | GC bookkeeping | +4.7M | `gc_note_slow` 108k calls, `Op::Sweep` 1.0M |

## 6. Caveats

- **Cachegrind is a model.** It has no prefetch and no SLC, its branch predictors are simple,
  and it uses a fixed 16 MB LL. The instruction counts are exact; the miss counts are
  indicative; the verdict rests on instructions vs measured wall time, not on the miss model.
- **php side is `php -S`, not php-fpm**, with the same opcache settings and no JIT, which is
  what `opcache.enable_cli=1` gives a CLI binary.
  - php-fpm's `handle()` ratio (phase bench) is in the same range: 4.6–4.8×.
  - The whole-request ratio here (3.99×) includes bootstrap, send and terminate.
- **SQLite differs:** ferro bundles its own, php links the system `libsqlite3`. That is the only
  piece where ferro executes fewer instructions.
- **The bucket rules are a judgement.** They are a reviewable table in `cg-buckets.py`; moving
  a borderline function changes a row by at most a few hundred thousand instructions, not the
  ranking.
- **Execution counts are approximate.** "≈ execs" are the hottest instruction's count, and
  per-call figures divide by the op census counts (`bench/results/2026-10-04-operand-census.md`).

## 7. Reproduce

```bash
docker build -t rustyphp-dev:8.5.7 -f docker/Dockerfile docker           # valgrind layer
docker/run.sh bash /work/php-rust/bench/drupal/php-dbg.sh                 # -g oracle twin, once
docker/run.sh bash -c 'CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_TARGET_DIR=/target/cg \
  cargo build --release -p php-cli'
docker/run.sh bash -c 'ENGINE=ferro TAG=ferro PORT=8261 M=20 bash /work/php-rust/bench/drupal/cachegrind.sh'
docker/run.sh bash -c 'ENGINE=php   TAG=php   PORT=8262 M=20 bash /work/php-rust/bench/drupal/cachegrind.sh'
docker/run.sh bash -c 'TOOL=callgrind ENGINE=ferro TAG=ferro-cl PORT=8271 M=10 bash /work/php-rust/bench/drupal/cachegrind.sh'
docker/run.sh bash -c 'TOOL=callgrind ENGINE=php   TAG=php-cl   PORT=8272 M=10 bash /work/php-rust/bench/drupal/cachegrind.sh'
docker/run.sh bash -c 'cd /scratch/cg && S=/work/php-rust/bench/drupal
  python3 $S/cg-funcs.py ferro-0.out ferro-20.out 20 --by fn --top 20      # §4 (also --by file|fnfile)
  python3 $S/cg-handlers.py ferro-cl-0.out ferro-cl-10.out 10 /target/cg/release/ferro --engine ferro \
    --src /work/php-rust/php-rust/crates/php-runtime/src/vm/run.rs --tsv ferro-handlers.tsv
  python3 $S/cg-handlers.py php-cl-0.out php-cl-10.out 10 /scratch/php-dbg/bin/php --engine php \
    --src /scratch/php-dbg-src/Zend/zend_vm_execute.h --tsv php-handlers.tsv
  python3 $S/cg-buckets.py ferro-handlers.tsv php-handlers.tsv --members 8 \
    --cg ferro-0.out ferro-20.out php-0.out php-20.out 20'                 # §5
docker/run.sh bash /work/php-rust/bench/drupal/cg-walltime.sh             # §2 wall time
```

Ferro under cachegrind takes ≈ 5 min for the pair of runs; php ≈ 2 min; the callgrind passes
≈ 2 min each. The arms can run in parallel (separate ports and sites; counts do not depend on
load).
