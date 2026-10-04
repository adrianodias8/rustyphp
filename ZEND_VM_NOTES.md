# Zend VM operand specialisation (PHP 8.5.7): notes for Ferrophant

Source tree: `php-src` at tag `php-8.5.7`. All paths are relative to that tree.
Every `file:line` was read in this tree. Anything not verified is marked **(unverified)**.

Notation: "+1" means one refcount increment (`GC_ADDREF`/`Z_ADDREF_P`), "-1" means one
decrement (`Z_DELREF`/`GC_DELREF`, usually through `zval_ptr_dtor_nogc`). Increments and
decrements happen only when the value is refcounted. Longs, doubles, null, bools and
interned strings cost 0 either way, because the macros test the type-flag bit first
(`ZVAL_COPY` at Zend/zend_types.h:1449-1459, `zval_ptr_dtor_nogc` at Zend/zend_variables.h:33-38).

Copy primitives used throughout:

| Macro | Effect | Refcount | Source |
|---|---|---|---|
| `ZVAL_COPY_VALUE(z, v)` | bitwise copy of value and type_info | 0 (a move, or a borrow if the source stays alive) | Zend/zend_types.h:1440-1447 |
| `ZVAL_COPY(z, v)` | bitwise copy | +1 if refcounted | Zend/zend_types.h:1449-1459 |
| `ZVAL_COPY_DEREF(z, v)` | if `v` is `IS_REFERENCE`, copy the inner value | +1 on the inner value if refcounted | Zend/zend_types.h:1533-1546 |
| `zval_ptr_dtor_nogc(p)` | release | -1; frees when the count reaches 0 | Zend/zend_variables.h:33-38 |

---

## 1. Operand kinds

### 1.1 Encoding

```c
#define IS_UNUSED   0       /* Unused operand */
#define IS_CONST    (1<<0)
#define IS_TMP_VAR  (1<<1)
#define IS_VAR      (1<<2)
#define IS_CV       (1<<3)  /* Compiled variable */
```
Zend/zend_compile.h:843-847. Bits 4 and 5 are reused in `result_type` as `IS_SMART_BRANCH_JMPZ/JMPNZ`
(Zend/zend_compile.h:850-851).

### 1.2 Where each kind lives

- The **call frame** is one contiguous block of zvals: the `zend_execute_data` header
  (Zend/zend_compile.h:625-635) is followed by `ZEND_CALL_FRAME_SLOT` zvals of header padding,
  then the CV slots, then the TMP/VAR slots.
  - `ZEND_CALL_VAR(call, n)` is a *byte offset* from the frame base (Zend/zend_compile.h:701-702).
    `EX_VAR(n)` is that offset applied to the current frame (Zend/zend_compile.h:735).
  - `EX_NUM_TO_VAR(n)` converts a slot number into that byte offset (Zend/zend_compile.h:739).
- **CV** (compiled variable, i.e. a named local `$x`). `lookup_cv` gives each distinct name
  slot `EX_NUM_TO_VAR(i)`, with `i` in `0..last_var` (Zend/zend_compile.c:539-560). Every CV
  is set to `IS_UNDEF` on frame entry (`zend_init_cvs`, Zend/zend_execute.c:4392-4403). All
  CVs are released with `i_zval_ptr_dtor` when the frame exits (`i_free_compiled_variables`,
  Zend/zend_execute.c:4271-4280).
  - The callee's parameters are its first CVs: `ZEND_CALL_ARG(call, n)` is
    `ZEND_CALL_VAR_NUM(call, n-1)` (Zend/zend_compile.h:707-708).
- **TMP_VAR / VAR**. The compiler numbers them `0..T` with `get_temporary_variable()`
  (`op_array->T++`, Zend/zend_compile.c:533-536). `pass_two` then moves them after the CVs:
  `opline->opN.var = EX_NUM_TO_VAR(op_array->last_var + opline->opN.var)`
  (Zend/zend_opcode.c:1230-1242). At runtime, TMP, VAR and CV operands all resolve to
  `EX_VAR(opline->opN.var)`. They are just frame slots.
- **CONST** lives in `op_array->literals`. After `pass_two`, `opN.constant` is rewritten into
  a byte offset *relative to the opline*, and `RT_CONSTANT(opline, node)` is
  `(zval*)((char*)opline + (int32_t)node.constant)` (Zend/zend_compile.h:820-821; the switch
  happens in `ZEND_PASS_TWO_UPDATE_CONSTANT`, Zend/zend_opcode.c:1230-1231). String literals
  are interned when they are inserted (`zend_insert_literal`, Zend/zend_compile.c:574-582).
- **UNUSED**: the operand has no value. The operand field may hold other data instead:
  `op2.num` = argument number for SEND (the `NUM` spec), a jump target, and so on. For
  property and method opcodes, an UNUSED op1 means `$this`: the `*_OBJ_*` getters map UNUSED
  to `&EX(This)` (Zend/zend_vm_gen.php:324-432). The compiler emits UNUSED for `$this->x`
  when `$this` is guaranteed to exist (Zend/zend_compile.c:3130-3133).

### 1.3 Ownership contract per kind

| Kind | Owner | Contract seen in the handlers |
|---|---|---|
| CONST | the op_array's literal table | Never freed by a handler: `FREE_OP1()` expands to `""` (Zend/zend_vm_gen.php:434-443). If it is copied into a slot that will own it, the handler adds +1 only when the literal is refcounted, and that check is marked `UNEXPECTED` (e.g. `ZEND_QM_ASSIGN`, Zend/zend_vm_def.h:7910-7914; `ZEND_SEND_VAL`, Zend/zend_vm_def.h:4881-4885). |
| TMP_VAR | the *single* consuming instruction | Holds an owned value, never `IS_REFERENCE` (`_get_zval_ptr_tmp` asserts it, Zend/zend_execute.c:252-259). The consumer must either move it out (`ZVAL_COPY_VALUE`, no further free) or free it exactly once (`FREE_OP1()` gives `zval_ptr_dtor_nogc(EX_VAR(...))`, Zend/zend_vm_gen.php:437). |
| VAR | the single consumer, same as TMP | Can also hold `IS_REFERENCE` (e.g. a by-ref function return; see the comment at Zend/zend_compile.c:3817-3819) or `IS_INDIRECT` (a pointer to another zval, produced by write fetches; `_get_zval_ptr_ptr_var` unwraps it, Zend/zend_execute.c:513-520). Freed by `FREE_OP1()` / `FREE_OP1_IF_VAR()`. |
| CV | the frame (variable storage) | Borrowed. Handlers never free a CV operand (`FREE_OP1()` gives `""` for CV). Copying it into a slot that will own it needs +1 (`ZVAL_COPY` / `ZVAL_COPY_DEREF`). It may be `IS_UNDEF`; a read in R mode emits "Undefined variable $x" and substitutes `&EG(uninitialized_zval)` (`_get_zval_ptr_cv_BP_VAR_R`, Zend/zend_execute.c:348-356; `zval_undefined_cv`, Zend/zend_execute.c:276-283). It may also be `IS_REFERENCE`, hence the `_deref` variants. |
| UNUSED | — | Nothing to own. `$this` (`&EX(This)`) is borrowed from the frame. |

The canonical "how do I store an operand of kind K" routine is `zend_copy_to_variable`
(Zend/zend_execute.h:138-163):

