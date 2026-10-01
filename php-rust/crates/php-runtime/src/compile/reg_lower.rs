//! Register-lowering pass — stage 2 v3 "raw registers" of the register
//! bytecode plan. The v1/v2 attempts were abandoned after a post-mortem; the
//! pass was re-armed under the micro-category judge — the earlier negative
//! verdict had been rendered on the diluted WordPress aggregate.
//!
//! v3 design rule (the discriminating experiment after the enum-operand
//! hybrid measured +1.2% consistent on A/B): the run_loop must see ZERO
//! runtime operand dispatch. Every fused shape is its own MONOMORPHIC op
//! with bare u16 indices ([`Op::BinarySS`]/[`Op::BinarySSDst`]/
//! [`Op::BinarySC`]/[`Op::BinarySCDst`]/[`Op::BinaryDst`]/[`Op::CmpJmpSS`]/
//! [`Op::CmpJmpSC`]); the compiler does all the resolution here. Shapes
//! outside this set are NOT rewritten (no stack-lhs source folds, no 1:1
//! CmpJmpConst rename): the stack forms keep their existing monomorphic
//! handlers, so no polymorphism is added anywhere.
//!
//! Fold rules:
//! - sources: `LoadVar` (only when the name const is byte-identical to
//!   `slot_names[slot]`, so the fused handler re-synthesises the exact
//!   "Undefined variable" warning) and `PushConst`; `LoadSlot` (silent,
//!   cold) is never folded.
//! - const is ALWAYS rhs. A written const-lhs folds ONLY for mirrorable
//!   comparisons (Lt↔Gt, Le↔Ge; Eq-family unchanged — compares emit no
//!   coercion diags and no operand-typed errors). Spaceship is not
//!   mirrorable → no fold. **Divergence from the original v3 design**: the
//!   commutative-arith swap (`3 + $x` → `$x + 3`) was DROPPED — an
//!   "Unsupported operand types" TypeError names the operands in ORDER
//!   (`int + array` vs `array + int`), so the swap is observable when the
//!   slot holds a non-numeric at runtime. The corpus never caught it
//!   ("correct by luck of the corpus" ≠ "correct").
//! - dst folds: `Binary, StoreSlot s` and `Binary, Dup, StoreSlot s, Pop`
//!   sink into the `*Dst` forms (net stack/slot/gc_note effect identical —
//!   the only elision is the transient duplicate, which no longer exists
//!   to note).
//!
//! Window guards (plan §3): one source line per window (diagnostic
//! parity), no jump target or exc-region boundary mid-window (the head MAY
//! be a target), folded indices fit u16. Compaction remaps every `Addr`
//! (op stream + exc table); addresses past the original length
//! (`Addr::MAX` jump-threading terminals) are preserved. `max_temps`
//! stays 0.

use crate::bytecode::{Addr, Const, Func, Op};
use crate::hir::{BinOp, UnOp};

/// MODE CONTRACT of `PHPR_REG_LOWER`. VALUE-PARSED spelling, CLOSED list:
///
///   - variable ABSENT     -> `DEFAULT_ON` (a default flip changes ONLY that
///     constant, never this function)
///   - `PHPR_REG_LOWER=1`  -> ON  (explicit opt-in)
///   - `PHPR_REG_LOWER=0`  -> OFF (explicit opt-out; before the contract
///     `=0` TURNED the pass ON — the check was presence-based via
///     `is_some()`, a capital defect)
///   - any other value     -> `DEFAULT_ON` + a warning on stderr that names
///     the grammar (never a silent fallback)
///
/// Read ONCE (OnceLock) and sealed EAGERLY by the mains via
/// `seal_reg_lower_mode()`; the unit-cache key carries the mode, so a unit
/// compiled in one mode can never serve a process running in the other.
pub(crate) fn enabled() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| mode_from_env(std::env::var_os("PHPR_REG_LOWER").as_deref()))
}

/// Process default when `PHPR_REG_LOWER` is ABSENT. FLIPPED to `true` when
/// flag-on was promoted to the default, with every gate green: the 1418-test
/// corpus by NAME with ZERO per-test diff between the two modes, bimodal
/// server parity (extended sentinel), the WordPress pair within
/// pre-registered bands, the test battery in EXPLICIT mode. The opt-out is
/// `PHPR_REG_LOWER=0` (value-parsed grammar above).
pub const DEFAULT_ON: bool = true;

/// The contract grammar, pure and testable without touching the environment.
pub fn mode_from_env(raw: Option<&std::ffi::OsStr>) -> bool {
    let Some(v) = raw else { return DEFAULT_ON };
    match v.to_str() {
        Some("1") => true,
        Some("0") => false,
        _ => {
            eprintln!(
                "ferro: PHPR_REG_LOWER={v:?} is not a valid value (accepted: `1`=on, \
                 `0`=off, unset=default); using the default"
            );
            DEFAULT_ON
        }
    }
}

