# 2026-10-04 — operand-kind census on Drupal (Zend-style specialisation, step 2)

Question (owner, 2026-10-04): would Zend-style operand specialisation (borrow CV/CONST operands,
move TMP operands) remove at least ~2 ms of clone/drop work from a Drupal `handle()`? If not,
no prototype is built (step 3). Background is in `ZEND_VM_NOTES.md`.

**Answer: no.** The avoidable clone/drop work at operand level is at most **0.11–0.27 ms**
per request (≤ 0.42 ms including the isset handlers' internal clones), against the 2 ms bar.
Step 3 was not started.

## Method

- `bench/drupal/opnd.sh build`: an `op-census` + `mem-census` build of `vm/opndcensus.rs`.
  It runs 5 warm front-page requests and keeps the last request's table.
- **Operand kinds.** A shadow of every frame's operand stack is reconciled before each
  dispatch by value identity (tag plus pointer, or the scalar bits):
  - entries that disappeared were consumed by the previous op;
  - new entries were produced by it.

  The producer gives the kind:

  | producer | kind |
  |---|---|
  | `LoadVar`/`LoadSlot` | CV |
  | `PushConst` | CONST |
  | `LoadVarPushConst` | CV then CONST |
  | `This` | THIS |
  | `Dup` | DUP |
  | any other op | TMP |
- **Fate** of an Rc-carrying CV/CONST/THIS operand. The census compares the source's refcount
  after the consumer ran (read through the live slot, literal or `$this`) with its value
  right after the load:
  - lower means the consumer **dropped** its copy, which is a pair a borrow removes;
  - equal or higher means it **kept** it, and Zend would `ZVAL_COPY` there too.

  A `Ret` of a CV is "unknown" because the frame is gone; Zend moves it (zend_vm_def.h:4536-4547).
- **Clones** are `Zval::clone` calls per op, by class (the census `impl Clone`, deltas per
  dispatch). `PushConst`'s `ZStr` bump is not a `Zval::clone`; it shows up as CONST operands.
  **String drops** are every `ZStr` handle drop per op (a counter in `ZStr::drop`, census builds
  only). Array/object/reference drops cannot be hooked without wrapping `Rc`. For those, the
  operand-level drops come from the fate column.
- Two runs gave the same operand and clone counts to within ±0.1 %; total string drops differed by 4 % (222k vs 231k).

## Top 15 ops by time (optime.sh, same day) — operands, clones, drops

The "rc" columns count Rc-carrying values (string, array, object, reference); "sc" columns count scalars.

| op | ms | execs | stack opnds sc | stack opnds rc | of which TMP rc | CV/CONST/THIS rc | dropped | kept | Zval clones sc | Zval clones rc | string drops |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| CallNsFallback | 3.73 | 9,895 | 3,813 | 14,715 | 2,932 | 11,783 | 11,433 | 350 | 1,464 | 9,576 | 31,656 |
| Ret | 2.44 | 17,132 | 9,341 | 7,806 | 3,589 | 4,217 | 1 | 15 | 7,552 | 1,611 | 32,998 |
| Include | 1.86 | 747 | 0 | 747 | 36 | 711 | 4 | 707 | 15,096 | 47 | 8 |
| CallHostBuiltin | 1.54 | 378 | 173 | 418 | 95 | 323 | 307 | 16 | 47 | 415 | 680 |
| FieldIsset | 1.07 | 6,419 | 259 | 6,405 | 1 | 6,404 | 6,377 | 27 | 507 | 21,852 | 19,206 |
| MethodCall | 0.80 | 7,519 | 716 | 15,965 | 4,849 | 11,116 | 44 | 11,035 | 27,229 | 1,913 | 137 |
| PropSetPop | 0.72 | 3,618 | 824 | 6,412 | 1,499 | 4,913 | 3,289 | 1,624 | 1,078 | 3,577 | 7,379 |
| DeclareDeferred | 0.34 | 232 | 0 | 0 | 0 | 0 | 0 | 0 | 464 | 696 | 0 |
| CoerceParams | 0.33 | 4,011 | 0 | 0 | 0 | 0 | 0 | 0 | 631 | 5,431 | 1,618 |
| PushConst | 0.27 | 32,317 | — | — | — | — | — | — | 0 | 0 (bumps) | 0 |
| ThisPropGet | 0.26 | 8,999 | 0 | 0 | 0 | 0 | 0 | 0 | 3,176 | 6,888 | 0 |
| AssignPath | 0.25 | 3,052 | 1,544 | 3,828 | 1,749 | 2,079 | 445 | 1,634 | 483 | 7,153 | 2,112 |
| LoadVar | 0.24 | 46,748 | — | — | — | — | — | — | 8,333 | 38,415 | 0 |
| FieldAssign | 0.24 | 1,327 | 572 | 2,036 | 253 | 1,783 | 120 | 1,663 | 474 | 1,614 | 1,590 |
| StoreSlot | 0.22 | 22,189 | 3,853 | 18,336 | 16,872 | 1,464 | 6 | 1,458 | 0 | 0 | 5,659 |

Reading the table:

- **CallNsFallback** (a namespaced call to a builtin):
  - 11.4k of its CV/CONST arguments are cloned and then dropped after the builtin returns.
  - Zend's `SEND_VAR` also adds a reference to a CV argument for an internal call and releases
    it after the call. So only the CONST share (3.7k) is free in Zend, as interned literals.
  - Its 31.7k string drops are mostly strings created and freed by the builtins themselves.
- **FieldIsset**:
  - Its stack operand is the `isset($this->a[$k])` key (a CV string), cloned and dropped.
  - The 21.9k internal clones are the walk itself copying the property/array values it tests.
    Zend's `ISSET_ISEMPTY_*` handlers take no reference at all. That is a handler-internal
    borrow fix, not operand specialisation.
- **MethodCall** keeps what it receives:
  - CV/CONST arguments move into callee slots (11.0k kept), where Zend also adds a reference.
  - The 27k scalar clones are argument copies into the callee frame.
- **StoreSlot** consumes TMPs by move (16.9k Rc values, zero clones).
- **Ret**: 3.6k Rc CVs are returned by clone plus frame drop, where Zend moves them.
- The **embedded-operand ops** carry their operands in the instruction and read them by
  reference: `ThisPropGet`, `PropSetPop` (receiver), `FieldIsset`/`FieldAssign` (base) and
  `AssignPath`/`IssetPath` (base). Their clone counts are result or stored copies that Zend
  also makes (`FETCH_OBJ_R` `ZVAL_COPY_DEREF`, ZEND_VM_NOTES §5b).

## Operand-kind combinations (top 15 ops, share of executions)

"Embedded" is what the instruction carries; "consumed" lists the stack operands, bottom first.

| op | embedded | top combinations (consumed) |
|---|---|---|
| CallNsFallback | name=CONST | 18.6 % CV:str,CONST:str · 14.6 % CV:str · 11.9 % CV:str,CONST:scalar,CV:scalar · 8.3 % TMP:str · 6.5 % CV:arr |
| Ret | — | 47.0 % CONST:scalar · 8.4 % TMP:obj · 8.1 % CV:str · 7.4 % TMP:str · 6.7 % CV:obj |
| Include | — | 92.4 % CV:str · 4.8 % TMP:str · 2.8 % CONST:str |
| CallHostBuiltin | — | 43.7 % CV:obj · 21.4 % CONST:str · 7.4 % TMP:scalar,TMP:str |
| FieldIsset | base=THIS (100 %) | 91.4 % CV:str · 3.8 % CONST:str,CV:str · 2.5 % TMP:scalar |
| MethodCall | name=CONST | 16.1 % THIS:obj,CV:str · 8.4 % THIS:obj,CV:str,CONST:str · 8.1 % CV:obj · 7.4 % TMP:obj,TMP:ref · 7.1 % TMP:obj |
| PropSetPop | this, name=CONST | 20.7 % THIS:obj,CV:obj · 12.4 % THIS:obj,CV:str · 11.3 % THIS:obj,TMP:arr · 9.9 % THIS:obj,CV:arr · 8.1 % THIS:obj,CONST:scalar |
| DeclareDeferred | — | none (100 %) |
| CoerceParams | — | none (works on the frame's slots) |
| PushConst | src=CONST | none (producer) |
| ThisPropGet | this, name=CONST | none (100 %) |
| AssignPath | base=CV | 21.8 % CV:scalar,TMP:obj · 16.1 % TMP:scalar,CV:str · 14.3 % none · 10.2 % TMP:str,CV:str |
| LoadVar | src=CV | none (producer) |
| FieldAssign | base=THIS | 21.9 % CV:str,CONST:scalar · 20.4 % CV:str,CV:obj · 9.9 % CV:str,CV:scalar,CV:arr · 9.4 % none |
| StoreSlot | dst=CV | 38.8 % TMP:str · 17.6 % TMP:arr · 12.7 % TMP:obj · 6.3 % CONST:scalar · 6.1 % DUP:str |

`PropSetPop` consumes THIS:obj because the compiler pushes `$this` with `Op::This` before it.
That is 2.8k dropped object clones, which a `$this`-embedded form (like `ThisPropGet`) would avoid.

## Totals per request and the estimate

| | count |
|---|---:|
| ops dispatched | 407,053 |
| Rc-carrying stack operands consumed: TMP (already moved) | 52,884 |
| Rc-carrying CV / CONST / THIS operands | 49,076 / 17,334 / 8,934 |
| … dropped by the consumer (borrow removes clone + drop) | 29,468 / 12,666 / 4,078 = **46,212** |
| … kept by the consumer (Zend copies too) | 15,966 / 4,260 / 4,668 |
| `Ret` of an Rc CV (Zend moves) | 3,618 |
| CONST kept (free in Zend: interned) | 4,260 |
| **operand-level avoidable pairs** | **~54,100** |
| isset/empty family internal Rc clones (a borrow inside the handler would avoid them) | 29,423 |
| all Rc-carrying `Zval::clone` | 196,552 |
| all `ZStr` handle drops | 222,363 |

Cost of one pair (`Zval::clone` + drop; a temporary php-types example, best of 5 × 50M, same image):

| value | hot (64 values) | cold (64k strings) |
|---|---:|---:|
| Long | 0.69 ns | — |
| string | 1.79 ns | 4.65 ns |
| array | 2.41 ns | — |

Estimate:

- operand-level: 54.1k × 2–5 ns = **0.11–0.27 ms**;
- plus the isset internals: 83.5k × 2–5 ns = **0.17–0.42 ms**;
- even every Rc clone in the request would be only 0.39–0.98 ms.

The profile agrees: `drop_glue::<Zval>` is 6.2 % self (≈ 1 ms) in total, and most of it is
TMP results, frame teardown and container frees, which no borrowing removes.

Verdict: **below the 2 ms bar by ~5×. Step 3 (monomorphised `Operand` handlers) was not built.**

## handle() this session

There is no engine change: the census code compiles only with the census features.

The phase bench (N=180, ROUNDS=6), from the host:

| run | ferro handle() | php-fpm handle() | ratio |
|---|---:|---:|---:|
| first | 18.31 ms | 3.82 ms | 4.79× |
| repeat | 18.01 ms | 3.90 ms | 4.62× |

Session 12 measured 16.27 vs 3.55 ms (4.58×) with the same engine code. Today's host is slower
for both arms.
