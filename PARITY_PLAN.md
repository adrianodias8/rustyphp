# PARITY_PLAN.md — instructions per request to php parity

Owner's plan, 2026-10-05. Metric and protocol: `bench/results/ir-trend.md` (cachegrind Ir per
warm Drupal request, `SIM=0 bench/drupal/cachegrind.sh`; secondary interleaved `handle()` ms).
Every step records both. Zend references are to php-src `php-8.5.7` as cited in
`ZEND_VM_NOTES.md` (§ numbers below); a reference marked **gap** is not in the notes yet and is
read and added to them before the item starts.

## Parity

- **Instruction parity: 38.8M Ir/request** (php 8.5.7 + opcache, same harness).
- **Time parity: ~40–45M.** Ferro's CPI is ≈ 0.85× php's (HWCOUNTERS_DRUPAL.md §3), so a
  little more than php's count runs in php's time.
- **Now: 151.8M (3.91×).** Excess over php by subsystem (callgrind, `cg-report.sh s15end`,
  153.3M): calls+return 12.3M, allocator 11.1M, dispatch 10.6M, value copy/drop 10.3M, other
  9.4M, strings/mem 8.5M, arrays 8.4M, hash lookup 7.4M, property access 7.1M, operand
  loads/stores 6.7M, builtin calls 4.8M, compare/arith 4.0M, GC bookkeeping 3.9M, include/link
  3.3M, unattributed 3.0M.

## Stage 1 — ~78M (2×). Safe Rust, current bytecode.

| item | targets (excess now) | goal | Zend reference |
|---|---|---:|---|
| 1. Interning: names carry a precomputed hash and pointer identity; class, function, constant, method and property tables keyed by interned name; compares become pointer checks | hash 7.4M, strings/`bcmp` 8.5M, property 7.1M | −12M | literals interned on insert, `zend_insert_literal` Zend/zend_compile.c:574-582 (§1.2); property cache slots `[ce, offset, prop_info]` Zend/zend_compile.c:3184-3188, filled by `CACHE_POLYMORPHIC_PTR_EX` Zend/zend_execute.h:555-558 (§5b). **gap:** `zend_string` hash cache and `zend_new_interned_string` (Zend/zend_string.c) |
| 2. In-place frames: one `Zval` stack, frame = header + base offset; args written straight into the callee's slots; slots released with an inline tag test; `Ret` moves its CV | calls+return 12.3M, part of copy/drop and dispatch | −12M (call ≤ 400 Ir) | frame = header + CVs + TMPs, `EX_VAR` Zend/zend_compile.h:625-635, 701-708, 735, 739 (§1.2); `zend_init_cvs` Zend/zend_execute.c:4392-4403; `i_free_compiled_variables` Zend/zend_execute.c:4271-4280; SEND into callee CV slot Zend/zend_compile.c:3908 (§5c); RETURN moves the CV Zend/zend_vm_def.h:4536-4547 (§6) |
| 3. Allocations: 164k → ≤ 80k per request (path-op key vectors, array growth, string building, `Props`) | allocator 11.1M | −7M | **gap:** `zend_alloc.c` small-bin allocator, `zend_hash` packed growth |
| 4. Ranked excess, largest first, re-ranked after each item: copy/drop (borrowed CV/CONST reads in the consumers that drop them, ZEND_VM_NOTES §7: ~54k pairs), dispatch bookkeeping (ip/func in locals, depth check on call only), "other" (attribute first), arrays, builtin-call resolution per site, GC bookkeeping, include/link | 10.3 + 10.6 + 9.4 + 8.4 + 4.8 + 3.9 + 3.3M | −27M | copy rules `zend_copy_to_variable` Zend/zend_execute.h:138-163, `ZVAL_COPY` Zend/zend_types.h:1449-1459, `zval_ptr_dtor_nogc` Zend/zend_variables.h:33-38 (§1.3, §6); handler fixed at load, `zend_vm_set_opcode_handler` Zend/zend_vm_execute.h:129377-129387, Zend/zend_opcode.c:1243 (§3.4). **gap:** `INIT_NS_FCALL_BY_NAME` runtime cache and frameless calls |