/// Visit every jump address the op carries. The single authority both the
/// target-collection and the remap phase use — a new `Addr`-bearing variant
/// must be added HERE or the pass corrupts it.
///
/// EXHAUSTIVE match, no wildcard on `Op` — a NEW variant does not compile
/// until it is classified here (same class of guard as the one in
/// vm/liveness.rs). The "no Addr" group is the CLOSED list, verified by hand
/// against the payloads in bytecode.rs (the only `Addr` fields today: the 16
/// arms above).
fn visit_addrs(op: &mut Op, f: &mut impl FnMut(&mut Addr)) {
    match op {
        Op::FillDefault { skip, .. } | Op::StaticGuard { skip, .. } => f(skip),
        Op::CatchMatch { body, .. } => f(body),
        Op::EndFinally { after } => f(after),
        Op::ParkJump(a)
        | Op::Jump(a)
        | Op::JumpIfFalse(a)
        | Op::JumpIfTrue(a)
        | Op::JumpIfNotNull(a)
        | Op::JumpIfNull(a) => f(a),
        Op::CmpJmp { addr, .. }
        | Op::CmpJmpConst { addr, .. }
        | Op::CmpJmpSS { addr, .. }
        | Op::CmpJmpSC { addr, .. }
        | Op::IncDecSlotJmp { addr, .. } => f(addr),
        Op::IterNext { end, .. } | Op::IterNextRef { end, .. } => f(end),
        Op::PropConcatGate { done, .. } => f(done),
        Op::NsShadowGuard { user, .. } => f(user),
        // ---- closed list: variants WITHOUT an Addr (nothing to visit) ----
        Op::PushConst { .. } | Op::Pop { .. } | Op::Dup { .. }
        | Op::LoadSlot { .. } | Op::LoadVar { .. } | Op::PushUndef { .. }
        | Op::StoreSlot { .. } | Op::ConcatAssignSlot { .. } | Op::Swap { .. }
        | Op::LoadGlobal { .. } | Op::StoreGlobal { .. } | Op::IncDecGlobal { .. }
        | Op::LoadSuperglobal { .. } | Op::StoreSuperglobal { .. }
        | Op::IncDecSuperglobal { .. } | Op::FetchDimList { .. }
        | Op::LoadGlobals { .. } | Op::GlobalsDynAssign { .. }
        | Op::CoerceParam { .. } | Op::CoerceParams { .. } | Op::CheckArity { .. }
        | Op::IncDecSlot { .. }
        // Implicit ip+3 skip, no Addr field to remap (it is born at shrink
        // time, AFTER this pass; classified here to satisfy the guard).
        | Op::PropDimGetConst { .. }
        | Op::BindRef { .. } | Op::StaticStore { .. } | Op::StaticAlias { .. }
        | Op::PushRef { .. } | Op::MakeRef { .. } | Op::PushArgPlace { .. }
        | Op::BindRefTo { .. } | Op::BindRefToChecked { .. } | Op::DerefTop { .. }
        | Op::MakeClosure { .. } | Op::MakeFcc { .. } | Op::CallValue { .. }
        | Op::CallNsFallback { .. } | Op::CallValueArgs { .. }
        | Op::CallNsFallbackArgs { .. } | Op::Throw { .. } | Op::Rethrow { .. }
        | Op::ParkReturn { .. } | Op::Binary { .. } | Op::BinaryAdd { .. }
        | Op::Unary { .. } | Op::Cast { .. } | Op::BinarySS { .. }
        | Op::BinarySSDst { .. } | Op::BinarySC { .. } | Op::BinarySCDst { .. }
        | Op::BinaryDst { .. } | Op::BinarySTDst { .. } | Op::BinaryTC { .. }
        | Op::BinarySCSC { .. } | Op::IncDecSlotPop { .. } | Op::PropGetSlot { .. }
        | Op::PropSetPop { .. } | Op::StringifySlot { .. }
        | Op::PropGetSlotRecv { .. } | Op::BinaryTCPropSetPop { .. }
        | Op::BinarySCSCDst { .. } | Op::LoadVarPushConst { .. }
        | Op::ConcatNConst { .. }
        | Op::ConcatN { .. } | Op::Echo { .. }
        | Op::Print { .. } | Op::Stringify { .. } | Op::ArrayInit { .. }
        | Op::ArrayPush { .. } | Op::ArrayInsert { .. }
        | Op::ArrayAppendSpread { .. } | Op::CallArgs { .. } | Op::FetchDim { .. }
        | Op::CoalesceFetchDim { .. } | Op::AssignPath { .. }
        | Op::AssignOpPath { .. } | Op::IncDecPath { .. } | Op::IssetPath { .. }
        | Op::EmptyPath { .. } | Op::UnsetPath { .. } | Op::Call { .. }
        | Op::DeclareFn { .. } | Op::DeclareClass { .. } | Op::DeclareTrait { .. }
        | Op::DeclareDeferred { .. } | Op::NewAnonDeferred { .. }
        | Op::CallBuiltin { .. } | Op::CallBuiltinSpread { .. }
        | Op::CallHostBuiltin { .. } | Op::CallHostBuiltinRef { .. }
        | Op::CallHostBuiltinOut { .. } | Op::CallHostBuiltinScanf { .. }
        | Op::CallArrayMultisort { .. } | Op::ConstFetch { .. }
        | Op::DefineConst { .. } | Op::CallBuiltinRef { .. }
        | Op::CallBuiltinRefSpread { .. } | Op::CallBuiltinRefCell { .. }
        | Op::Ret { .. } | Op::Yield { .. } | Op::YieldFrom { .. }
        | Op::IterInit { .. } | Op::IterInitRef { .. } | Op::IterPop { .. }
        | Op::Alloc { .. } | Op::This { .. } | Op::Clone { .. } | Op::Eval { .. }
        | Op::Include { .. } | Op::PropGet { .. } | Op::ThisPropGet { .. }
        | Op::PropSet { .. } | Op::PropOpSet { .. } | Op::PropIncDec { .. }
        | Op::PropIsset { .. } | Op::PropIssetFetchGate { .. }
        | Op::PropIssetDyn { .. } | Op::LoadVarDyn { .. } | Op::StoreVarDyn { .. }
        | Op::BindGlobalDyn { .. } | Op::ClassConstDynamic { .. }
        | Op::PropGetSilent { .. } | Op::PropGetDynamic { .. }
        | Op::PropGetDynamicSilent { .. } | Op::MatchError { .. }
        | Op::PropUnset { .. } | Op::MethodCall { .. } | Op::ThisMethodCall { .. }
        | Op::MethodCallArgs { .. } | Op::MethodCallDynamic { .. }
        | Op::MethodCallDynamicArgs { .. } | Op::MethodCallNamed { .. }
        | Op::CallNamed { .. } | Op::CallSpread { .. } | Op::InvokeMethod { .. }
        | Op::InstanceOf { .. } | Op::InstanceOfStatic { .. }
        | Op::InstanceOfDynamic { .. } | Op::InstanceOfBuiltin { .. }
        | Op::StaticCall { .. } | Op::HookCall { .. } | Op::ClosureStatic { .. }
        | Op::StaticCallArgs { .. } | Op::StaticCallDynamic { .. }
        | Op::StaticCallDynamicArgs { .. } | Op::StaticCallDynamicMethod { .. }
        | Op::StaticCallTargetDynamicMethod { .. }
        | Op::StaticPropGetDynName { .. } | Op::StaticPropSetDynName { .. }
        | Op::StaticCallDynamicMethodArgs { .. }
        | Op::StaticCallTargetDynamicMethodArgs { .. } | Op::ClassConst { .. }
        | Op::ClassConstDyn { .. } | Op::ClassConstFromValue { .. }
        | Op::EnumCase { .. } | Op::ClassNameStatic { .. }
        | Op::ClassNameScope { .. } | Op::AllocStatic { .. }
        | Op::AllocDynamic { .. } | Op::InvokeCtor { .. }
        | Op::InvokeCtorArgs { .. } | Op::InitProps { .. }
        | Op::StampThrowable { .. } | Op::StaticPropGet { .. }
        | Op::StaticPropSet { .. } | Op::StaticPropRef { .. }
        | Op::StaticPropOpSet { .. } | Op::StaticPropIncDec { .. }
        | Op::StaticPropGetDynamic { .. } | Op::StaticPropSetDynamic { .. }
        | Op::StaticPropOpSetDynamic { .. } | Op::StaticPropIncDecDynamic { .. }
        | Op::FieldAssign { .. } | Op::FieldAssignOp { .. }
        | Op::FieldIncDec { .. } | Op::FieldIsset { .. } | Op::FieldEmpty { .. }
        | Op::FieldUnset { .. } | Op::Fatal { .. } | Op::EmitNotice { .. }
        | Op::Exit { .. } | Op::SuppressBegin { .. } | Op::SuppressEnd { .. }
        | Op::Sweep { .. } | Op::Nop { .. } => {}
    }
}