```c
if (ZEND_CONST_COND(value_type & (IS_VAR|IS_CV), 1) && Z_ISREF_P(value)) {
    ref = Z_COUNTED_P(value); value = Z_REFVAL_P(value);
}
ZVAL_COPY_VALUE(variable_ptr, value);
if (ZEND_CONST_COND(value_type == IS_CONST, 0)) {
    if (UNEXPECTED(Z_OPT_REFCOUNTED_P(variable_ptr))) Z_ADDREF_P(variable_ptr);  // CONST: rare +1
} else if (value_type & (IS_CONST|IS_CV)) {
    if (Z_OPT_REFCOUNTED_P(variable_ptr)) Z_ADDREF_P(variable_ptr);             // CV: +1
} else if (ZEND_CONST_COND(value_type == IS_VAR, 1) && UNEXPECTED(ref)) {
    if (UNEXPECTED(GC_DELREF(ref) == 0)) efree_size(ref, ...);                   // VAR ref: unwrap
    else if (Z_OPT_REFCOUNTED_P(variable_ptr)) Z_ADDREF_P(variable_ptr);
}
// TMP (and a non-reference VAR): pure move, 0 refcount ops
```
`ZEND_ASSIGN` relies on this. It never frees op2 ("zend_assign_to_variable() always takes
care of op2, never free it!", Zend/zend_vm_def.h:2834).

### 1.4 How the compiler chooses TMP or VAR

- `zend_emit_op()` gives the result kind **VAR** (`zend_make_var_result`,
  Zend/zend_compile.c:2272-2277, 2288-2306). `zend_emit_op_tmp()` gives **TMP**
  (Zend/zend_compile.c:2280-2285, 2308).
- Arithmetic and binary operators produce TMP: `zend_emit_op_tmp(result, opcode, &left_node,
  &right_node)` (Zend/zend_compile.c:10276).
- Function calls produce VAR: `opline = zend_emit_op(result, call_op, NULL, NULL)`
  (Zend/zend_compile.c:3995). The result may be a reference if the function returns by ref.
- Variable fetches (`FETCH_DIM_*`, `FETCH_OBJ_*`, `FETCH_STATIC_PROP_*`) start as VAR. In
  `zend_adjust_for_fetch_type`, `BP_VAR_R` and `BP_VAR_IS` turn the result into TMP; the W,
  RW, FUNC_ARG and UNSET modes keep VAR and only shift the opcode (Zend/zend_compile.c:2241-2268).
  So `$o->p` read is `FETCH_OBJ_R` with a **TMP** result, and `$o->p = ...` / `foo($o->p)`
  by-ref use **VAR**.
- Assignments produce TMP (`ASSIGN_DIM`, `ASSIGN_OBJ`, `ASSIGN_STATIC_PROP`;
  Zend/zend_compile.c:3484, 3496, 3509).
- Plain `$x` is a **CV** whenever `zend_try_compile_cv` succeeds (Zend/zend_compile.c:2874ff).
  It fails for auto-globals and variable-variables, which become `FETCH_R/W` with TMP/VAR results.

Rule of thumb from the above: **VAR = "may be a reference or indirect, i.e. something that
can be written through"**; **TMP = a plain owned rvalue**.

---

## 2. The spec language in `zend_vm_def.h`

### 2.1 Handler header

```
ZEND_VM_[HOT_|INLINE_|HOT_OBJ_|HOT_SEND_|HOT_NOCONST_|HOT_NOCONSTCONST_|COLD_|COLD_CONST_|COLD_CONSTCONST_]HANDLER(
    <opcode number>, <NAME>, <op1 spec>, <op2 spec> [, <ext flags>] [, SPEC(<rules>)])
```
This is the regex in `gen_vm` (Zend/zend_vm_gen.php:2642-2655). Examples:

- `ZEND_VM_HOT_NOCONSTCONST_HANDLER(1, ZEND_ADD, CONST|TMPVARCV, CONST|TMPVARCV)` (Zend/zend_vm_def.h:47)
- `ZEND_VM_HOT_OBJ_HANDLER(82, ZEND_FETCH_OBJ_R, CONST|TMPVAR|UNUSED|THIS|CV, CONST|TMPVAR|CV, CACHE_SLOT)` (Zend/zend_vm_def.h:2069)
- `ZEND_VM_HANDLER(23, ZEND_ASSIGN_DIM, VAR|CV, CONST|TMPVAR|UNUSED|NEXT|CV, SPEC(OP_DATA=CONST|TMP|VAR|CV))` (Zend/zend_vm_def.h:2657)

### 2.2 Operand-spec tokens

From `$vm_op_decode` (Zend/zend_vm_gen.php:103-122):

- **Specialising tokens** (they set `ZEND_VM_OP_SPEC`): `CONST`, `TMP`, `VAR`, `UNUSED`, `CV`,
  `TMPVAR` (one body shared by TMP and VAR), `TMPVARCV` (one body shared by TMP, VAR and CV).
- **`ANY`**: no specialisation. The kind is read at runtime from `opline->opN_type`.
- **Non-specialising tokens** are metadata only (flags written to `zend_vm_opcodes.c`): `NUM`,
  `JMP_ADDR`, `TRY_CATCH`, `LOOP_END`, `THIS`, `NEXT`, `CLASS_FETCH`, `CONSTRUCTOR`,
  `CONST_FETCH`, `CACHE_SLOT`. When combined with specialising tokens, they add no handler
  variant. For example, `UNUSED|THIS` gives only the `_UNUSED` variant, and `CONST|UNUSED|NUM`
  gives `_CONST` and `_UNUSED` (NUM operands have `op_type == IS_UNUSED`).
- A spec that contains no specialising token must be a single token, otherwise generation
  fails (Zend/zend_vm_gen.php:2408-2427).

### 2.3 `SPEC(...)` rules

Parsed by `parse_spec_rules` (Zend/zend_vm_gen.php:2444-2492). Each rule adds a dimension to the handler table:

| Rule | Values | Meaning / substitution |
|---|---|---|
| `OP_DATA=K1\|K2…` | per kind | Specialises on the kind of the following `ZEND_OP_DATA` op's op1 (the value in `$a[x] = v`). `GET_OP_DATA_ZVAL_PTR`, `FREE_OP_DATA` etc. come from `$op_data_*` (Zend/zend_vm_gen.php:478-542). |
| `RETVAL` | 0/1 | `RETURN_VALUE_USED(opline)` becomes a literal `0`/`1` (Zend/zend_vm_gen.php:757). For `ZEND_ASSIGN` with an unused result, this removes the `ZVAL_COPY(EX_VAR(result), value)` and its +1 (Zend/zend_vm_def.h:2819-2830). |
| `SMART_BRANCH` | 0/1/2 | Fuses a compare with the following `JMPZ`/`JMPNZ`. The compiler marks this with `result_type = IS_TMP_VAR \| IS_SMART_BRANCH_JMPZ` (Zend/zend_compile.c:2393-2402); the generator rewrites `ZEND_VM_SMART_BRANCH(...)` (Zend/zend_vm_gen.php:759-776). |
| `QUICK_ARG` | 0/1 | `arg_num <= MAX_ARG_FLAG_NUM` becomes a literal. Used by the SEND `_EX` handlers (Zend/zend_vm_gen.php:758). |
| `ISSET` | 0/1 | `opline->extended_value & ZEND_ISEMPTY` becomes a literal (Zend/zend_vm_gen.php:777-782). |
| `OBSERVER` | 0/1 | Observer (profiling hook) variant. Observer variants are generated *unspecialised* (op1 = op2 = ANY) and cold (Zend/zend_vm_gen.php:1040-1056, 972-976). |
| `COMMUTATIVE` | — | Generate only one operand order. Variants where `$commutative_order[op1] < $commutative_order[op2]` are skipped (Zend/zend_vm_gen.php:941-945). At load time, operands are swapped so that `op1_type >= op2_type` (Zend/zend_vm_execute.h:129381-129385). |
| `NO_CONST_CONST` | — | Skip the CONST,CONST variant (Zend/zend_vm_gen.php:935-939). |

### 2.4 Hot/cold attributes

`is_hot_handler` / `is_cold_handler` (Zend/zend_vm_gen.php:950-990):

| Prefix | Hot when | Cold when |
|---|---|---|
| `HOT_`, `INLINE_` | always | — |
| `HOT_NOCONST_` | op1 != CONST | op1 == CONST |
| `HOT_NOCONSTCONST_` | not (op1 == CONST and op2 == CONST) | CONST,CONST |
| `HOT_OBJ_` | op1 ∈ {UNUSED, CV} and op2 == CONST | op1 == CONST |
| `HOT_SEND_` | `QUICK_ARG` = 1 | — |
| `COLD_`, `COLD_CONST_`, `COLD_CONSTCONST_` | — | always / op1 CONST / both CONST |

Any `SMART_BRANCH=0` or `OBSERVER=1` variant is never hot (Zend/zend_vm_gen.php:951-956).

How the attributes are used:

- In the HYBRID VM, hot handlers are emitted `static ZEND_VM_HOT` and called from the body of
  the computed-goto `execute_ex` (Zend/zend_vm_gen.php:1078-1103).
- With global registers, HYBRID defines `ZEND_VM_HOT` as `zend_always_inline ZEND_COLD
  ZEND_OPT_SIZE` (Zend/zend_vm_gen.php:1990). This inlines the handler into the dispatch loop.
  **(unverified)** My reading is that `ZEND_COLD ZEND_OPT_SIZE` only affects the leftover
  out-of-line copy.
- Cold handlers get `ZEND_VM_COLD` = `ZEND_COLD ZEND_OPT_SIZE` (Zend/zend_vm_gen.php:1991, 1104).
- A handler that another handler dispatches to (via `ZEND_VM_DISPATCH_TO_HANDLER`) and that is
  also hot is emitted as a `*_INLINE_HANDLER` plus a thin wrapper. Example:
  `ZEND_FETCH_OBJ_R_SPEC_UNUSED_CONST_INLINE_HANDLER` (Zend/zend_vm_execute.h:34615) and its
  wrapper (Zend/zend_vm_execute.h:34787-34790).

### 2.5 Type-specialised handlers (`ZEND_VM_*TYPE_SPEC_HANDLER`)

Header regex: Zend/zend_vm_gen.php:2702-2714.

```
ZEND_VM_HOT_TYPE_SPEC_HANDLER(<base opcode(s)>, <C condition>, <NEW_NAME>, op1spec, op2spec [, ext] [, SPEC(...)])
```

- Each one gets a synthetic opcode number above 255 (`$extra_num++`, Zend/zend_vm_gen.php:2720).
  The condition is stored in `$opcodes[base]['type_spec']` (Zend/zend_vm_gen.php:2721-2728).
- Selection happens **only** in `zend_vm_set_opcode_handler_ex(op, op1_info, op2_info,
  res_info)`. Its generated `switch (opcode)` evaluates the conditions in definition order
  (Zend/zend_vm_gen.php:3100-3140). The caller is the optimizer, which passes SSA/type-inference
  masks (Zend/Optimizer/zend_optimizer.c:1446) and some opcache persistence paths
  (ext/opcache/zend_persist.c:555, 571). **Without opcache, type-specialised handlers are
  never selected.** Plain `zend_vm_set_opcode_handler` ignores them
  (Zend/zend_vm_execute.h:129377-129387).
- Generated code for ADD (Zend/zend_vm_execute.h:129394-129419):

```c
case ZEND_ADD:
    if (res_info == MAY_BE_LONG && op1_info == MAY_BE_LONG && op2_info == MAY_BE_LONG) {
        if (op->op1_type == IS_CONST && op->op2_type == IS_CONST) break;
        spec = 2586 | SPEC_RULE_OP1 | SPEC_RULE_OP2 | SPEC_RULE_COMMUTATIVE;   // ADD_LONG_NO_OVERFLOW
        if (op->op1_type < op->op2_type) zend_swap_operands(op);
    } else if (op1_info == MAY_BE_LONG && op2_info == MAY_BE_LONG) { ... spec = 2611 ...  // ADD_LONG
    } else if (op1_info == MAY_BE_DOUBLE && op2_info == MAY_BE_DOUBLE) { ... spec = 2636 ... // ADD_DOUBLE
```

There are 42 `TYPE_SPEC_HANDLER` definitions in Zend/zend_vm_def.h (grep count). Other
examples:

- `ZEND_QM_ASSIGN_NOREF`: the value is known not to be refcounted, so the handler is just a
  `ZVAL_COPY_VALUE` (Zend/zend_vm_def.h:10324-10332).
- `ZEND_SEND_VAR_SIMPLE`: the value is known to be neither UNDEF nor a reference (Zend/zend_vm_def.h:10389).
- `ZEND_SEND_VAL_SIMPLE`: a non-refcounted constant (Zend/zend_vm_def.h:10428).

---

## 3. How `zend_vm_gen.php` expands a handler

### 3.1 Enumeration

`gen_executor_code` iterates op1 over `$op_types_ex` = `ANY, CONST, TMPVARCV, TMPVAR, TMP, VAR,
UNUSED, CV` (Zend/zend_vm_gen.php:159-168). Op2 iterates over the same list, and so do the
extra-spec values. A body is generated whenever both kinds appear literally in the handler's
spec (Zend/zend_vm_gen.php:1733-1750). So `TMPVARCV` produces **one** C function that serves
TMP, VAR and CV. The suffix comes from `$prefix` (`_CONST`, `_TMPVARCV`, …,
Zend/zend_vm_gen.php:170-179), giving names like `ZEND_ADD_SPEC_CONST_TMPVARCV_HANDLER`.

### 3.2 Macro substitution (`gen_code`, Zend/zend_vm_gen.php:697-791)

This is purely textual `preg_replace`. The main rows (op1 shown; op2 and OP_DATA are symmetric):

| Macro in vm_def | CONST | TMP | VAR | CV | UNUSED | TMPVAR | TMPVARCV | ANY |
|---|---|---|---|---|---|---|---|---|
| `OP1_TYPE` (192-201) | `IS_CONST` | `IS_TMP_VAR` | `IS_VAR` | `IS_CV` | `IS_UNUSED` | `(IS_TMP_VAR\|IS_VAR)` | `(IS_TMP_VAR\|IS_VAR\|IS_CV)` | `opline->op1_type` |
| `GET_OP1_ZVAL_PTR(t)` (214-223) | `RT_CONSTANT(opline, opline->op1)` | `_get_zval_ptr_tmp(..)` | `_get_zval_ptr_var(..)` | `_get_zval_ptr_cv_<t>(..)` (UNDEF check + warning) | `NULL` | `_get_zval_ptr_var` | `???` | `get_zval_ptr(...)` |
| `GET_OP1_ZVAL_PTR_UNDEF(t)` (280-289) | RT_CONSTANT | `_tmp` | `_var` | **`EX_VAR(opline->op1.var)`** (no check) | NULL | `_var` | `EX_VAR(..)` | runtime |
| `GET_OP1_ZVAL_PTR_DEREF(t)` (258-267) | RT_CONSTANT | `_tmp` | `_var_deref` | `_cv_deref_<t>` | NULL | `???` | `_get_zval_ptr_tmpvarcv(..)` | runtime |
| `GET_OP1_ZVAL_PTR_PTR(t)` (236-245), for writes | `zend_get_bad_ptr()` | bad_ptr | `_get_zval_ptr_ptr_var` (unwraps INDIRECT) | `_cv_<t>` | NULL | `???` | `???` | runtime |
| `GET_OP1_OBJ_ZVAL_PTR*` (324-432) | same as above, except **UNUSED → `&EX(This)`** | | | | | | | |
| `FREE_OP1()` (434-443) | `""` | `zval_ptr_dtor_nogc(EX_VAR(opline->op1.var))` | same | `""` | `""` | same as TMP | `FREE_OP(opline->op1_type, ...)` (runtime test) | `FREE_OP(...)` |
| `FREE_OP1_IF_VAR()` (456-465) | `""` | **`""`** | `zval_ptr_dtor_nogc(EX_VAR(..))` | `""` | `""` | `???` | `???` | `if (opline->op1_type == IS_VAR) {...}` |

Row numbers are lines in Zend/zend_vm_gen.php.

- `"???"` is emitted literally. Using that macro with that kind is therefore a C compile
  error, i.e. the combination is unsupported for that macro.
- `FREE_OP(type, var)` is `if ((type) & (IS_TMP_VAR|IS_VAR)) zval_ptr_dtor_nogc(EX_VAR(var));`
  (Zend/zend_execute.c:177-180).
- 8.5.7 has no `FREE_OP1_VAR_PTR` macro: grep finds zero occurrences in vm_def.h and vm_gen.php.

Other substitutions:

- `ZEND_VM_SPEC` becomes `1` in any specialised body and `!ZEND_VM_SPEC` becomes `0`
  (Zend/zend_vm_gen.php:742-743).
- `ZEND_VM_C_LABEL/GOTO` labels get a `_SPEC…` suffix in the SWITCH/GOTO VMs to stay unique
  (Zend/zend_vm_gen.php:744-745).
- `#if 1 || …` / `#if 0 && …` preprocessor lines are folded (Zend/zend_vm_gen.php:746-749).

### 3.3 Dead-code elimination

The generator does **not** remove C `if`s. After substitution, conditions such as
`if (IS_CONST == IS_CONST)` or `if (1 && IS_CONST == IS_CONST && (IS_TMP_VAR|IS_VAR|IS_CV) == IS_CONST)`
remain in Zend/zend_vm_execute.h (e.g. line 8598). The file contains 554 lines matching
`if (IS_CONST == IS_CONST)` or `if (IS_CV == IS_CONST)`. The C compiler folds them.

Empty `FREE_OP1()` expansions leave blank lines; see the two empty lines where `FREE_OP1();`
stood in `ZEND_SEND_VAR_SPEC_CV_UNUSED_HANDLER` (Zend/zend_vm_execute.h:51946-51947).

### 3.4 Handler table and index computation

- `zend_spec_handlers[opcode]` holds `start_index | SPEC_RULE_*` flags. For example, ADD is
  `1 | SPEC_RULE_OP1 | SPEC_RULE_OP2` (Zend/zend_vm_execute.h:128952). The flag bits are at
  Zend/zend_vm_execute.h:311-321.
- The index is computed by `zend_vm_get_opcode_handler_idx` (Zend/zend_vm_execute.h:129316-129358):

```c
static const int zend_vm_decode[] = { _UNUSED_CODE, _CONST_CODE, _TMP_CODE, _UNUSED_CODE,
                                      _VAR_CODE, _UNUSED_CODE, _UNUSED_CODE, _UNUSED_CODE, _CV_CODE };
uint32_t offset = 0;
if (spec & SPEC_RULE_OP1) offset = offset * 5 + zend_vm_decode[op->op1_type];
if (spec & SPEC_RULE_OP2) offset = offset * 5 + zend_vm_decode[op->op2_type];
if (spec & SPEC_EXTRA_MASK) { /* RETVAL*2, QUICK_ARG*2, OP_DATA*5, ISSET*2, SMART_BRANCH*3, OBSERVER*2 */ }
return (spec & SPEC_START_MASK) + offset;
```

  The codes are `_CONST_CODE 0, _TMP_CODE 1, _VAR_CODE 2, _UNUSED_CODE 3, _CV_CODE 4`
  (Zend/zend_execute.c:113-117). Each opcode with two specialised operands therefore gets a
  dense 5×5 block.
- `zend_vm_set_opcode_handler` writes `op->handler = zend_opcode_handlers[idx]`, after the
  commutative swap (Zend/zend_vm_execute.h:129377-129387). `pass_two` calls it for every
  opline (`ZEND_VM_SET_OPCODE_HANDLER`, Zend/zend_opcode.c:1243). **Dispatch is therefore
  resolved once at compile/load time.** At runtime the VM jumps to `opline->handler` with no
  per-execution type switch on operand kinds.
- In the TMP and VAR slots of the block, the gen fills in the `TMPVAR`/`TMPVARCV` body if one
  exists. CV falls back to `TMPVARCV`. Anything else falls back to `ANY`, or to NULL
  (Zend/zend_vm_gen.php:1287-1334, 1366-1420).
- ADD's block shows this. Entry order is op1 ∈ {CONST, TMP, VAR, UNUSED, CV} × op2 in the same
  order (Zend/zend_vm_execute.h:121954-121978):

```
CONST_CONST, CONST_TMPVARCV, CONST_TMPVARCV, NULL, CONST_TMPVARCV,
TMPVARCV_CONST, TMPVARCV_TMPVARCV, TMPVARCV_TMPVARCV, NULL, TMPVARCV_TMPVARCV,   (op1=TMP)
... (op1=VAR same) ..., NULL x5 (op1=UNUSED), ... (op1=CV same)
```

### 3.5 Unsupported combinations

Unsupported combinations get `ZEND_NULL_HANDLER` (Zend/zend_vm_execute.h:56502-56508). Its body is

```c
zend_error_noreturn(E_ERROR, "Invalid opcode %d/%d/%d.", OPLINE->opcode, OPLINE->op1_type, OPLINE->op2_type);
```

It exists only so that the table stays dense. The compiler must never emit those combinations.

### 3.6 Counts (8.5.7, `ZEND_VM_SPEC` = 1, Zend/zend_vm_opcodes.h:26)

- 208 opcode handler definitions plus 42 `TYPE_SPEC` definitions in Zend/zend_vm_def.h
  (grep counts). `ZEND_VM_LAST_OPCODE` = 210 (Zend/zend_vm_opcodes.h:335).
- The CALL-kind handler table `funcs[]` (Zend/zend_vm_execute.h:121952-125447) has **3494
  slots**. **1616** of them are `ZEND_NULL_HANDLER`, and the remaining 1878 slots point to
  **1021 distinct specialised handler functions**. Counted with grep/sort over that line range.
- The HYBRID `labels[]` table (Zend/zend_vm_execute.h:112701-116196) also has 3494 entries and
  1021 distinct labels.
- The file contains a second full copy of every handler for the TAILCALL VM kind (`*_TAILCALL_HANDLER`,
  e.g. Zend/zend_vm_execute.h:64269). That is why there are about 2046 distinct `_HANDLER`
  function names in total.
- The VM kind is chosen in Zend/zend_vm_opcodes.h:41-47: HYBRID with GCC global regs, else
  TAILCALL if `musttail` and `preserve_none` are available, else CALL.

---

## 4. Freeing, moving, results and exceptions

### 4.1 No `free_op1` variable

In 8.5.7 there is **no `zend_free_op free_op1` local**. grep finds no `free_op1` identifier in
Zend/zend_vm_def.h, and the only hit in Zend/zend_vm_gen.php is a stale comment at line 2928.
Freeing is re-derived from the operand's slot:

- `FREE_OP1()` gives `zval_ptr_dtor_nogc(EX_VAR(opline->op1.var))` for TMP/VAR/TMPVAR, and
  nothing for CONST/CV/UNUSED (Zend/zend_vm_gen.php:434-443).
- `FREE_OP1_IF_VAR()` frees only a VAR, and is empty for TMP (Zend/zend_vm_gen.php:456-465).
  It is used where a TMP was *moved* into the result and must not be freed, while a VAR was
  copied. `ZEND_CAST` is the example (Zend/zend_vm_def.h:6516-6525):

```c
ZVAL_COPY_VALUE(result, expr);
if (OP1_TYPE == IS_CONST) { if (UNEXPECTED(Z_OPT_REFCOUNTED_P(result))) Z_ADDREF_P(result); }
else if (OP1_TYPE != IS_TMP_VAR) { if (Z_OPT_REFCOUNTED_P(result)) Z_ADDREF_P(result); }   // VAR, CV: +1
FREE_OP1_IF_VAR();                                                                          // VAR: -1
```
  Net effect: TMP = move with 0 ops. VAR = +1 then -1. (The VAR operand here was dereferenced
  first, at Zend/zend_vm_def.h:6513-6514, so it is copied out of the reference.)

### 4.2 Writing the result slot

The result is always `EX_VAR(opline->result.var)`. The result slot is *uninitialised* on
entry: handlers overwrite it without destroying the old contents. The live-range rules below
guarantee that the slot holds nothing live at that point. **(Inferred from the handlers
writing with `ZVAL_COPY_VALUE`/`ZVAL_LONG` and never calling a dtor on the result first;
there is no explicit comment.)**

`ZEND_QM_ASSIGN` (Zend/zend_vm_def.h:7882-7915) shows every move/copy case side by side:

| op1 kind | code | refcount ops |
|---|---|---|
| CV | `ZVAL_COPY_DEREF(result, value)` (7896-7897) | +1 |
| VAR, not a ref | `ZVAL_COPY_VALUE` (7907) | 0 (move) |
| VAR, ref | copy inner value; `Z_DELREF` the ref; if the ref survives, +1 on the inner value (7899-7905) | -1 on ref (+1 inner) |
| TMP | `ZVAL_COPY_VALUE` (7910) | 0 (move) |
| CONST | `ZVAL_COPY_VALUE` + `UNEXPECTED` addref (7910-7914) | normally 0 |

### 4.3 Exceptions and live ranges

- `HANDLE_EXCEPTION()` is `LOAD_OPLINE(); ZEND_VM_CONTINUE()` (Zend/zend_vm_execute.h:432).
  The throwing code has already pointed the opline at the exception handler op. The
  `ZEND_HANDLE_EXCEPTION` handler (Zend/zend_vm_def.h:8174ff) calls `cleanup_live_vars` and
  then searches the try/catch table.
- **Live ranges** are `zend_live_range {var, start, end}`. The kind sits in the low bits of
  `var`: `ZEND_LIVE_TMPVAR`, `LOOP`, `SILENCE`, `ROPE`, `NEW` (Zend/zend_compile.h:180-191).
  `zend_calc_live_ranges` (Zend/zend_opcode.c:935ff) builds them by scanning backwards from
  each TMP/VAR use to its definition.
  - For the default case `start++` applies, so the range is **[def+1, use)**: it excludes
    both the defining and the consuming instruction (Zend/zend_opcode.c:825-827).
  - If the use immediately follows the definition, no range is stored ("Skip trivial
    live-range", Zend/zend_opcode.c:965-966).
  - An optional `needs_live_range` callback lets opcache drop ranges for values that are known
    not to be refcounted (Zend/zend_opcode.c:829-831).
- `cleanup_live_vars(ex, op_num, catch_op_num)` (Zend/zend_execute.c:4884ff) destroys every
  range with `start <= op_num < end`. For `ZEND_LIVE_TMPVAR` it calls
  `zval_ptr_dtor_nogc(var)`. LOOP, NEW and ROPE get dedicated cleanup.
- Consequence for handler authors: because a range ends at its consumer, **a handler that
  throws must already have freed its own TMP/VAR operands**. That is why the error paths do
  `FREE_OP1(); HANDLE_EXCEPTION();`. Examples:
  - the `zend_handle_named_arg` failure in `ZEND_SEND_VAR`, which expands to
    `zval_ptr_dtor_nogc(EX_VAR(opline->op1.var)); HANDLE_EXCEPTION();` for VAR
    (Zend/zend_vm_execute.h:30795-30797);
  - `zend_add_helper`, which frees TMP/VAR operands before `ZEND_VM_NEXT_OPCODE_CHECK_EXCEPTION()`
    (Zend/zend_vm_def.h:37-44).

  Values still waiting in *other* slots, such as an earlier operand of a pending call, are
  covered by their live ranges.
- `keeps_op1_alive` lists the opcodes that read but do not consume their TMP op1: `CASE`,
  `SWITCH_*`, `MATCH`, `FETCH_LIST_*`, `COPY_TMP`, … (Zend/zend_opcode.c:895-913). Their op1
  stays live until a later `FREE`.

---

## 5. Worked examples

### 5a. `ZEND_ADD`

**Source** (Zend/zend_vm_def.h:47-83, trimmed):

```c
ZEND_VM_HOT_NOCONSTCONST_HANDLER(1, ZEND_ADD, CONST|TMPVARCV, CONST|TMPVARCV)
{
    op1 = GET_OP1_ZVAL_PTR_UNDEF(BP_VAR_R);
    op2 = GET_OP2_ZVAL_PTR_UNDEF(BP_VAR_R);
    if (ZEND_VM_SPEC && OP1_TYPE == IS_CONST && OP2_TYPE == IS_CONST) {
        /* pass */
    } else if (EXPECTED(Z_TYPE_INFO_P(op1) == IS_LONG)) {
        if (EXPECTED(Z_TYPE_INFO_P(op2) == IS_LONG)) {
            result = EX_VAR(opline->result.var);
            fast_long_add_function(result, op1, op2);
            ZEND_VM_NEXT_OPCODE();
        } else if (EXPECTED(Z_TYPE_INFO_P(op2) == IS_DOUBLE)) { d1 = (double)Z_LVAL_P(op1); d2 = Z_DVAL_P(op2); goto add_double; }
    } else if (EXPECTED(Z_TYPE_INFO_P(op1) == IS_DOUBLE)) { ... ZVAL_DOUBLE(result, d1 + d2); ZEND_VM_NEXT_OPCODE(); ... }
    ZEND_VM_DISPATCH_TO_HELPER(zend_add_helper, op_1, op1, op_2, op2);
}
```

**Slow-path helper** (Zend/zend_vm_def.h:26-45). It is declared `ANY, ANY`, so only one copy
exists: `zend_add_helper_SPEC` (Zend/zend_vm_execute.h:459-477).

```c
SAVE_OPLINE();
if (UNEXPECTED(Z_TYPE_INFO_P(op_1) == IS_UNDEF)) op_1 = ZVAL_UNDEFINED_OP1();   // CV undef → warning
if (UNEXPECTED(Z_TYPE_INFO_P(op_2) == IS_UNDEF)) op_2 = ZVAL_UNDEFINED_OP2();
add_function(EX_VAR(opline->result.var), op_1, op_2);
if (opline->op1_type & (IS_TMP_VAR|IS_VAR)) zval_ptr_dtor_nogc(op_1);           // runtime kind test
if (opline->op2_type & (IS_TMP_VAR|IS_VAR)) zval_ptr_dtor_nogc(op_2);
ZEND_VM_NEXT_OPCODE_CHECK_EXCEPTION();
```

**`ZEND_ADD_SPEC_CONST_TMPVARCV_HANDLER`** (Zend/zend_vm_execute.h:8590-8626):

```c
op1 = RT_CONSTANT(opline, opline->op1);
op2 = EX_VAR(opline->op2.var);                     // TMP, VAR or CV: raw slot, no UNDEF/ref check
if (1 && IS_CONST == IS_CONST && (IS_TMP_VAR|IS_VAR|IS_CV) == IS_CONST) {   // folds to false
} else if (EXPECTED(Z_TYPE_INFO_P(op1) == IS_LONG)) {
    if (EXPECTED(Z_TYPE_INFO_P(op2) == IS_LONG)) {
        result = EX_VAR(opline->result.var);
        fast_long_add_function(result, op1, op2);
        ZEND_VM_NEXT_OPCODE();
    } ...
ZEND_VM_DISPATCH_TO_HELPER(zend_add_helper_SPEC(ZEND_OPCODE_HANDLER_ARGS_PASSTHRU_EX op1, op2));
```

**`ZEND_ADD_SPEC_TMPVARCV_TMPVARCV_HANDLER`** (Zend/zend_vm_execute.h:14537-14573) is the same
code with `op1 = EX_VAR(opline->op1.var)`.

Annotations:

- Both operands are borrowed as `zval*` pointers into the literal table or the frame. Nothing
  is copied onto a stack.
- **Fast path (long+long, long+double, double+double): 0 increments, 0 decrements.** No
  `FREE_OP` is needed even when an operand is a TMP, because a long or double is never
  refcounted. A TMP holding a long can simply be overwritten later.
- The UNDEF check (CV) and the reference check (VAR or CV holding `IS_REFERENCE`) are not done
  separately. Their type tags are neither `IS_LONG` nor `IS_DOUBLE`, so they fall through to
  the helper, and the generic `add_function` handles dereferencing.
- `fast_long_add_function` turns overflow into a double result (Zend/zend_operators.h:704ff,
  e.g. the `__builtin_saddl_overflow` branch at lines 762-768).
- **Slow path:** CONST and CV operands stay borrowed. TMP and VAR operands are consumed with
  -1 each in the helper.
- Plain `ZEND_ADD` has no `COMMUTATIVE` flag (Zend/zend_vm_execute.h:128952), so CONST_TMPVARCV
  and TMPVARCV_CONST both exist. CONST_CONST exists but is cold, and its body goes straight to
  the helper.

**Type-specialised `ZEND_ADD_LONG_NO_OVERFLOW`.** Source at Zend/zend_vm_def.h:9947-9957
(`SPEC(NO_CONST_CONST,COMMUTATIVE)`). Selected when `res_info == op1_info == op2_info ==
MAY_BE_LONG`, i.e. the optimizer proved no overflow (Zend/zend_vm_execute.h:129395-129401).
`ZEND_ADD_LONG_NO_OVERFLOW_SPEC_TMPVARCV_CONST_HANDLER` (Zend/zend_vm_execute.h:14055-14065):

```c
op1 = EX_VAR(opline->op1.var);
op2 = RT_CONSTANT(opline, opline->op2);
result = EX_VAR(opline->result.var);
ZVAL_LONG(result, Z_LVAL_P(op1) + Z_LVAL_P(op2));
ZEND_VM_NEXT_OPCODE();
```

- No type checks and no overflow check: 0 refcount operations, 0 branches.
- Because of `COMMUTATIVE`, only `TMPVARCV_CONST` and `TMPVARCV_TMPVARCV` exist. No
  `CONST_TMPVARCV` symbol exists (grep count 0); the operands are swapped at load time instead.
- `ZEND_ADD_LONG` (Zend/zend_vm_def.h:9959) keeps `fast_long_add_function` for overflow.
  `ZEND_ADD_DOUBLE` (Zend/zend_vm_def.h:9971) uses a plain `ZVAL_DOUBLE`.

### 5b. `ZEND_FETCH_OBJ_R` (`$obj->prop` read)

**Source** (Zend/zend_vm_def.h:2069-2236, trimmed):

```c
ZEND_VM_HOT_OBJ_HANDLER(82, ZEND_FETCH_OBJ_R, CONST|TMPVAR|UNUSED|THIS|CV, CONST|TMPVAR|CV, CACHE_SLOT)
{
    container = GET_OP1_OBJ_ZVAL_PTR_UNDEF(BP_VAR_R);                         // 2076
    if (OP1_TYPE == IS_CONST || (OP1_TYPE != IS_UNUSED && UNEXPECTED(Z_TYPE_P(container) != IS_OBJECT))) {
        /* deref VAR/CV refs; CV undef warning; zend_wrong_property_read; result = NULL; goto finish */
    }
    zobj = Z_OBJ_P(container);
    if (OP2_TYPE == IS_CONST) {
        cache_slot = CACHE_ADDR(opline->extended_value & ~ZEND_FETCH_REF);  // 2103
        if (EXPECTED(zobj->ce == CACHED_PTR_EX(cache_slot))) {               // 2105 monomorphic class check
            uintptr_t prop_offset = (uintptr_t)CACHED_PTR_EX(cache_slot + 1);
            if (EXPECTED(IS_VALID_PROPERTY_OFFSET(prop_offset))) {
fetch_obj_r_simple:
                retval = OBJ_PROP(zobj, prop_offset);                         // direct slot in the object
                if (EXPECTED(Z_TYPE_INFO_P(retval) != IS_UNDEF)) {
                    if (!ZEND_VM_SPEC || (OP1_TYPE & (IS_TMP_VAR|IS_VAR)) != 0) {
                        goto fetch_obj_r_copy;                                 // must still FREE_OP1
                    } else {
fetch_obj_r_fast_copy:
                        ZVAL_COPY_DEREF(EX_VAR(opline->result.var), retval);   // 2116
                        ZEND_VM_NEXT_OPCODE();
                    }
                }
            } else if (IS_HOOKED_PROPERTY_OFFSET(prop_offset)) { /* property hooks: may push a call frame */ }
            else if (zobj->properties) { /* dynamic property: cached bucket index */ }
        }
        name = Z_STR_P(GET_OP2_ZVAL_PTR(BP_VAR_R));
    } else { name = zval_try_get_tmp_string(GET_OP2_ZVAL_PTR(BP_VAR_R), &tmp_name); ... }
    retval = zobj->handlers->read_property(zobj, name, BP_VAR_R, cache_slot, EX_VAR(opline->result.var)); // 2211
    if (retval != EX_VAR(opline->result.var)) {
fetch_obj_r_copy:
        ZVAL_COPY_DEREF(EX_VAR(opline->result.var), retval);                  // 2226
    } else if (UNEXPECTED(Z_ISREF_P(retval))) { zend_unwrap_reference(retval); }
fetch_obj_r_finish:
    FREE_OP2();                                                               // 2233
    FREE_OP1();                                                               // 2234
    ZEND_VM_NEXT_OPCODE_CHECK_EXCEPTION();
}
```

The compiler allocates 3 cache slots per CONST property name (Zend/zend_compile.c:3184-3188):
`[ce, offset, prop_info]`. The slow path fills them through
`CACHE_POLYMORPHIC_PTR_EX(cache_slot, ce, offset)` (Zend/zend_object_handlers.c:458; macro at
Zend/zend_execute.h:555-558). `read_property` returns either a *borrowed* pointer into the
object (`retval = OBJ_PROP(...)`, Zend/zend_object_handlers.c:755) or `rv`, i.e. the result
slot itself, already holding an *owned* value (e.g. Zend/zend_object_handlers.c:764-765). The
VM handles both: copy with +1 when it is not the result slot, otherwise only unwrap a reference.

**`ZEND_FETCH_OBJ_R_SPEC_UNUSED_CONST_INLINE_HANDLER`** for `$this->prop`
(Zend/zend_vm_execute.h:34615-34785, key lines):

```c
container = &EX(This);                                                   // 34622
if (IS_UNUSED == IS_CONST || (IS_UNUSED != IS_UNUSED && ...)) {...}      // folds to false: no object check
...
if (0 || (IS_UNUSED & (IS_TMP_VAR|IS_VAR)) != 0) { goto fetch_obj_r_copy; }   // folds to false
else {
fetch_obj_r_fast_copy:
    ZVAL_COPY_DEREF(EX_VAR(opline->result.var), retval);                 // 34661-34662
    ZEND_VM_NEXT_OPCODE();
}
...
if (IS_UNUSED & IS_CV) { GC_ADDREF(zobj); }        // hook path, dead for UNUSED (34677-34679)
...
fetch_obj_r_finish:
                                                   // FREE_OP2/FREE_OP1 expanded to nothing (34779-34783)
ZEND_VM_NEXT_OPCODE_CHECK_EXCEPTION();
```

The CV,CONST variant (`ZEND_FETCH_OBJ_R_SPEC_CV_CONST_INLINE_HANDLER`,
Zend/zend_vm_execute.h:44050ff) is the same, except that `container = EX_VAR(opline->op1.var)`
(44057) and the `Z_TYPE_P(container) != IS_OBJECT` check stays (44059-44060).

**`ZEND_FETCH_OBJ_R_SPEC_TMPVAR_CONST_HANDLER`**, e.g. `foo()->prop`
(Zend/zend_vm_execute.h:16716-16884):

```c
container = _get_zval_ptr_var(opline->op1.var EXECUTE_DATA_CC);          // 16723
...
if (0 || ((IS_TMP_VAR|IS_VAR) & (IS_TMP_VAR|IS_VAR)) != 0) {             // 16758: true
    goto fetch_obj_r_copy;
}
...
fetch_obj_r_copy:
    ZVAL_COPY_DEREF(EX_VAR(opline->result.var), retval);                 // 16873-16874
...
fetch_obj_r_finish:
    zval_ptr_dtor_nogc(EX_VAR(opline->op1.var));                         // 16883  FREE_OP1 on the container
ZEND_VM_NEXT_OPCODE_CHECK_EXCEPTION();
```

Annotations for a declared property, cache hit:

| Variant | container | property value → result | container release | total |
|---|---|---|---|---|
| UNUSED,CONST (`$this->p`) | borrowed `&EX(This)`, no type check | `ZVAL_COPY_DEREF` +1 (if refcounted) | none | **+1** |
| CV,CONST (`$o->p`) | borrowed frame slot | +1 | none | **+1** |
| TMPVAR,CONST (`f()->p`) | consumed | +1 | `zval_ptr_dtor_nogc` -1 (may destroy the object) | **+1, -1** |

- The +1 on the property value is unavoidable here. The result slot is a TMP that will be
  consumed and freed independently of the object. Since the object might die (the TMPVAR
  case), the value cannot be borrowed.
- The fast path also needs `SAVE_OPLINE()` (2075), one class-pointer compare and one offset
  load. There is no hash lookup.
- For TMP/VAR, the code cannot leave through `fast_copy` + `ZEND_VM_NEXT_OPCODE()`, because
  `FREE_OP1()` must run. That is the only reason for the
  `(OP1_TYPE & (IS_TMP_VAR|IS_VAR))` split at 2112.
- **Ownership transfer to a callee (property hook path):** for CV,
  `GC_ADDREF(zobj)` + `ZEND_CALL_RELEASE_THIS`. For TMP/VAR, only `ZEND_CALL_RELEASE_THIS`:
  the TMP's reference is *moved* into the hook's `This`, and the handler skips `FREE_OP1` by
  entering the callee directly. For UNUSED, neither applies (Zend/zend_vm_def.h:2130-2136;
  expanded at Zend/zend_vm_execute.h:34677-34682).

### 5c. `ZEND_SEND_VAR` and its siblings

**Opcode choice** (`zend_compile_args`, Zend/zend_compile.c:3795-3909). Notation: `fbc` known =
the callee is resolved at compile time.

| Argument expression | callee known, by-value param | callee known, by-ref param | callee unknown |
|---|---|---|---|
| CV `$x` | `SEND_VAR` (3836, 3875) | `SEND_REF` | `SEND_VAR_EX` (runtime by-ref check) (3844-3848) |
| other variable (`$a[0]`, `$o->p`) compiled R → TMP | `SEND_VAL` (`op_type == IS_TMP_VAR ? SEND_VAL : SEND_VAR`, 3836) | `SEND_REF` | `CHECK_FUNC_ARG` + `SEND_FUNC_ARG` (3851-3862) |
| call result (VAR) | `SEND_VAR`, or `SEND_VAL` when the param *may* be by-ref (3814-3823) | `SEND_VAR_NO_REF` (3815) | `SEND_VAR_NO_REF_EX` (3826) |
| `++$a`-like VAR | `SEND_VAR` / `SEND_VAL` (3868-3875) | `SEND_VAR_NO_REF` | `SEND_VAR_NO_REF_EX` |
| CONST or TMP expression | `SEND_VAL` (3893) | `SEND_VAL_EX` (runtime error) | `SEND_VAL_EX` (3895) |

Without named args, the destination is fixed at compile time:
`opline->result.var = EX_NUM_TO_VAR(arg_num - 1)` (Zend/zend_compile.c:3908). This is the
callee's CV slot inside the already-pushed frame `EX(call)`, reached with
`ZEND_CALL_VAR(EX(call), opline->result.var)`. **Arguments are written directly into the
callee's local-variable slots. There is no separate argument stack.**

Handler specs:

- `SEND_VAL`: `CONST|TMPVAR` (4861)
- `SEND_VAL_EX`: `CONST|TMP` (4901)
- `SEND_VAR`: `VAR|CV` (4938)
- `SEND_VAR_NO_REF`: `VAR` (4985)
- `SEND_VAR_NO_REF_EX`: `VAR` (5016)
- `SEND_VAR_EX`: `VAR|CV` (5114)

All in Zend/zend_vm_def.h. `op2` is `CONST` for a named arg and `UNUSED|NUM` for a positional one.

**`ZEND_SEND_VAR` source** (Zend/zend_vm_def.h:4938-4983, trimmed):

```c
ZEND_VM_HOT_HANDLER(117, ZEND_SEND_VAR, VAR|CV, CONST|UNUSED|NUM)
{
    if (OP2_TYPE == IS_CONST) { /* named arg; on failure: FREE_OP1(); HANDLE_EXCEPTION(); */ }
    else arg = ZEND_CALL_VAR(EX(call), opline->result.var);
    varptr = GET_OP1_ZVAL_PTR_UNDEF(BP_VAR_R);
    if (OP1_TYPE == IS_CV && UNEXPECTED(Z_TYPE_INFO_P(varptr) == IS_UNDEF)) {
        SAVE_OPLINE(); ZVAL_UNDEFINED_OP1(); ZVAL_NULL(arg); ZEND_VM_NEXT_OPCODE_CHECK_EXCEPTION();
    }
    if (OP1_TYPE == IS_CV) {
        ZVAL_COPY_DEREF(arg, varptr);                                  // 4965
    } else /* IS_VAR */ {
        if (UNEXPECTED(Z_ISREF_P(varptr))) {
            zend_refcounted *ref = Z_COUNTED_P(varptr);
            varptr = Z_REFVAL_P(varptr);
            ZVAL_COPY_VALUE(arg, varptr);
            if (UNEXPECTED(GC_DELREF(ref) == 0)) efree_size(ref, sizeof(zend_reference));
            else if (Z_OPT_REFCOUNTED_P(arg)) Z_ADDREF_P(arg);
        } else {
            ZVAL_COPY_VALUE(arg, varptr);                              // 4978: move
        }
    }
    ZEND_VM_NEXT_OPCODE();
}
```

**`ZEND_SEND_VAR_SPEC_CV_UNUSED_HANDLER`** (Zend/zend_vm_execute.h:51935-51980):

```c
if (IS_UNUSED == IS_CONST) { ... /* dead */ } else { arg = ZEND_CALL_VAR(EX(call), opline->result.var); }
varptr = EX_VAR(opline->op1.var);
if (IS_CV == IS_CV && UNEXPECTED(Z_TYPE_INFO_P(varptr) == IS_UNDEF)) { ... warning, arg = NULL ... }
if (IS_CV == IS_CV) {
    ZVAL_COPY_DEREF(arg, varptr);          // borrow from frame → +1 into callee slot
} else { /* dead VAR branch */ }
ZEND_VM_NEXT_OPCODE();
```

**`ZEND_SEND_VAR_SPEC_VAR_UNUSED_HANDLER`** (Zend/zend_vm_execute.h:30785-30829):

```c
varptr = _get_zval_ptr_var(opline->op1.var EXECUTE_DATA_CC);
if (IS_VAR == IS_CV && ...) { /* dead */ }
if (IS_VAR == IS_CV) { /* dead */ }
else {
    if (UNEXPECTED(Z_ISREF_P(varptr))) { /* unwrap: -1 on ref, +1 on inner if ref survives */ }
    else { ZVAL_COPY_VALUE(arg, varptr); }   // move: 0 ops, VAR slot is now logically empty
}
```

**`ZEND_SEND_VAL_SPEC_TMPVAR_UNUSED_HANDLER`** (Zend/zend_vm_execute.h:19188-19213):

```c
value = _get_zval_ptr_var(opline->op1.var EXECUTE_DATA_CC);
ZVAL_COPY_VALUE(arg, value);                          // move, 0 ops
if ((IS_TMP_VAR|IS_VAR) == IS_CONST) { ... }          // dead
ZEND_VM_NEXT_OPCODE();
```
For a VAR, `SEND_VAL` passes an `IS_REFERENCE` through *without* dereferencing. That is
intentional for "may be sent by ref" params (comment at Zend/zend_compile.c:3817-3819).

**Type-specialised `ZEND_SEND_VAR_SIMPLE`** applies when opcache proved the value is not UNDEF
and not a ref (Zend/zend_vm_def.h:10389-10403). `ZEND_SEND_VAR_SIMPLE_SPEC_CV_HANDLER`
(Zend/zend_vm_execute.h:43028-43041):

```c
varptr = EX_VAR(opline->op1.var);
arg = ZEND_CALL_VAR(EX(call), opline->result.var);
ZVAL_COPY(arg, varptr);      // +1 if refcounted; no UNDEF test, no deref test
ZEND_VM_NEXT_OPCODE();
```

Annotations (fast path, positional arg):

| Opcode / kind | source | destination write | refcount |
|---|---|---|---|
| `SEND_VAR` CV | borrowed | `ZVAL_COPY_DEREF` | +1 (if refcounted); a CV ref adds +1 on the inner value |
| `SEND_VAR` VAR, not a ref | consumed | `ZVAL_COPY_VALUE` (move) | 0 |
| `SEND_VAR` VAR, ref | consumed | copy inner value | -1 ref, +1 inner (unless the ref dies) |
| `SEND_VAL` TMP/VAR | consumed | `ZVAL_COPY_VALUE` (move) | **0** |
| `SEND_VAL` CONST | borrowed literal | `ZVAL_COPY_VALUE` + `UNEXPECTED` addref (Zend/zend_vm_def.h:4880-4885) | normally 0 |
| `SEND_VAL_SIMPLE` CONST (proved non-refcounted) | — | `ZVAL_COPY_VALUE` (Zend/zend_vm_def.h:10435) | 0 |
| `SEND_VAR_SIMPLE` CV | borrowed | `ZVAL_COPY` | +1 |

- `SEND_VAR_NO_REF[_EX]` handles a function-call result going into a by-ref param. If the VAR
  is not a reference, it wraps the value with `ZVAL_NEW_REF(arg, arg)` and emits the notice
  "Only variables should be passed by reference" (Zend/zend_vm_def.h:5004-5013).
- The `_EX` variants exist only because the by-ref-ness of the parameter is unknown at compile
  time. They test `QUICK_ARG_SHOULD_BE_SENT_BY_REF(EX(call)->func, arg_num)` at runtime
  (Zend/zend_vm_def.h:5133-5137). `SPEC(QUICK_ARG)` removes the
  `arg_num <= MAX_ARG_FLAG_NUM` test.
- Who frees what afterwards: the callee owns its CV slots and drops them on return
  (`i_free_compiled_variables`, Zend/zend_execute.c:4271-4280). Between the SEND and the
  `DO_FCALL`, the pushed call frame is covered by the caller's exception cleanup.
  **(unverified in detail: I did not trace `cleanup_unfinished_calls` for this note.)**

---

## 6. Summary: refcount cost per operand kind

These are costs for a *refcounted* value (string, array, object). For scalars every cell is 0.

| Kind | Read only (handler borrows) | Store into result / callee arg / variable | Disposal by the handler |
|---|---|---|---|
| **CONST** | 0. `RT_CONSTANT` pointer into literals; no checks | `ZVAL_COPY_VALUE` + `UNEXPECTED` +1. Effectively 0 for interned strings and other non-refcounted literals; with opcache-proved non-refcounted literals, the `_SIMPLE`/`_NOREF` variants drop even the check | never (`FREE_OP1` = empty) |
| **TMP_VAR** | 0. Raw slot; never a reference | **0: move** (`ZVAL_COPY_VALUE`), and the handler then must not free it | exactly once: `zval_ptr_dtor_nogc` = -1 (`FREE_OP1`), unless moved. On exceptions, the consumer frees it; live ranges cover only def+1..use-1 |
| **VAR** | 0, but may need a deref (`IS_REFERENCE`) or INDIRECT unwrap (W mode) | non-ref: **0 (move)**. Ref: -1 on the reference, +1 on the inner value | -1 (`FREE_OP1` / `FREE_OP1_IF_VAR`) unless moved |
| **CV** | 0, plus an UNDEF check (cold path emits a warning) and an optional deref. `*_UNDEF` getters skip the check when the handler type-tests anyway (ADD) | **+1** (`ZVAL_COPY` / `ZVAL_COPY_DEREF`); the frame keeps its reference | never by the handler; the frame releases it on exit or overwrite |
| **UNUSED (`$this`)** | 0. `&EX(This)`; object check removed at compile time | +1 if stored (e.g. passed as a hook's `This`) | never |

### Takeaways for Ferrophant (inference, not citations)

1. **Zend never copies operands to read them.** Every handler gets `zval*` pointers into the
   literal table or frame slots (§3.2). Refcount traffic happens only when a value is
   *stored* somewhere that will own it (result slot, callee arg slot, variable), or when a
   consumed TMP/VAR is released.
   - In Ferrophant terms: read CONST and CV operands as `&Zval` borrows from the
     literal pool and frame. Read TMP operands by taking ownership out of the slot
     (`mem::take`/`replace` with UNDEF). An `Rc::clone` is needed only at a "store from a
     borrowed source" point.
2. **The kind of the source alone decides between move and clone** (`zend_copy_to_variable`,
   Zend/zend_execute.h:138-163). TMP gives a move, CV gives a clone, CONST gives a clone that
   is free for non-`Rc` payloads. This is static per instruction. Zend resolves it once at
   load time through the handler table (§3.4), so the hot path has no runtime kind branch.
   - The same effect needs no 1021 handlers: an operand-kind tag in the instruction, plus a
     `match` (or monomorphised generic handlers over `K1, K2: OperandKind`), gives the
     borrow/move/clone choice without cloning onto a value stack.
3. **Scalar fast paths are where specialisation pays most.** In `ZEND_ADD` the long/double
   path touches no refcount *for any kind*, including TMP, which skips its free because ints
   are not refcounted. The `TYPE_SPEC` variants (`ADD_LONG_NO_OVERFLOW`, `QM_ASSIGN_NOREF`,
   `SEND_VAR_SIMPLE`) additionally remove type tests, but only when opcache type inference
   proves the types (§2.5).
4. **Exception safety relies on one invariant:** the consumer frees its operands even on
   error, and all other pending temporaries are released by the live-range table.
   - A Rust VM that moves TMPs out of slots gets this almost for free through `Drop`. It
     still has to make sure that slots it has moved out of are left UNDEF and not
     double-dropped.

One addition to §5 not covered above: `ZEND_RETURN` **moves** a CV out of the dying frame
instead of copying it. For a refcounted, non-reference CV, when the call is neither top-level
code nor observed, it does `ZVAL_COPY_VALUE(return_value, retval_ptr)` and then `ZVAL_NULL(retval_ptr)`,
with 0 refcount operations (Zend/zend_vm_def.h:4536-4547).

---

## 7. What this means for Ferrophant: the Drupal census (step 2)

This section is measurement, not citation. Tool: `bench/drupal/opnd.sh` (an `op-census` +
`mem-census` build of `vm/opndcensus.rs`). Data: the warm front page, 2026-10-04.
Full tables are in `bench/results/2026-10-04-operand-census.md`.

How Ferrophant maps onto Zend's kinds:

- **TMP is already a move.** A value produced by one op and consumed by the next stays on the
  frame's operand stack and is popped *by value*. No clone and no extra drop happen: this is
  Zend's `ZVAL_COPY_VALUE` move. The 53k Rc-carrying TMP operands consumed per request cost
  nothing extra.
- **CV and CONST are copied, not borrowed.** `LoadVar`/`LoadSlot` clone the slot onto the
  stack, and `PushConst` bumps the literal's refcount. The consumer then either keeps the copy
  (Zend would `ZVAL_COPY` too) or drops it. Only the *dropped* ones are a clone/drop pair that
  Zend does not pay. `$this` (`Op::This`) behaves the same way.
- Many hot ops already embed their CV/CONST/`$this` operands in the instruction, reading them
  by reference: `ThisPropGet`, `PropSetPop` (receiver), `FieldIsset`/`FieldAssign` (base),
  `AssignPath`/`IssetPath` (base), `CmpJmpSS/SC`, and the `Binary*` slot forms. These are
  Ferrophant's equivalent of a specialised handler.

Measured per request, out of 407k ops dispatched:

| | count |
|---|---:|
| Rc-carrying CV/CONST/THIS operands **dropped** by their consumer (a borrow would remove the clone and the drop) | 46,212 |
| `Ret` of an Rc-carrying CV (Zend moves it, §5 addition) | 3,618 |
| Rc-carrying CONST operands *kept* (free in Zend: interned literals) | ~4,300 |
| **operand-level avoidable pairs** | **~54,000** |
| internal clones in the read-only `isset`/`empty` handlers (Zend's `ISSET_ISEMPTY_*` take no reference; not an operand-kind issue) | ~29,400 |
| every Rc-carrying `Zval::clone` in the request, for scale | 196,552 |

A clone/drop pair of an Rc value costs 1.8–2.4 ns when cache-hot and 4.7 ns when cold
(microbenchmark of `Zval::clone` + drop, same image). That makes the operand-level ceiling
54k × 2–5 ns = **0.11–0.27 ms**. Adding the isset internals gives **≤ 0.42 ms**. Even removing
*every* Rc clone in the request would be only 0.4–1.0 ms.

**Verdict:** step 3 required ≥ ~2 ms of avoidable clone/drop work, and this is at most 0.42 ms,
so the prototype (generic operand-kind handlers) was not built. Zend's advantage is not in
operand ownership, which Ferrophant's stack already gets mostly right. The census points
elsewhere: `CallNsFallback` alone is 20 % of op time, `Ret` 13 %, `FieldIsset` 6 %
(`bench/drupal/optime.sh`).