Bottom-up the four items sum to ≈ −58M, i.e. **≈ 94M expected; 78M needs about two thirds of
every addressable bucket**. The 90M gate below is therefore a live one, not a formality.

## Stage 2 — ~55M (1.3×). New instruction format.

| item | Zend reference |
|---|---|
| Three-address bytecode: operands are frame slots, no operand stack (removes the 6.7M operand load/store ops) | TMP/VAR/CV all `EX_VAR(opN.var)`, renumbered after the CVs in `pass_two` Zend/zend_opcode.c:1230-1242 (§1.2); ownership per kind (§1.3); live ranges for exception cleanup, `zend_calc_live_ranges` Zend/zend_opcode.c:935ff, `cleanup_live_vars` Zend/zend_execute.c:4884ff (§4.3) |
| 32-byte ops with compile-time operand kinds; one handler per (op, kind) resolved at load | kinds Zend/zend_compile.h:843-847 (§1.1); index `zend_vm_get_opcode_handler_idx` Zend/zend_vm_execute.h:129316-129358 (§3.4); macro expansion per kind Zend/zend_vm_gen.php:697-791 (§3.2). **gap:** `struct zend_op` layout (Zend/zend_compile.h) |
| Register-held dispatch across the whole loop (ip, frame base, literals never spilled) | HYBRID VM with global registers, Zend/zend_vm_gen.php:1990 (§2.4); VM kind choice Zend/zend_vm_opcodes.h:41-47 (§3.6) |
| Immortal interned strings and immutable literal arrays: no refcount traffic on literals | CONST operands never freed, `FREE_OP1` = `""` Zend/zend_vm_gen.php:434-443 (§1.3); `RT_CONSTANT` Zend/zend_compile.h:820-821. **gap:** `GC_IMMUTABLE` / `IS_ARRAY_IMMUTABLE` (Zend/zend_types.h) |
| Per-function runtime cache slots (property, method, function, constant, class) | `CACHE_ADDR` + monomorphic check Zend/zend_vm_def.h:2103-2105 (§5b); slot allocation Zend/zend_compile.c:3184-3188 |
| Scoped `unsafe` rules: a written amendment to DECISION_KERNEL.md §8 (which modules, which invariants, Miri coverage) before the first line | — |

Stage 2 replaces the bytecode, the lowering and `run_loop`; **upstream rebasing ends here**.

## Stage 3 — ~40–45M (time parity).

| item | Zend reference |
|---|---|
| SSA type inference per function | type-specialised selection only via the optimizer, `zend_vm_set_opcode_handler_ex` called from Zend/Optimizer/zend_optimizer.c:1446 (§2.5). **gap:** Zend/Optimizer/zend_ssa.c, zend_inference.c |
| Type-specialised ops | 42 `TYPE_SPEC` handlers; `ADD_LONG_NO_OVERFLOW` selection Zend/zend_vm_execute.h:129394-129419; `QM_ASSIGN_NOREF` Zend/zend_vm_def.h:10324-10332; `SEND_VAR_SIMPLE` Zend/zend_vm_def.h:10389-10403 (§2.5) |
| Constant propagation, dead code | **gap:** Zend/Optimizer/sccp.c, dce.c |
| Guarded inlining of trivial methods (getters/setters, class-id guard + deopt to the call) | **gap:** no opcache-optimizer counterpart known; JIT's handling to be checked |

## Decision gates

- **Stage 1 stalls above 90M** (two consecutive items under 1M each with the stage above 90M):
  stop and reassess with the owner before starting stage 2.
- **Stage 2 misses 1.5× (58.2M):** reassess before stage 3.
- Each stage ends with the bucket table re-run (`cg-report.sh`), the A/B suite and the usual
  gates; each item is its own commit with its `ir-trend.md` row.