/// A foldable `LoadVar` source: index fits u16 and the name const equals
/// `slot_names[slot]` (warning parity). `LoadSlot` is silent — never folded.
fn fold_slot(f: &Func, i: usize) -> Option<u16> {
    match &f.ops[i] {
        Op::LoadVar { slot, name } if *slot <= u16::MAX as u32 => {
            match &f.consts[*name as usize] {
                Const::Str(s)
                    if f.slot_names.get(*slot as usize).map(|n| &n[..]) == Some(s.as_bytes()) =>
                {
                    Some(*slot as u16)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// A foldable `PushConst` source (index fits u16).
fn fold_const(f: &Func, i: usize) -> Option<u16> {
    match &f.ops[i] {
        Op::PushConst(c) if *c <= u16::MAX as u32 => Some(*c as u16),
        _ => None,
    }
}

/// Mirror a comparison so its operands can swap sides (const to rhs).
/// Comparisons emit no coercion diagnostics and no operand-typed errors,
/// so the swap is unobservable. `None` = not mirrorable (Spaceship,
/// arithmetic — arithmetic is order-observable through the
/// "Unsupported operand types: <lhs> <op> <rhs>" TypeError).
fn mirror_cmp(b: BinOp) -> Option<BinOp> {
    Some(match b {
        BinOp::Eq | BinOp::NotEq | BinOp::Identical | BinOp::NotIdentical => b,
        BinOp::Lt => BinOp::Gt,
        BinOp::Le => BinOp::Ge,
        BinOp::Gt => BinOp::Lt,
        BinOp::Ge => BinOp::Le,
        _ => return None,
    })
}

/// The pass (called from `compile_body` behind [`enabled`]): scan for
/// fusable windows, rebuild `ops`/`lines`, remap every address.
pub(super) fn lower_func(f: &mut Func) {
    let n = f.ops.len();
    if n == 0 {
        return;
    }
    debug_assert_eq!(f.lines.len(), n, "lines parallel to ops");
    // Positions a window may not absorb: jump targets and exc boundaries.
    let mut blocked = vec![false; n + 1];
    {
        let mut mark = |a: Addr| {
            if (a as usize) <= n {
                blocked[a as usize] = true;
            }
        };
        for r in &f.exc_table {
            mark(r.start);
            mark(r.end);
            mark(r.target);
        }
        for op in &mut f.ops {
            visit_addrs(op, &mut |a| mark(*a));
        }
    }
    let mut new_ops: Vec<Op> = Vec::with_capacity(n);
    let mut new_lines = Vec::with_capacity(n);
    let mut map = vec![0u32; n + 1];
    let mut i = 0usize;
    while i < n {
        // Neg-fold: [PushConst(c), Unary(Neg)] on the same line, i+1 not a
        // target — the const is NEGATED IN THE TABLE (consts-append) and a
        // single PushConst remains. Lives HERE and not in fuse_window (which
        // has &Func: the const table is mutable only in the driver). ONLY
        // Int with checked_neg()=Some (INT_MIN stays unfused: its negation
        // promotes to float through the unary funnel) and Float; the negation
        // replicates apply_unop_ovl at compile time.
        if i + 1 < n && !blocked[i + 1] && f.lines[i + 1] == f.lines[i] {
            if let (Op::PushConst(c), Op::Unary(UnOp::Neg)) = (&f.ops[i], &f.ops[i + 1]) {
                let negated = match f.consts.get(*c as usize) {
                    Some(Const::Int(v)) => v.checked_neg().map(Const::Int),
                    Some(Const::Float(x)) => Some(Const::Float(-x)),
                    _ => None,
                };
                if let Some(k) = negated {
                    let nc = f.consts.len() as u32;
                    f.consts.push(k);
                    for k2 in i..i + 2 {
                        map[k2] = new_ops.len() as u32;
                    }
                    new_lines.push(f.lines[i]);
                    new_ops.push(Op::PushConst(nc));
                    i += 2;
                    continue;
                }
            }
        }
        let (op, w) = fuse_window(f, &blocked, i);
        for k in i..i + w {
            map[k] = new_ops.len() as u32;
        }
        new_lines.push(f.lines[i]);
        new_ops.push(op);
        i += w;
    }
    map[n] = new_ops.len() as u32;
    for op in &mut new_ops {
        visit_addrs(op, &mut |a| {
            if (*a as usize) <= n {
                *a = map[*a as usize];
            }
        });
    }
    for r in &mut f.exc_table {
        for a in [&mut r.start, &mut r.end, &mut r.target] {
            if (*a as usize) <= n {
                *a = map[*a as usize];
            }
        }
    }
    // MEASURED DECISION (12.9 ns/occurrence, above the 1.0 floor; the
    // judge's add.php has 1 residual per iteration): the default flip must
    // not WITHDRAW the `+` specialization from the stack sites the windows
    // do not cover — every `Binary(Add)` that survives the windows becomes
    // the specialized form. Equivalence proven by reg_lower_differential;
    // tripwire: flag-on, the emission NEVER contains a generic
    // `Binary(Add)` (either a fused form or `BinaryAdd`).
    for op in &mut new_ops {
        if matches!(op, Op::Binary(BinOp::Add)) {
            *op = Op::BinaryAdd;
        }
    }
    f.ops = new_ops;
    f.lines = new_lines;
}

/// Source shape of a Binary window, pre-resolved by the scanner.
enum BinKind {
    SS(u16, u16),
    SC(u16, u16),
    /// lhs from `LoadSlot` (silent read), rhs from the stack — the
    /// compound-assign prefix `LoadSlot(l), Swap`.
    /// Fuses ONLY with an assign-and-discard tail: there is no bare
    /// `BinaryST` value form by choice (minimal change, measured).
    ST(u16),
    Stack,
}

/// The binary operator an op carries, seeing through the emission-time
/// `+` specialization (H-B2): `BinaryAdd` IS `Binary(Add)` by construction,
/// so the windows fuse both spellings — the production flag-on pipeline
/// only ever emits `Binary(Add)`, but the test battery (and any future
/// mixed pipeline) compiles with the specialized emission.
///
/// EXHAUSTIVE match — a NEW `Op` variant (in particular a future Binary-like
/// specialization such as `BinarySub`) DOES NOT COMPILE until it is
/// classified here: the ledger of forms the windows fuse cannot decay
/// silently.
fn bin_op_of(op: &Op) -> Option<BinOp> {
    match op {
        Op::Binary(b) => Some(*b),
        Op::BinaryAdd => Some(BinOp::Add),
        // ---- closed list: variants the windows do NOT fuse ----
        // (the already-lowered register forms — BinarySS/SC/Dst/CmpJmpSS/SC —
        // stay out BY CHOICE: the pass does not re-fuse its own output.)
        Op::PropConcatGate { .. } | Op::NsShadowGuard { .. }
        | Op::PushConst { .. } | Op::Pop { .. } | Op::Dup { .. }
        | Op::LoadSlot { .. } | Op::LoadVar { .. } | Op::PushUndef { .. }
        | Op::StoreSlot { .. } | Op::ConcatAssignSlot { .. } | Op::Swap { .. }
        | Op::LoadGlobal { .. } | Op::StoreGlobal { .. } | Op::IncDecGlobal { .. }
        | Op::LoadSuperglobal { .. } | Op::StoreSuperglobal { .. }
        | Op::IncDecSuperglobal { .. } | Op::FetchDimList { .. }
        | Op::LoadGlobals { .. } | Op::GlobalsDynAssign { .. }
        | Op::FillDefault { .. } | Op::CoerceParam { .. } | Op::CoerceParams { .. }
        | Op::CheckArity { .. }
        | Op::IncDecSlot { .. } | Op::BindRef { .. } | Op::StaticGuard { .. }
        | Op::StaticStore { .. } | Op::StaticAlias { .. } | Op::PushRef { .. }
        | Op::MakeRef { .. } | Op::PushArgPlace { .. } | Op::BindRefTo { .. }
        | Op::BindRefToChecked { .. } | Op::DerefTop { .. }
        | Op::MakeClosure { .. } | Op::MakeFcc { .. } | Op::CallValue { .. }
        | Op::CallNsFallback { .. } | Op::CallValueArgs { .. }
        | Op::CallNsFallbackArgs { .. } | Op::Throw { .. } | Op::Rethrow { .. }
        | Op::CatchMatch { .. } | Op::EndFinally { .. } | Op::ParkReturn { .. }
        | Op::ParkJump { .. } | Op::Unary { .. } | Op::Cast { .. }
        | Op::Jump { .. } | Op::JumpIfFalse { .. } | Op::JumpIfTrue { .. }
        | Op::CmpJmp { .. } | Op::CmpJmpConst { .. } | Op::BinarySS { .. }
        | Op::BinarySSDst { .. } | Op::BinarySC { .. } | Op::BinarySCDst { .. }
        // Post-pass fused form; the windows do not fuse it.
        | Op::PropDimGetConst { .. }
        | Op::BinaryDst { .. } | Op::BinarySTDst { .. } | Op::BinaryTC { .. }
        | Op::BinarySCSC { .. } | Op::IncDecSlotPop { .. } | Op::IncDecSlotJmp { .. }
        | Op::PropGetSlot { .. } | Op::PropSetPop { .. } | Op::StringifySlot { .. }
        | Op::PropGetSlotRecv { .. } | Op::BinaryTCPropSetPop { .. }
        | Op::BinarySCSCDst { .. } | Op::LoadVarPushConst { .. }
        | Op::ConcatNConst { .. }
        | Op::CmpJmpSS { .. } | Op::CmpJmpSC { .. }
        | Op::ConcatN { .. } | Op::JumpIfNotNull { .. } | Op::JumpIfNull { .. }
        | Op::Echo { .. } | Op::Print { .. } | Op::Stringify { .. }
        | Op::ArrayInit { .. } | Op::ArrayPush { .. } | Op::ArrayInsert { .. }
        | Op::ArrayAppendSpread { .. } | Op::CallArgs { .. } | Op::FetchDim { .. }
        | Op::CoalesceFetchDim { .. } | Op::AssignPath { .. }
        | Op::AssignOpPath { .. } | Op::IncDecPath { .. } | Op::IssetPath { .. }
        | Op::EmptyPath { .. } | Op::UnsetPath { .. } | Op::Call { .. }
        | Op::DeclareFn { .. } | Op::DeclareClass { .. } | Op::DeclareTrait { .. }
        | Op::DeclareDeferred { .. } | Op::NewAnonDeferred { .. }
        | Op::CallBuiltin { .. } | Op::CallBuiltinSpread { .. }
        | Op::CallHostBuiltin { .. } | Op::CallHostBuiltinRef { .. }
        | Op::CallHostBuiltinOut { .. } | Op::CallHostBuiltinScanf { .. }
        | Op::CallArrayMultisort { .. } | Op::ConstFetch { .. }
        | Op::DefineConst { .. } | Op::CallBuiltinRef { .. }
        | Op::CallBuiltinRefSpread { .. } | Op::CallBuiltinRefCell { .. }
        | Op::Ret { .. } | Op::Yield { .. } | Op::YieldFrom { .. }
        | Op::IterInit { .. } | Op::IterNext { .. } | Op::IterInitRef { .. }
        | Op::IterNextRef { .. } | Op::IterPop { .. } | Op::Alloc { .. }
        | Op::This { .. } | Op::Clone { .. } | Op::Eval { .. }
        | Op::Include { .. } | Op::PropGet { .. } | Op::ThisPropGet { .. }
        | Op::PropSet { .. } | Op::PropOpSet { .. } | Op::PropIncDec { .. }
        | Op::PropIsset { .. } | Op::PropIssetFetchGate { .. }
        | Op::PropIssetDyn { .. } | Op::LoadVarDyn { .. } | Op::StoreVarDyn { .. }
        | Op::BindGlobalDyn { .. } | Op::ClassConstDynamic { .. }
        | Op::PropGetSilent { .. } | Op::PropGetDynamic { .. }
        | Op::PropGetDynamicSilent { .. } | Op::MatchError { .. }
        | Op::PropUnset { .. } | Op::MethodCall { .. } | Op::ThisMethodCall { .. }
        | Op::MethodCallArgs { .. } | Op::MethodCallDynamic { .. }
        | Op::MethodCallDynamicArgs { .. } | Op::MethodCallNamed { .. }
        | Op::CallNamed { .. } | Op::CallSpread { .. } | Op::InvokeMethod { .. }
        | Op::InstanceOf { .. } | Op::InstanceOfStatic { .. }
        | Op::InstanceOfDynamic { .. } | Op::InstanceOfBuiltin { .. }
        | Op::StaticCall { .. } | Op::HookCall { .. } | Op::ClosureStatic { .. }
        | Op::StaticCallArgs { .. } | Op::StaticCallDynamic { .. }
        | Op::StaticCallDynamicArgs { .. } | Op::StaticCallDynamicMethod { .. }
        | Op::StaticCallTargetDynamicMethod { .. }
        | Op::StaticPropGetDynName { .. } | Op::StaticPropSetDynName { .. }
        | Op::StaticCallDynamicMethodArgs { .. }
        | Op::StaticCallTargetDynamicMethodArgs { .. } | Op::ClassConst { .. }
        | Op::ClassConstDyn { .. } | Op::ClassConstFromValue { .. }
        | Op::EnumCase { .. } | Op::ClassNameStatic { .. }
        | Op::ClassNameScope { .. } | Op::AllocStatic { .. }
        | Op::AllocDynamic { .. } | Op::InvokeCtor { .. }
        | Op::InvokeCtorArgs { .. } | Op::InitProps { .. }
        | Op::StampThrowable { .. } | Op::StaticPropGet { .. }
        | Op::StaticPropSet { .. } | Op::StaticPropRef { .. }
        | Op::StaticPropOpSet { .. } | Op::StaticPropIncDec { .. }
        | Op::StaticPropGetDynamic { .. } | Op::StaticPropSetDynamic { .. }
        | Op::StaticPropOpSetDynamic { .. } | Op::StaticPropIncDecDynamic { .. }
        | Op::FieldAssign { .. } | Op::FieldAssignOp { .. }
        | Op::FieldIncDec { .. } | Op::FieldIsset { .. } | Op::FieldEmpty { .. }
        | Op::FieldUnset { .. } | Op::Fatal { .. } | Op::EmitNotice { .. }
        | Op::Exit { .. } | Op::SuppressBegin { .. } | Op::SuppressEnd { .. }
        | Op::Sweep { .. } | Op::Nop { .. } => None,
    }
}

/// Recognise the longest fusable window starting at `i`; `(op, width)` —
/// width 1 with the original op when nothing fuses.
fn fuse_window(f: &Func, blocked: &[bool], i: usize) -> (Op, usize) {
    let n = f.ops.len();
    let line = f.lines[i];
    let free = |j: usize| j < n && !blocked[j] && f.lines[j] == line;

    if let Some(a) = fold_slot(f, i) {
        // W6 (BEFORE the 3-op windows: the longest wins): the tree
        // [LoadVar, PushConst, Binary, LoadVar, PushConst, Binary, Binary]
        // — two slot⊚const subexpressions feeding one Binary
        // (arith census: BinarySC;BinarySC;Binary(Sub), 50M/run). No Dst
        // tail by choice (the judge feeds BinarySTDst).
        if (3..=6).all(|k| free(i + k)) && free(i + 1) && free(i + 2) {
            if let (Some(ca), Some(opa), Some(lb), Some(cb), Some(opb), Some(op)) = (
                fold_const(f, i + 1),
                bin_op_of(&f.ops[i + 2]),
                fold_slot(f, i + 3),
                fold_const(f, i + 4),
                bin_op_of(&f.ops[i + 5]),
                bin_op_of(&f.ops[i + 6]),
            ) {
                // W10: the BinarySTDst tail on the SAME line —
                // [LoadSlot(l), Swap, Binary(opd), tail-assign] right after
                // the tree: the whole statement `$s opd= (…)` in a single op.
                // Without the tail, the W6 window stays identical.
                if (7..=9).all(|k| free(i + k)) {
                    if let (Op::LoadSlot(l), Op::Swap) = (&f.ops[i + 7], &f.ops[i + 8]) {
                        if *l <= u16::MAX as u32 {
                            if let Some(opd) = bin_op_of(&f.ops[i + 9]) {
                                if free(i + 10) {
                                    if let Op::StoreSlot(dst) = &f.ops[i + 10] {
                                        if *dst <= u16::MAX as u32 {
                                            return (
                                                Op::BinarySCSCDst { opa, la: a, ca, opb, lb, cb, op, opd, l: *l as u16, dst: *dst as u16 },
                                                11,
                                            );
                                        }
                                    }
                                    if matches!(f.ops[i + 10], Op::Dup) && free(i + 11) && free(i + 12) {
                                        if let (Op::StoreSlot(dst), Op::Pop) = (&f.ops[i + 11], &f.ops[i + 12]) {
                                            if *dst <= u16::MAX as u32 {
                                                return (
                                                    Op::BinarySCSCDst { opa, la: a, ca, opb, lb, cb, op, opd, l: *l as u16, dst: *dst as u16 },
                                                    13,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                return (
                    Op::BinarySCSC { opa, la: a, ca, opb, lb, cb, op },
                    7,
                );
            }
        }
        if free(i + 1) && free(i + 2) {
            // [LoadVar, LoadVar, Binary|CmpJmp]
            if let Some(b) = fold_slot(f, i + 1) {
                if let Some(op) = bin_op_of(&f.ops[i + 2]) {
                    return bin_dst(f, &free, i, 3, BinKind::SS(a, b), op);
                }
                match &f.ops[i + 2] {
                    Op::CmpJmp { op, addr, when } => {
                        return (
                            Op::CmpJmpSS { op: *op, l: a, r: b, addr: *addr, when: *when },
                            3,
                        )
                    }
                    _ => {}
                }
            }
            // [LoadVar, PushConst, Binary]
            if let Some(c) = fold_const(f, i + 1) {
                if let Some(op) = bin_op_of(&f.ops[i + 2]) {
                    return bin_dst(f, &free, i, 3, BinKind::SC(a, c), op);
                }
            }
        }
        // W3/W8: 2-op windows headed by LoadVar — the fused read keeps
        // warning parity (fold_slot guard + `unit_slot_name` at runtime).
        if free(i + 1) {
            match &f.ops[i + 1] {
                // [LoadVar, PropGet] → PropGetSlot (60M bigram in the prop judge)
                Op::PropGet { name, ic } => {
                    return (
                        Op::PropGetSlot { slot: a, name: name.clone(), ic: ic.clone() },
                        2,
                    )
                }
                // [LoadVar, Stringify] → StringifySlot (str/arr interpolation)
                Op::Stringify => return (Op::StringifySlot { slot: a }, 2),
                _ => {}
            }
        }
        // [LoadVar, CmpJmpConst] → slot vs const compare (mirror const-lhs)
        if free(i + 1) {
            if let Op::CmpJmpConst { op, cidx, addr, when, const_lhs } = &f.ops[i + 1] {
                if *cidx <= u16::MAX as u32 {
                    let op2 = if *const_lhs { mirror_cmp(*op) } else { Some(*op) };
                    if let Some(op2) = op2 {
                        return (
                            Op::CmpJmpSC {
                                op: op2,
                                slot: a,
                                cidx: *cidx as u16,
                                addr: *addr,
                                when: *when,
                            },
                            2,
                        );
                    }
                }
            }
        }
        // W13: [LoadVar, PushConst] — the argument-push pair (calls/arr/re)
        // when NO longer window has won. Non-interference guard: if the op
        // after the const is a Binary on the same line, the pair is LEFT to
        // the W5 arm ([PushConst, Binary] → BinaryTC); if after the const
        // there is a [foldable LoadVar, mirrorable Cmp] on the same line, the
        // const belongs to the MIRROR fold (on that pattern the emission is
        // the earlier one). W13 does however stay BEFORE F2
        // ([PushConst, ConcatN]): on [LoadVar, PushConst, ConcatN] W13 wins as
        // before — F2 fuses only the consts no earlier window absorbed. A
        // batch ADDS fusions; it never changes the emission of earlier
        // batches.
        if free(i + 1) {
            if let Some(c) = fold_const(f, i + 1) {
                let w5 = free(i + 2) && bin_op_of(&f.ops[i + 2]).is_some();
                let mirror = free(i + 2)
                    && free(i + 3)
                    && fold_slot(f, i + 2).is_some()
                    && bin_op_of(&f.ops[i + 3]).and_then(mirror_cmp).is_some();
                if !w5 && !mirror {
                    return (Op::LoadVarPushConst { slot: a, cidx: c }, 2);
                }
            }
        }
    }
    // [PushConst, LoadVar, Binary] — const written first: fold only a
    // mirrorable COMPARISON (order-free by construction). The original v3
    // commutative-arith swap is deliberately absent (see module doc).
    if let Some(c) = fold_const(f, i) {
        if free(i + 1) && free(i + 2) {
            if let Some(a) = fold_slot(f, i + 1) {
                if let Some(op) = bin_op_of(&f.ops[i + 2]) {
                    if let Some(m) = mirror_cmp(op) {
                        return bin_dst(f, &free, i, 3, BinKind::SC(a, c), m);
                    }
                }
            }
        }
        // F2: [PushConst(Str), ConcatN] — the literal part of the join goes
        // into the concatenation op (shared helper concat_n). Guard: ONLY
        // Const::Str (the all-Str fast path of ConcatN stays monomorphic);
        // ConcatN is pure by construction — no suspendable helper inside a
        // window (a standing constraint).
        if free(i + 1) {
            if let Op::ConcatN(nn) = &f.ops[i + 1] {
                if matches!(f.consts.get(c as usize), Some(Const::Str(_))) {
                    return (Op::ConcatNConst { n: *nn, cidx: c }, 2);
                }
            }
        }
        // W5: ADJACENT [PushConst, Binary] — lhs on the STACK, const ALWAYS
        // rhs by construction (the PushConst immediately precedes the Binary
        // ⇒ the const is the top). No swap, no order divergence possible
        // (prop bigram: PushConst(1);BinaryAdd, 30M).
        if free(i + 1) {
            if let Some(op) = bin_op_of(&f.ops[i + 1]) {
                // W9b: the [PropSet, Pop] tail on the same line — the whole
                // `$o->p = <stack> op const;` (flat BinaryTC funnel, then the
                // PropSet DISCARD entry as the last step).
                if free(i + 2) && free(i + 3) {
                    if let (Op::PropSet { name, ic }, Op::Pop) = (&f.ops[i + 2], &f.ops[i + 3]) {
                        return (
                            Op::BinaryTCPropSetPop { op, cidx: c, name: name.clone(), ic: ic.clone() },
                            4,
                        );
                    }
                }
                return (Op::BinaryTC { op, cidx: c }, 2);
            }
        }
    }
    // [LoadSlot, Swap, Binary] — RMW on a slot: the lhs-from-slot prefix of
    // the compound assign `$s <op>= expr`. A DECLARED AMENDMENT of the v3
    // rule "LoadSlot never folded": that rule assumed LoadSlot was COLD and
    // the arith judge's dump refutes it (it is the compound-assign read in
    // the hot body). The read is SILENT by contract — no warning to
    // re-synthesise ⇒ the fold is diagnostic-safe. Fuses ONLY with an
    // assign-and-discard tail (BinKind::ST without a tail does not fuse: no
    // value form).
    if let Op::LoadSlot(l) = &f.ops[i] {
        if *l <= u16::MAX as u32 && free(i + 1) && free(i + 2) {
            if matches!(f.ops[i + 1], Op::Swap) {
                if let Some(op) = bin_op_of(&f.ops[i + 2]) {
                    return bin_dst(f, &free, i, 3, BinKind::ST(*l as u16), op);
                }
            }
            // W9a: [LoadSlot(recv), LoadVar, PropGet] — the RMW head of the
            // prop judge (`$o->c = $o->c …`): silent push of the receiver +
            // the whole W3 window. The hook/__get suspension stays in the
            // LAST helper of the handler.
            if let Some(slot) = fold_slot(f, i + 1) {
                if let Op::PropGet { name, ic } = &f.ops[i + 2] {
                    return (
                        Op::PropGetSlotRecv { recv: *l as u16, slot, name: name.clone(), ic: ic.clone() },
                        3,
                    );
                }
            }
        }
    }
    // W1: [IncDecSlot, Pop] (+ back-edge Jump) — the pushed value is
    // DISCARDED, so pre/post collapse into the fused form and the elided
    // gc_note is a no-op (the transient is always a scalar: ++/-- on an
    // array/object is a TypeError before any push). The trigram with the
    // Jump is the hot form in ALL six judges.
    if let Op::IncDecSlot { slot, inc, pre: _ } = &f.ops[i] {
        if *slot <= u16::MAX as u32 && free(i + 1) && matches!(f.ops[i + 1], Op::Pop) {
            if free(i + 2) {
                if let Op::Jump(a) = &f.ops[i + 2] {
                    return (
                        Op::IncDecSlotJmp { slot: *slot as u16, inc: *inc, addr: *a },
                        3,
                    );
                }
            }
            return (Op::IncDecSlotPop { slot: *slot as u16, inc: *inc }, 2);
        }
    }
    // W2: [Dup, StoreSlot s, Pop] ≡ [StoreSlot s] — no new op: the
    // transient duplicate is elided (same precedent as the *Dst tails; the
    // Pop's gc_note fell on the transient clone, which no longer exists).
    // Catches the assign-and-discard tails behind NON-fusable producers
    // (Call, CallBuiltin, the callee's Ret — calls/str census).
    if matches!(f.ops[i], Op::Dup) && free(i + 1) && free(i + 2) {
        if let (Op::StoreSlot(s), Op::Pop) = (&f.ops[i + 1], &f.ops[i + 2]) {
            return (Op::StoreSlot(*s), 3);
        }
    }
    // W4: [PropSet, Pop] → PropSetPop (statement `$o->p = v;`).
    if let Op::PropSet { name, ic } = &f.ops[i] {
        if free(i + 1) && matches!(f.ops[i + 1], Op::Pop) {
            return (Op::PropSetPop { name: name.clone(), ic: ic.clone() }, 2);
        }
    }
    // Bare Binary: wins only with an assign-and-discard tail.
    if let Some(op) = bin_op_of(&f.ops[i]) {
        return bin_dst(f, &free, i, 1, BinKind::Stack, op);
    }
    (f.ops[i].clone(), 1)
}

/// Extend a Binary window over an assign-and-discard tail (`StoreSlot s` or
/// `Dup, StoreSlot s, Pop`) and emit the matching monomorphic variant. With
/// no tail: `SS`/`SC` push, a bare stack Binary stays as it is (nothing to
/// win).
fn bin_dst(
    f: &Func,
    free: &dyn Fn(usize) -> bool,
    i: usize,
    w: usize,
    kind: BinKind,
    op: BinOp,
) -> (Op, usize) {
    let j = i + w;
    let tail: Option<(u16, usize)> = if free(j) {
        match &f.ops[j] {
            Op::StoreSlot(s) if *s <= u16::MAX as u32 => Some((*s as u16, 1)),
            Op::Dup if free(j + 1) && free(j + 2) => {
                match (&f.ops[j + 1], &f.ops[j + 2]) {
                    (Op::StoreSlot(s), Op::Pop) if *s <= u16::MAX as u32 => {
                        Some((*s as u16, 3))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    } else {
        None
    };
    match (kind, tail) {
        (BinKind::SS(l, r), Some((dst, e))) => (Op::BinarySSDst { op, l, r, dst }, w + e),
        (BinKind::SS(l, r), None) => (Op::BinarySS { op, l, r }, w),
        (BinKind::SC(slot, cidx), Some((dst, e))) => {
            (Op::BinarySCDst { op, slot, cidx, dst }, w + e)
        }
        (BinKind::SC(slot, cidx), None) => (Op::BinarySC { op, slot, cidx }, w),
        // Without a tail the ST window does NOT fuse (no value form).
        (BinKind::ST(l), Some((dst, e))) => (Op::BinarySTDst { op, l, dst }, w + e),
        (BinKind::ST(_), None) => (f.ops[i].clone(), 1),
        (BinKind::Stack, Some((dst, e))) => (Op::BinaryDst { op, dst }, w + e),
        (BinKind::Stack, None) => (f.ops[i].clone(), 1),
    }
}

/// Whether `PHPR_DUMP_OPS` is set: dump every compiled unit's bytecode to
/// stderr. Compile-time-only diagnostic for this arc: diff a flag-off dump
/// against a flag-on dump to prove a stage's rewrite is a no-op (stage 1) or
/// inspect exactly what it rewrote (stage 2+).
fn dump_enabled() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("PHPR_DUMP_OPS").is_some())
}

/// Dump a compiled unit's bytecode to stderr (gated on `PHPR_DUMP_OPS`).
/// Scope: main, functions, closures, class
/// methods, property-HOOK bodies — every body that passes through the
/// `compile_body` funnel and that the lowering pass therefore rewrites
/// flag-on — plus the prop-init thunks (hand-built, NEVER lowered: declared
/// OUT of the pass; dumped anyway so the production truth is visible).
/// Reflection/const/attribute thunks stay out: hand-built via `FnCompiler`
/// (`compile_const_thunk`, attribute thunks), the pass cannot touch them.
/// Hook order is sorted by property name — `prop_info` is a HashMap and the
/// dump feeds diff-based gates, so iteration order must be deterministic.
pub(super) fn dump_module_ops(m: &crate::bytecode::Module) {
    if !dump_enabled() {
        return;
    }
    let err = std::io::stderr();
    let mut w = err.lock();
    dump_module_to(&mut w, m);
}

/// The dump body, writer-parametric so the coverage test can
/// assert the scope against the Module without touching process stderr.
pub(super) fn dump_module_to(w: &mut impl std::io::Write, m: &crate::bytecode::Module) {
    let _ = writeln!(w, "== unit {} ==", String::from_utf8_lossy(&m.file));
    dump_func(w, "{main}", &m.main);
    for f in &m.functions {
        dump_func(w, &format!("fn {}", String::from_utf8_lossy(&f.name)), f);
    }
    for (i, f) in m.closures.iter().enumerate() {
        dump_func(w, &format!("closure#{i}"), f);
    }
    for c in &m.classes {
        let cname = String::from_utf8_lossy(&c.name);
        for meth in &c.methods {
            let label = format!("{cname}::{}", String::from_utf8_lossy(&meth.name));
            dump_func(w, &label, &meth.func);
        }
        if let Some(pi) = &c.prop_init {
            dump_func(w, &format!("{cname}::{{prop-init}}"), pi);
        }
        let mut hooked: Vec<(&[u8], &crate::bytecode::PropHooks)> = c
            .prop_info
            .iter()
            .filter_map(|(n, i)| i.hooks.as_ref().map(|h| (&n[..], h)))
            .collect();
        hooked.sort_by(|a, b| a.0.cmp(b.0));
        for (pname, h) in hooked {
            let p = String::from_utf8_lossy(pname);
            if let Some(g) = &h.get {
                dump_func(w, &format!("{cname}::${p}::get"), g);
            }
            if let Some(s) = &h.set {
                dump_func(w, &format!("{cname}::${p}::set"), s);
            }
        }
    }
}

fn dump_func(w: &mut impl std::io::Write, label: &str, f: &Func) {
    let _ = writeln!(w, "-- {label} n_slots={} max_temps={} --", f.n_slots, f.max_temps);
    for (i, op) in f.ops.iter().enumerate() {
        let _ = writeln!(w, "{i:04} {op:?}");
    }
    for (i, c) in f.consts.iter().enumerate() {
        let _ = writeln!(w, "cst{i:03} {c:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::Registry;
    use crate::bytecode::Module;

    /// Compile in an EXPLICIT mode: the emission battery does not depend on
    /// the process environment — `compile` = mode OFF, `compile_on` = the
    /// REAL funnel with the pass on (hooks included, prop_init/thunks
    /// excluded by construction: compile_body decides).
    fn compile_mode(src: &[u8], reg_lower: bool) -> Module {
        let program = crate::lower_source(b"t.php", src).expect("lowers");
        crate::compile::compile_program_with_mode(&program, &Registry::default(), reg_lower)
            .expect("compiles")
    }

    fn compile(src: &[u8]) -> Module {
        compile_mode(src, false)
    }

    fn compile_on(src: &[u8]) -> Module {
        compile_mode(src, true)
    }

    // The old `lowered()` helper (a HAND-WRITTEN mirror of the funnel, and a
    // source of drift) was removed — the flag-on arm of the tests is now the
    // REAL funnel via `compile_on` (`compile_program_with_mode`).

    /// Enumerate EVERY body of the Module by EXHAUSTIVE DESTRUCTURING — a new
    /// field of `Module` (or of `CompiledClass`/`PropHooks` below) that
    /// carried bodies DOES NOT COMPILE until it is classified here (same
    /// class of guard as the one in vm/liveness.rs). Classification:
    /// - IN the `compile_body` funnel (the pass rewrites them flag-on): main,
    ///   functions, closures, methods, hook get/set.
    /// - OUT (hand-built, the pass NEVER sees them): prop_init, const-thunk
    ///   (`consts[].func`), attribute-thunk (new/args), enum-case.
    /// - No compiled bodies in the Module: the fields bound to `_` below
    ///   (hir not yet compiled, indices, metadata).
    fn all_funcs(m: &Module) -> Vec<&Func> {
        let Module {
            main,
            functions,
            conditional_fns: _,
            fn_ci: _,
            conditional_classes: _,
            // uncompiled hir: the bodies are born at Declare, via compile_body
            conditional_traits: _,
            // re-lowered at runtime: goes through compile_body at its execution point
            deferred: _,
            closures,
            classes,
            file: _,
            class_index: _,
            static_count: _,
            strict: _,
            // attribute-thunk: hand-built FnCompiler, OUTSIDE the funnel
            const_attributes: _,
            elided: _,
        } = m;
        let mut all: Vec<&Func> = vec![main];
        all.extend(functions.iter().map(|f| f.as_ref()));
        all.extend(closures.iter());
        for c in classes {
            all.extend(c.methods.iter().map(|meth| &meth.func));
            for info in c.prop_info.values() {
                if let Some(crate::bytecode::PropHooks { get, set, backed: _ }) = &info.hooks {
                    all.extend(get.iter());
                    all.extend(set.iter());
                }
            }
            // OUTSIDE the funnel but real bodies: included anyway — the
            // flag-off battery asserts "no register form ANYWHERE", and for
            // the out-of-funnel bodies the absence must hold in BOTH modes.
            all.extend(c.prop_init.iter());
            all.extend(c.consts.iter().map(|k| &k.func));
        }
        all
    }

    /// The fixture's class (the Module includes the prelude: looked up by NAME).
    fn zoo_class(m: &Module) -> &crate::bytecode::CompiledClass {
        m.classes
            .iter()
            .find(|c| &*c.name == b"C")
            .expect("fixture: classe C nel Module")
    }

    /// The fixture with ONE body per kind: function, closure, method,
    /// hook get/set, non-constant prop-init, class const.
    const BODY_ZOO: &[u8] = br#"<?php
function g($a){ $s=0; for($i=0;$i<9;$i++){ $s = $s + $i*3; } return $s+$a; }
$h = function($x){ return $x+1; };
class C {
  const K = 1;
  public $d = self::K + 1;
  public int $v = 0;
  public int $p { get { $s=0; for($i=0;$i<9;$i++){ $s = $s + $i*3; } return $s; } set { $this->v = $value * 2; } }
  public function m($a,$b){ $c=$a+$b; if($c>3){$c=$c*2;} return $c; }
}
echo g(1), ($h)(2), C::K;"#;

    /// The dump covers every body of the `compile_body` funnel (hooks
    /// INCLUDED — a former blind spot) plus the prop-init declared outside
    /// it. The list of bodies comes from the Module (all_funcs, exhaustive
    /// destructuring), not from a hand-written list.
    #[test]
    fn dump_scope_covers_every_funnel_body() {
        let m = compile(BODY_ZOO);
        assert!(zoo_class(&m).prop_init.is_some(), "fixture: il prop-init deve esistere");
        let mut buf = Vec::new();
        dump_module_to(&mut buf, &m);
        let out = String::from_utf8(buf).expect("dump utf8");
        for h in [
            "-- {main} ",
            "-- fn g ",
            "-- closure#0 ",
            "-- C::m ",
            "-- C::$p::get ",
            "-- C::$p::set ",
            "-- C::{prop-init} ",
        ] {
            assert!(out.contains(h), "dump privo del corpo `{h}`:\n{out}");
        }
    }

    /// Hook bodies GO THROUGH `compile_body`, the pass rewrites them
    /// flag-on — helper and dump must see it. Positive control: the get-hook
    /// with a foldable loop shows register forms after the pass, and the
    /// dump's hook chunk shows them too.
    #[test]
    fn hooks_are_lowered_and_visible_in_the_dump() {
        let l = compile_on(BODY_ZOO);
        let hooks = zoo_class(&l)
            .prop_info
            .get(&b"p"[..])
            .and_then(|i| i.hooks.as_ref())
            .expect("fixture: hook su $p");
        let get = hooks.get.as_ref().expect("fixture: get hook");
        assert!(
            get.ops.iter().any(is_reg_form),
            "il pass non riscrive il corpo del get-hook: RC-1 di nuovo cieco\n{:?}",
            get.ops
        );
        // And the prop-init stays NOT lowered in the helper too (outside the pass).
        let pi = zoo_class(&l).prop_init.as_ref().expect("prop-init");
        assert!(
            !pi.ops.iter().any(is_reg_form),
            "il funnel flag-on abbassa prop_init che la produzione non abbassa MAI (RC-2)"
        );
        let mut buf = Vec::new();
        dump_module_to(&mut buf, &l);
        let out = String::from_utf8(buf).expect("dump utf8");
        let chunk = out
            .split("-- C::$p::get ")
            .nth(1)
            .and_then(|r| r.split("\n-- ").next())
            .expect("chunk del get-hook nel dump");
        assert!(
            chunk.contains("BinarySC") || chunk.contains("BinarySS") || chunk.contains("CmpJmpSC"),
            "nessuna forma registro nel chunk hook del dump:\n{chunk}"
        );
    }

    /// Run and return the CLI-faithful stream (diagnostics inline) — the
    /// parity comparison must cover the warning text/order too.
    fn run(m: &Module) -> Vec<u8> {
        let reg = Registry::default();
        let out = crate::vm::run_module(m, &reg);
        assert!(out.fatal.is_none(), "unexpected fatal: {:?}", out.fatal);
        out.rendered
    }

    fn is_reg_form(o: &Op) -> bool {
        matches!(
            o,
            Op::BinarySS { .. }
                | Op::BinarySSDst { .. }
                | Op::BinarySC { .. }
                | Op::BinarySCDst { .. }
                | Op::BinaryDst { .. }
                | Op::CmpJmpSS { .. }
                | Op::CmpJmpSC { .. }
                // The flag-off guard also covers the newer forms — the OFF
                // compilation NEVER emits one.
                | Op::BinarySTDst { .. }
                | Op::BinaryTC { .. }
                | Op::BinarySCSC { .. }
                | Op::IncDecSlotPop { .. }
                | Op::IncDecSlotJmp { .. }
                | Op::PropGetSlot { .. }
                | Op::PropSetPop { .. }
                | Op::StringifySlot { .. }
                // Second batch: the flag-off guard covers these too.
                | Op::PropGetSlotRecv { .. }
                | Op::BinaryTCPropSetPop { .. }
                | Op::BinarySCSCDst { .. }
                | Op::LoadVarPushConst { .. }
                // Third batch: and this one.
                | Op::ConcatNConst { .. }
        )
    }

    /// v3 shape: hot windows become the specialized monomorphic forms and
    /// no fused compare window survives un-rewritten; no register temps.
    #[test]
    fn stage2v3_rewrites_hot_windows() {
        let src = br#"<?php
            function f($a, $b) {
                $c = $a + $b;
                if ($a > $b) { $c = $c * 2; }
                if ($a == 3) { return -1; }
                if (3 < $b) { $c = $c + 1; }
                return $c . "s";
            }
            echo f(1, 2), f(4, 2), f(3, 0), f(1, 7);
            "#;
        let m = compile(src);
        let lm = compile_on(src);
        let lf = lm
            .functions
            .iter()
            .find(|f| f.name.as_ref() == b"f")
            .expect("fn f present")
            .as_ref();
        let has = |p: &dyn Fn(&Op) -> bool| lf.ops.iter().any(|o| p(o));
        assert!(has(&|o| matches!(o, Op::BinarySSDst { .. })), "$c=$a+$b: {:#?}", lf.ops);
        assert!(has(&|o| matches!(o, Op::CmpJmpSS { .. })), "$a>$b");
        assert!(has(&|o| matches!(o, Op::CmpJmpSC { .. })), "$a==3 / 3<$b (mirrored)");
        assert!(
            has(&|o| matches!(o, Op::BinarySCDst { .. })),
            "$c*2 / $c+1 (const fold with dst)"
        );
        // No compare window with a foldable LoadVar in front may survive.
        for (x, y) in lf.ops.iter().zip(lf.ops.iter().skip(1)) {
            assert!(
                !(matches!(x, Op::LoadVar { .. })
                    && matches!(y, Op::CmpJmpConst { .. } | Op::CmpJmp { .. })),
                "unfused compare window"
            );
        }
        for f in all_funcs(&lm) {
            assert_eq!(f.max_temps, 0, "v3 emits no temps");
        }
        assert_eq!(run(&m), run(&lm));
    }

    /// A compare whose lhs comes from the stack (no foldable producer) keeps
    /// the monomorphic WP-34 CmpJmpConst — no-elision rewrites are the
    /// measured v1 regression.
    #[test]
    fn stage2v3_stack_lhs_compare_keeps_cmpjmpconst() {
        let src = br#"<?php
            function g($a) { return $a + 1; }
            function h($a) { if (g($a) == 3) { return 1; } return 2; }
            echo h(2), h(5);
            "#;
        let m = compile(src);
        let lm = compile_on(src);
        let lh = lm
            .functions
            .iter()
            .find(|f| f.name.as_ref() == b"h")
            .expect("fn h present");
        assert!(
            lh.ops.iter().any(|o| matches!(o, Op::CmpJmpConst { .. })),
            "stack-lhs compare must stay CmpJmpConst: {:#?}",
            lh.ops
        );
        assert!(
            !lh.ops.iter().any(|o| matches!(o, Op::CmpJmpSS { .. } | Op::CmpJmpSC { .. })),
            "no fold available in h"
        );
        assert_eq!(run(&m), run(&lm));
    }

    /// A const-FIRST arithmetic window must NOT fold — the
    /// "Unsupported operand types" TypeError names its operands in order,
    /// so `3 + $x` and `$x + 3` are distinguishable when `$x` is an array.
    /// (This is the original v3 commutative swap, dropped on soundness.)
    #[test]
    fn stage2v3_const_first_arith_does_not_fold() {
        let src = br#"<?php $a=5; $b = 3 + $a; $c = 3 * $a; echo $b, ",", $c;"#;
        let m = compile(src);
        let lm = compile_on(src);
        assert!(
            !lm.main.ops.iter().any(|o| matches!(
                o,
                Op::BinarySC { .. } | Op::BinarySCDst { .. }
            )),
            "const-first arith must stay on the stack: {:#?}",
            lm.main.ops
        );
        assert_eq!(run(&m), run(&lm));
    }

    /// Guard with a pre-registered INTENDED emission: on the stream
    /// [LoadVar, PushConst, LoadVar, mirrorable Cmp] the const belongs to
    /// the MIRROR fold — W13 YIELDS (extended guard), the head LoadVar stays
    /// bare and the emission is the first-batch one (mirrored BinarySC).
    /// Before the guard, W13 stole the PushConst (a de-optimisation with an
    /// identical value).
    #[test]
    fn w13_cede_al_fold_specchio() {
        let src = br#"<?php
            function f($x, $y) { return $x + ($y ? 1 : 0); }
            $a=1; $b=2; echo f($a, 3 < $b);
            "#;
        let m = compile(src);
        let lm = compile_on(src);
        assert!(
            lm.main.ops.iter().any(|o| matches!(o, Op::BinarySC { .. })),
            "fold specchio atteso (BinarySC dal const-lhs): {:#?}",
            lm.main.ops
        );
        assert!(
            !lm.main.ops.iter().any(|o| matches!(o, Op::LoadVarPushConst { .. })),
            "W13 non deve rubare il PushConst del fold specchio: {:#?}",
            lm.main.ops
        );
        assert_eq!(run(&m), run(&lm));
    }

    /// Third batch: the str body `$s = substr($s . "abc", -30)` emits
    /// ConcatNConst (F2) and the NEGATED PushConst without a Unary (F1);
    /// the OFF mode stays pure stack.
    #[test]
    fn lotto3_negfold_and_concatnconst() {
        // Same shape as the str judge but on a USER function (the battery's
        // run() does not register the host builtins): the Neg's PushConst
        // follows ConcatN — out of W13's reach, F1 fuses. (A Neg whose const
        // follows a foldable LoadVar is absorbed by W13 FIRST: identical
        // value, the Neg stays at runtime — a declared limitation.)
        let src = br#"<?php
            function keep($a, $b) { return $a . $b; }
            $s=''; for($i=0;$i<3;$i++){ $s = keep($s . "ab", -7); } echo $s;
            "#;
        let m = compile(src);
        let lm = compile_on(src);
        assert!(
            lm.main.ops.iter().any(|o| matches!(o, Op::ConcatNConst { .. })),
            "F2 atteso (ConcatNConst): {:#?}",
            lm.main.ops
        );
        assert!(
            !lm.main.ops.iter().any(|o| matches!(o, Op::Unary(UnOp::Neg))),
            "F1 atteso (Neg-fold via consts-append, niente Unary): {:#?}",
            lm.main.ops
        );
        // INT_MIN does not fuse: the negation promotes to float in the funnel.
        let edge = br#"<?php echo -9223372036854775808;"#;
        let me = compile(edge);
        let lme = compile_on(edge);
        assert_eq!(run(&me), run(&lme));
        assert_eq!(run(&m), run(&lm));
    }

    /// v3 parity battery: control flow that emits `Addr`s (loops, if/else,
    /// try/catch/finally, foreach by value and by ref, static guard, param
    /// defaults, ?? / ?:), plus the diagnostic paths the folds must preserve
    /// (undefined-variable warning through slot_names, DivisionByZeroError
    /// at the fused op, references, self-assign, const-first mirror and
    /// const-first arith NON-fold, numeric-string coercions) — lowered
    /// output must equal stack output, and every remapped address must land
    /// inside the function.
    #[test]
    fn stage2v3_behavioral_parity_and_remap() {
        let snippets: &[&[u8]] = &[
            br#"<?php $s=0; for ($i=0; $i<10; $i++) { $s = $s + $i; } echo $s;"#,
            br#"<?php $i=0; while ($i < 5) { $i = $i + 1; if ($i == 3) continue; echo $i; } echo "|", $i;"#,
            br#"<?php $a=2; $b=3; try { echo $a % ($b - 3); } catch (\DivisionByZeroError $e) { echo "dbz"; } finally { echo "-f"; }"#,
            br#"<?php function g($x = 5) { static $n = 0; $n = $n + 1; return $x + $n; } echo g(), g(1), g();"#,
            br#"<?php $t = ['a'=>1,'b'=>2]; $s=''; foreach ($t as $k=>$v) { $s = $s . $k . ($v + 1); } echo $s;"#,
            br#"<?php $arr=[1,2,3]; foreach ($arr as &$v) { $v = $v * 2; } unset($v); echo $arr[0], $arr[1], $arr[2];"#,
            br#"<?php echo $u + 1; $q = $u2 . "x"; echo $q;"#,
            br#"<?php $a=1; $b=&$a; $c = $b + 1; $b = $b + 10; echo $a, ",", $c;"#,
            br#"<?php $a=1; $a = $a + 1; $a = 41 + $a > 42 ? $a * 10 : $a - 1; echo $a;"#,
            br#"<?php $x=null; $y = $x ?? 7; $z = $x ?: 9; echo $y, $z, 5 <=> 3, "10" == "1e1" ? "t" : "f";"#,
            br#"<?php $s="ab"; $n=2; if ($s == "ab" && $n > 1) { echo "y"; } if (3 == $n + 1) { echo "z"; }"#,
            br#"<?php $a=5; $b = 3 + $a; $c = 3 - $a; echo $b, ",", $c; if (3 < $a) { echo "m"; } if (3 <= $a) { echo "e"; } if ("x" == $a) { echo "s"; } else { echo "n"; }"#,
            br#"<?php $w = "7"; echo 3 + $w, 3 * $w, 10 - $w, "3" . $w; if (10 > $w) echo "g";"#,
        ];
        for src in snippets {
            let m = compile(src);
            let lm = compile_on(src);
            for (f, orig) in all_funcs(&lm).into_iter().zip(all_funcs(&m)) {
                let (new_n, old_n) = (f.ops.len(), orig.ops.len());
                let check = |a: Addr| {
                    assert!(
                        (a as usize) <= new_n || (a as usize) > old_n,
                        "addr {a} out of range (new {new_n}, old {old_n}) in {:?}",
                        f.name
                    );
                };
                let mut ops = f.ops.clone();
                for op in &mut ops {
                    visit_addrs(op, &mut |a| check(*a));
                }
                for r in &f.exc_table {
                    check(r.start);
                    check(r.end);
                    check(r.target);
                }
            }
            assert_eq!(
                run(&m),
                run(&lm),
                "lowered output diverges for {}",
                String::from_utf8_lossy(src)
            );
        }
    }

    /// The register forms must not widen the Op enum (every ops Vec pays a
    /// wider element — D-cache). Pinned so a future field addition trips
    /// this consciously.
    #[test]
    fn stage2v3_op_size_unchanged() {
        assert_eq!(std::mem::size_of::<Op>(), 48, "Op must not widen");
    }

    /// Dual-mode guard: in (EXPLICIT) OFF mode — the battery no longer has
    /// environmental premises, the mode is a parameter — the compilation
    /// NEVER emits a register form nor a BinaryAdd from the extension, and
    /// the frame contract is unchanged.
    #[test]
    fn stage2v3_flag_off_emits_no_register_forms() {
        let m = compile(br#"<?php function f($a,$b){ $c=$a+$b; if($c>3){$c=$c*2;} return $c; } echo f(1,2);"#);
        for f in all_funcs(&m) {
            assert_eq!(f.max_temps, 0);
            assert!(
                !f.ops.iter().any(is_reg_form),
                "flag-off compile must stay stack-based"
            );
        }
    }

    /// The PRODUCTION entry (`compile_program`) follows the PROCESS mode
    /// (`enabled()`, value-parsed contract): the test holds in WHATEVER
    /// mode the battery runs — no same-mode false green, and a default flip
    /// inverts no premise.
    #[test]
    fn production_entry_follows_process_mode() {
        let src = br#"<?php $s=0; for($i=0;$i<9;$i++){ $s = $s + $i*3; } echo $s;"#;
        let program = crate::lower_source(b"t.php", src).expect("lowers");
        let m = crate::compile::compile_program(&program, &Registry::default()).expect("compiles");
        let has_reg = m.main.ops.iter().any(is_reg_form);
        assert_eq!(
            has_reg,
            enabled(),
            "compile_program non segue il modo di processo del contratto \
             (enabled()={}, forme registro nel main={})",
            enabled(),
            has_reg
        );
    }

    /// The mode contract: value-parsed grammar, closed list, tested PURE
    /// (no environment touched). The `=0` case is the trap that motivated
    /// the contract: under `is_some()` it turned the pass ON.
    #[test]
    fn mode_contract_grammar_is_value_parsed() {
        use std::ffi::OsStr;
        assert_eq!(mode_from_env(None), DEFAULT_ON, "assente => default nominato");
        assert!(mode_from_env(Some(OsStr::new("1"))), "`=1` => ON");
        assert!(!mode_from_env(Some(OsStr::new("0"))), "`=0` => OFF, MAI presence");
        // Outside the grammar: default (with a stderr warning, not assertable here).
        assert_eq!(mode_from_env(Some(OsStr::new(""))), DEFAULT_ON);
        assert_eq!(mode_from_env(Some(OsStr::new("on"))), DEFAULT_ON);
        assert_eq!(mode_from_env(Some(OsStr::new("true"))), DEFAULT_ON);
    }

    /// Post-flip the named default is ON: whoever inverts it again must
    /// declare it HERE and re-derive the guards and the launcher — the OFF
    /// arm stays exercised on every rotation via an explicit
    /// `PHPR_REG_LOWER=0`.
    #[test]
    fn mode_contract_default_is_on_post_flip() {
        assert!(DEFAULT_ON, "default ri-invertito: ri-derivare denti anti-putenv, launcher e batteria PRIMA di spedire");
    }

    /// The in-process OFF arm emits the PRODUCTION emission even for the
    /// RESIDUAL STACK add — the site that `emit_binary` used to decide with
    /// the global `enabled()` instead of `ctx.reg_lower` (fixed in b618e3a).
    /// Under OFF the site is a direct `BinaryAdd` and no generic
    /// `Binary(Add)` exists; under ON, same source, the generic form
    /// disappears (zero-`Binary(Add)` tripwire).
    #[test]
    fn in_process_off_arm_emits_production_stack_add() {
        use crate::hir::BinOp;
        // `g($a+$b+$c)`: the first add leaves its result on the stack
        // (argument) — the residual class that the `+` extension rewrites
        // ONLY flag-on.
        let src = br#"<?php function g($x){ return $x; } $a=1;$b=2;$c=3; echo g($a+$b+$c);"#;
        let count = |m: &Module, pred: &dyn Fn(&Op) -> bool| -> usize {
            all_funcs(m)
                .iter()
                .map(|f| f.ops.iter().filter(|o| pred(o)).count())
                .sum()
        };
        // Production OFF: emit_binary emits `BinaryAdd` DIRECTLY when
        // `!ctx.reg_lower` — the old bug (reading `enabled()` with the
        // default ON) dropped the site into the generic `Binary(Add)` arm.
        // The guard pins the TRUE polarity.
        let off = compile(src);
        assert!(
            count(&off, &|o| matches!(o, Op::BinaryAdd)) >= 1,
            "OFF: l'add di pila e' BinaryAdd di produzione (H-B2 flag-off)"
        );
        assert_eq!(
            count(&off, &|o| matches!(o, Op::Binary(BinOp::Add))),
            0,
            "OFF: nessun Binary(Add) generico (il sintomo del bug emit_binary)"
        );
        let on = compile_on(src);
        assert_eq!(
            count(&on, &|o| matches!(o, Op::Binary(BinOp::Add))),
            0,
            "ON: tripwire zero-Binary(Add) (ogni add e' forma fusa o BinaryAdd)"
        );
    }

    /// BODY_ZOO — the zero-`Binary(Add)` tripwire extended to the bodies
    /// OUTSIDE the funnel. `ctx.reg_lower` is ONE field for the whole
    /// Module, but the register pass only visits the bodies of the
    /// `compile_body` funnel: prop_init (hand-built) NEVER sees it.
    /// EXPECTATION WRITTEN BEFOREHAND (from reading emit_binary):
    /// - OFF: `!ctx.reg_lower` ⇒ emit_binary emits a direct `BinaryAdd`
    ///   EVERYWHERE, prop_init included ⇒ zero `Binary(Add)` in the whole
    ///   module.
    /// - ON: emit_binary emits a generic `Binary(Add)` relying on the pass;
    ///   in prop_init the pass does not run ⇒ the generic form SURVIVES. The
    ///   "zero-Binary(Add) whole module" invariant under ON is FALSE outside
    ///   the funnel: the residual is pinned BY NAME (prop_init carve-out),
    ///   not hidden. Semantics unchanged (the generic form is correct, just
    ///   not specialized).
    #[test]
    fn body_zoo_off_funnel_add_polarity() {
        use crate::hir::BinOp;
        let src = br#"<?php
            class Z {
                const K = 5;
                public $p = self::K + 1;
            }
            $z = new Z(); echo $z->p;"#;
        let count_in = |fs: &[&Func], pred: &dyn Fn(&Op) -> bool| -> usize {
            fs.iter().map(|f| f.ops.iter().filter(|o| pred(o)).count()).sum()
        };
        let off = compile(src);
        assert_eq!(
            count_in(&all_funcs(&off), &|o| matches!(o, Op::Binary(BinOp::Add))),
            0,
            "OFF: nessun Binary(Add) generico in NESSUN corpo (prop_init compreso)"
        );
        let on = compile_on(src);
        // Carve-out BY NAME: the generic residual under ON lives ONLY in
        // prop_init (outside the funnel). The funnel bodies stay at zero.
        let funnel: Vec<&Func> = {
            let mut v: Vec<&Func> = vec![&on.main];
            v.extend(on.functions.iter().map(|f| f.as_ref()));
            v.extend(on.closures.iter());
            for c in &on.classes {
                v.extend(c.methods.iter().map(|m| &m.func));
            }
            v
        };
        assert_eq!(
            count_in(&funnel, &|o| matches!(o, Op::Binary(BinOp::Add))),
            0,
            "ON: zero Binary(Add) nei corpi DEL funnel"
        );
        let prop_inits: Vec<&Func> =
            on.classes.iter().filter_map(|c| c.prop_init.as_ref()).collect();
        assert!(
            count_in(&prop_inits, &|o| matches!(o, Op::Binary(BinOp::Add))) >= 1,
            "ON: il residuo fuori-funnel esiste ed e' PINNATO qui (prop_init, RC-2); \
             se questo assert diventa rosso il pass ha iniziato a visitare \
             prop_init — aggiornare il carve-out PER NOME"
        );
    }

    /// Rewritten after review: the old "same emission" half was `f(x)==f(x)`
    /// — it compared `compile_mode(src, a)` with `compile_mode(src, b)`
    /// AFTER asserting `a == b`: it pinned the compiler's DETERMINISM, not
    /// "absent ≡ =1" (fabricated coverage: a residual environmental site
    /// would colour both arms the same way). Only the real content of the
    /// guard remains here: the grammar line. The TRUE absent↔`=1` pair lives
    /// in a SUBPROCESS with a dump diff: php-cli/tests/absent_eq_one.rs.
    #[test]
    fn absent_env_resolves_like_explicit_one() {
        use std::ffi::OsStr;
        assert_eq!(
            mode_from_env(None),
            mode_from_env(Some(OsStr::new("1"))),
            "assente e `=1` devono risolvere lo stesso modo (grammatica value-parsed)"
        );
    }
}
