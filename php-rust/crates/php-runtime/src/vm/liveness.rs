//! Phase F1 — per-slot LAST-USE analysis, MEASUREMENT ONLY.
//!
//! Answers a single question for every `LoadSlot`/`LoadVar` of every compiled
//! function: *after this read, is there a path that reads the slot again before
//! it is overwritten?* If not, the read is a last use and the value COULD be
//! moved (`TakeSlot`) instead of cloned. Nothing is moved here: the result
//! only feeds the `would_take*` counters of [`super::zvalcensus`].
//!
//! F1 SCOPE (intentional, not an oversight): this is the BARE dataflow
//! analysis. The ways PHP observes a slot outside the linear flow —
//! `compact()`/`extract()`/`get_defined_vars()`, `$$x`, `eval`/`include`,
//! by-ref closures, generators, shared `Zval::Ref`s, destructor order — are
//! the phase-F2 RENOUNCE predicates: the difference between the F1 and F2
//! counts is "what caution costs".
//!
//! Direction of model errors (documented because it is a choice):
//! - an unmodelled WRITE (e.g. `extract`, `StoreVarDyn`) makes something dead
//!   look live → UNDER-counts movable reads (conservative, accepted);
//! - an unmodelled READ over-counts: the only reads outside the model are
//!   exactly those of the F2 scope (listed by name above);
//! - `*Global(s)` ops touch the slots of `frames[0]`: counting them as a USE
//!   of local slot `s` is correct in main and spurious (but under-counting,
//!   hence harmless) inside a function.
//!
//! Same convention as `zvalcensus`: compiled ONLY behind `zval-census`, so
//! not a single bit of the parity binary changes.

use crate::bytecode::{DimBase, FieldBase, Func, Op};

/// Result of the analysis of one function: `movable[i]` is true iff `ops[i]` is
/// a `LoadSlot`/`LoadVar` whose read is a last use (F1, no F2 renounces);
/// `movable_safe[i]` additionally applies the CONSERVATIVE F2 scope.
pub struct Analysis {
    pub movable: Vec<bool>,
    /// F2: movable AND outside every renounce predicate (see [`renounce`]).
    pub movable_safe: Vec<bool>,
    /// Total static `LoadSlot`/`LoadVar` sites in the function.
    pub sites_total: u64,
    /// How many of those sites are marked movable.
    pub sites_movable: u64,
    /// How many stay movable under the F2 scope.
    pub sites_safe: u64,
}

/// Set of slots as a word bitset. PHP functions have tens of slots, not
/// thousands: simplicity beats sophistication in a measurement build.
#[derive(Clone, PartialEq)]
struct Bits(Vec<u64>);

impl Bits {
    fn new(nbits: usize) -> Self {
        Bits(vec![0; nbits.div_ceil(64)])
    }
    fn set(&mut self, i: u32) {
        self.0[(i / 64) as usize] |= 1 << (i % 64);
    }
    fn clear(&mut self, i: u32) {
        self.0[(i / 64) as usize] &= !(1 << (i % 64));
    }
    /// Out of range = false (F2 bitsets may be narrower than the analysis
    /// ones: never index out of bounds in a measurement build).
    fn get(&self, i: u32) -> bool {
        self.0.get((i / 64) as usize).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }
    fn set_all(&mut self) {
        for w in &mut self.0 {
            *w = !0;
        }
    }
    fn or_assign(&mut self, o: &Bits) {
        for (a, b) in self.0.iter_mut().zip(&o.0) {
            *a |= *b;
        }
    }
}

/// Effect of an op on the slot dataflow. `edges` are the NON-fall-through
/// successors; `fall` says whether `ip+1` is a successor. Each edge carries its
/// own PER-EDGE def set (e.g. `IterNext` defines `value`/`key` only on the
/// branch that enters the body, not on the exit branch).
struct Effect {
    uses: Vec<u32>,
    defs: Vec<u32>,
    uses_all: bool,
    fall: bool,
    /// Defs that hold only on the fall-through edge (IterNext/IterNextRef).
    fall_defs: Vec<u32>,
    /// (target, per-edge defs)
    edges: Vec<(usize, Vec<u32>)>,
}

fn dim_base_use(b: &DimBase, uses: &mut Vec<u32>) {
    match b {
        DimBase::Local(s) | DimBase::Global(s) => uses.push(*s),
        DimBase::Superglobal(_) => {}
    }
}

fn field_base_use(b: &FieldBase, uses: &mut Vec<u32>) {
    match b {
        FieldBase::Local(s) | FieldBase::Global(s) => uses.push(*s),
        FieldBase::Superglobal(_) | FieldBase::This => {}
    }
}

/// Classifies `op`. `park_targets` are ALL the `ParkJump` targets of the
/// function: an `EndFinally` may resume any of those parked jumps, so it gets
/// all of them as successors (conservative).
fn effect(op: &Op, park_targets: &[usize]) -> Effect {
    let mut e = Effect {
        uses: Vec::new(),
        defs: Vec::new(),
        uses_all: false,
        fall: true,
        fall_defs: Vec::new(),
        edges: Vec::new(),
    };
    match op {
        // ----- slot reads (the two optimization targets + the pure readers) -----
        Op::LoadSlot(s) => e.uses.push(*s),
        Op::LoadVar { slot, .. } => e.uses.push(*slot),
        // Register forms: they read their operands by index (use) and the
        // *Dst forms overwrite `dst` entirely (def, same classification as
        // StoreSlot). The CmpJmp* forms carry an edge.
        Op::BinarySS { l, r, .. } => {
            e.uses.push(*l as u32);
            e.uses.push(*r as u32);
        }
        Op::BinarySSDst { l, r, dst, .. } => {
            e.uses.push(*l as u32);
            e.uses.push(*r as u32);
            e.defs.push(*dst as u32);
        }
        Op::BinarySC { slot, .. } => e.uses.push(*slot as u32),
        Op::BinarySCDst { slot, dst, .. } => {
            e.uses.push(*slot as u32);
            e.defs.push(*dst as u32);
        }
        Op::BinaryDst { dst, .. } => e.defs.push(*dst as u32),
        // Superinstructions: reads by index (use); the IncDec* pair is a
        // read+write of the same slot; IncDecSlotJmp is an unconditional
        // Jump (fall=false, edge to addr).
        Op::BinarySCSC { la, lb, .. } => {
            e.uses.push(*la as u32);
            e.uses.push(*lb as u32);
        }
        // Amended classification: the zval-census build had stopped compiling
        // because BinarySTDst/BinaryTC/PropSetPop were never classified here
        // (the exhaustiveness check only bites when the feature is compiled).
        Op::BinarySTDst { l, dst, .. } => {
            e.uses.push(*l as u32);
            e.defs.push(*dst as u32);
        }
        Op::BinarySCSCDst { la, lb, l, dst, .. } => {
            e.uses.push(*la as u32);
            e.uses.push(*lb as u32);
            e.uses.push(*l as u32);
            e.defs.push(*dst as u32);
        }
        Op::PropGetSlotRecv { recv, slot, .. } => {
            e.uses.push(*recv as u32);
            e.uses.push(*slot as u32);
        }
        Op::LoadVarPushConst { slot, .. } => e.uses.push(*slot as u32),
        Op::PropGetSlot { slot, .. } | Op::StringifySlot { slot } => {
            e.uses.push(*slot as u32)
        }
        // Reads the slot; the implicit `ip+3` jump cannot be expressed as an
        // absolute edge here (effect does not see the pc) — conservative:
        // uses_all, the tool does not speculate further (measurement only).
        Op::PropDimGetConst { slot, .. } => {
            e.uses.push(*slot as u32);
            e.uses_all = true;
        }
        Op::IncDecSlotPop { slot, .. } => {
            e.uses.push(*slot as u32);
            e.defs.push(*slot as u32);
        }
        Op::IncDecSlotJmp { slot, addr, .. } => {
            e.uses.push(*slot as u32);
            e.defs.push(*slot as u32);
            e.fall = false;
            e.edges.push((*addr as usize, Vec::new()));
        }
        Op::CmpJmpSS { l, r, addr, .. } => {
            e.uses.push(*l as u32);
            e.uses.push(*r as u32);
            e.edges.push((*addr as usize, Vec::new()));
        }
        Op::CmpJmpSC { slot, addr, .. } => {
            e.uses.push(*slot as u32);
            e.edges.push((*addr as usize, Vec::new()));
        }
        Op::MatchError(s) => {
            e.uses.push(*s);
            e.fall = false;
        }
        Op::IterInitRef(s) => e.uses.push(*s),
        Op::PushRef(s) => e.uses.push(*s),
        Op::MakeClosure { captures, .. } => {
            for c in captures.iter() {
                e.uses.push(c.src);
            }
        }

        // ----- pure writes -----
        Op::StoreSlot(s) => e.defs.push(*s),
        Op::StaticAlias { slot, .. } => e.defs.push(*slot),
        Op::CallHostBuiltinOut { out_slot, out_slot2, .. } => {
            e.defs.extend(out_slot.iter().copied());
            e.defs.extend(out_slot2.iter().copied());
        }
        Op::CallHostBuiltinScanf { out_slots, .. } => {
            e.defs.extend(out_slots.iter().flatten().copied());
        }
        // Hint prologue: reads and rewrites every parameter slot.
        Op::CoerceParams { n } => {
            e.uses.extend(0..*n);
            e.defs.extend(0..*n);
        }

        // ----- read+write on the same slot -----
        Op::ConcatAssignSlot(s) | Op::IncDecSlot { slot: s, .. } | Op::CoerceParam { slot: s, .. } => {
            e.uses.push(*s);
            e.defs.push(*s);
        }
        Op::CallBuiltinRef { slot, .. }
        | Op::CallBuiltinRefSpread { slot, .. }
        | Op::CallHostBuiltinRef { slot, .. } => {
            e.uses.push(*slot);
            e.defs.push(*slot);
        }
        Op::CallArrayMultisort { arg_slots, .. } => {
            for s in arg_slots.iter().flatten() {
                e.uses.push(*s);
                e.defs.push(*s);
            }
        }

        // ----- alias between two named places -----
        Op::BindRef { target, source } => {
            dim_base_use(source, &mut e.uses);
            match target {
                DimBase::Local(t) => e.defs.push(*t),
                DimBase::Global(t) => e.uses.push(*t),
                DimBase::Superglobal(_) => {}
            }
        }
        Op::BindRefTo { base, steps } | Op::BindRefToChecked { base, steps } => {
            // Base with no steps: overwritten entirely (def). With steps: we
            // navigate INSIDE the container held by the slot (use).
            if steps.is_empty() {
                match base {
                    FieldBase::Local(t) => e.defs.push(*t),
                    FieldBase::Global(t) => e.uses.push(*t),
                    FieldBase::Superglobal(_) | FieldBase::This => {}
                }
            } else {
                field_base_use(base, &mut e.uses);
            }
        }

        // ----- ops that go INSIDE the container of a slot (use, never def) -----
        Op::MakeRef { base, .. } | Op::PushArgPlace { base, .. } => field_base_use(base, &mut e.uses),
        Op::FieldAssign { base, .. }
        | Op::FieldAssignOp { base, .. }
        | Op::FieldIncDec { base, .. }
        | Op::FieldIsset { base, .. }
        | Op::FieldEmpty { base, .. }
        | Op::FieldUnset { base, .. } => field_base_use(base, &mut e.uses),
        Op::AssignPath { base, .. } | Op::AssignOpPath { base, .. } | Op::IncDecPath { base, .. } => {
            dim_base_use(base, &mut e.uses)
        }
        Op::IssetPath { base, .. } | Op::EmptyPath { base, .. } => dim_base_use(base, &mut e.uses),
        Op::UnsetPath { base, nkeys } => {
            if *nkeys == 0 {
                match base {
                    DimBase::Local(s) => e.defs.push(*s),
                    DimBase::Global(s) => e.uses.push(*s),
                    DimBase::Superglobal(_) => {}
                }
            } else {
                dim_base_use(base, &mut e.uses);
            }
        }

        // ----- $GLOBALS by slot: in main they are the current slots (use) -----
        Op::LoadGlobal(s) | Op::IncDecGlobal { slot: s, .. } => e.uses.push(*s),
        // Writing a global must NOT kill the same-named local slot inside a
        // function (that would be an over-count): use, never def.
        Op::StoreGlobal(s) => e.uses.push(*s),
        // Dynamic name / snapshot of the whole scope: every slot is potentially read.
        Op::LoadGlobals | Op::GlobalsDynAssign | Op::BindGlobalDyn => e.uses_all = true,

        // ----- control flow -----
        Op::Jump(a) => {
            e.fall = false;
            e.edges.push((*a as usize, Vec::new()));
        }
        Op::JumpIfFalse(a)
        | Op::JumpIfTrue(a)
        | Op::JumpIfNotNull(a)
        | Op::JumpIfNull(a)
        | Op::CmpJmp { addr: a, .. }
        | Op::CmpJmpConst { addr: a, .. }
        | Op::ParkJump(a) => {
            // ParkJump by itself falls through (the jump happens at the
            // EndFinally), but giving it the edge here too is only conservative.
            e.edges.push((*a as usize, Vec::new()));
        }
        Op::FillDefault { slot, skip } => {
            e.uses.push(*slot);
            e.edges.push((*skip as usize, Vec::new()));
        }
        // Stack-only conditional jump (hit → `done`, miss → fall through).
        Op::PropConcatGate { done, .. } => {
            e.edges.push((*done as usize, Vec::new()));
        }
        // Stack-neutral conditional jump (shadow declared → `user`).
        Op::NsShadowGuard { user, .. } => {
            e.edges.push((*user as usize, Vec::new()));
        }
        Op::StaticGuard { skip, .. } => e.edges.push((*skip as usize, Vec::new())),
        Op::CatchMatch { var, body, .. } => {
            // `var` is defined only on the edge taken into the catch body.
            let edge_defs: Vec<u32> = var.iter().copied().collect();
            e.edges.push((*body as usize, edge_defs));
        }
        Op::IterNext { value, key, end } | Op::IterNextRef { value, key, end } => {
            // value/key are defined only when entering the body (fall-through);
            // on the exit edge the slot keeps its last value (PHP's documented
            // gotcha), so NO def there.
            e.fall_defs.push(*value);
            e.fall_defs.extend(key.iter().copied());
            e.edges.push((*end as usize, Vec::new()));
        }
        Op::EndFinally { after } => {
            e.edges.push((*after as usize, Vec::new()));
            for t in park_targets {
                e.edges.push((*t, Vec::new()));
            }
        }

        // ----- terminators -----
        Op::Ret | Op::Throw | Op::Rethrow | Op::Exit { .. } | Op::Fatal(_) => e.fall = false,

        // ----- no effect on slots BY INDEX, fall-through -----
        // The list is EXPLICIT and the match EXHAUSTIVE. The old `_ => {}`
        // was a soundness hole waiting to happen: any NEW `Op` variant that
        // read or wrote a slot would have been silently classified as "no
        // effect", and the header invariant was guarded by nothing. Now a
        // new variant does NOT COMPILE until someone classifies it by hand —
        // silent decay becomes noisy.
        //
        // One-off audit of today's list: `CallBuiltinRefCell` carries NO
        // slot — the by-ref cell sits on the stack, produced by `MakeRef`,
        // whose base is already marked in `renounce()`;
        // `NewAnonDeferred`/`DeclareDeferred` re-read the caller's locals by
        // NAME and not by index, so their home is the WHOLE-function
        // renounce; `Yield`/`YieldFrom`, `Eval`/`Include`,
        // `LoadVarDyn`/`StoreVarDyn` are here because their effect is on the
        // whole function, not on a named slot, and also lives in
        // `renounce()`.
        Op::Alloc { .. }
        | Op::AllocDynamic { .. }
        | Op::AllocStatic { .. }
        | Op::ArrayAppendSpread { .. }
        | Op::ArrayInit { .. }
        | Op::ArrayInsert { .. }
        | Op::ArrayPush { .. }
        | Op::Binary { .. }
        // `BinaryAdd` is the pure-stack form of `Binary(Add)`: same effects
        // (no slot by index).
        | Op::BinaryAdd
        // Pure-stack forms (const inlined, no slot by index).
        | Op::BinaryTC { .. }
        | Op::BinaryTCPropSetPop { .. }
        | Op::Call { .. }
        | Op::CallArgs { .. }
        | Op::CallBuiltin { .. }
        | Op::CallBuiltinRefCell { .. }
        | Op::CallBuiltinSpread { .. }
        | Op::CallHostBuiltin { .. }
        | Op::CallNamed { .. }
        | Op::CallNsFallback { .. }
        | Op::CallNsFallbackArgs { .. }
        | Op::CallSpread { .. }
        | Op::CallValue { .. }
        | Op::CallValueArgs { .. }
        | Op::Cast { .. }
        | Op::CheckArity { .. }
        | Op::ClassConst { .. }
        | Op::ClassConstDyn { .. }
        | Op::ClassConstDynamic { .. }
        | Op::ClassConstFromValue { .. }
        | Op::ClassNameScope { .. }
        | Op::ClassNameStatic { .. }
        | Op::Clone { .. }
        | Op::ClosureStatic { .. }
        | Op::CoalesceFetchDim { .. }
        | Op::ConcatN { .. }
        // Like ConcatN — stack only, no slot effect.
        | Op::ConcatNConst { .. }
        | Op::ConstFetch { .. }
        | Op::DeclareClass { .. }
        | Op::DeclareDeferred { .. }
        | Op::DeclareFn { .. }
        | Op::DeclareTrait { .. }
        | Op::DefineConst { .. }
        | Op::DerefTop { .. }
        | Op::Dup { .. }
        | Op::Echo { .. }
        | Op::EmitNotice { .. }
        | Op::EnumCase { .. }
        | Op::Eval { .. }
        | Op::FetchDim { .. }
        | Op::FetchDimList { .. }
        | Op::HookCall { .. }
        | Op::IncDecSuperglobal { .. }
        | Op::Include { .. }
        | Op::InitProps { .. }
        | Op::InstanceOf { .. }
        | Op::InstanceOfBuiltin { .. }
        | Op::InstanceOfDynamic { .. }
        | Op::InstanceOfStatic { .. }
        | Op::InvokeCtor { .. }
        | Op::InvokeCtorArgs { .. }
        | Op::InvokeMethod { .. }
        | Op::IterInit { .. }
        | Op::IterPop { .. }
        | Op::LoadSuperglobal { .. }
        | Op::LoadVarDyn { .. }
        | Op::MakeFcc { .. }
        | Op::MethodCall { .. }
        | Op::MethodCallArgs { .. }
        | Op::MethodCallDynamic { .. }
        | Op::MethodCallDynamicArgs { .. }
        | Op::MethodCallNamed { .. }
        | Op::NewAnonDeferred { .. }
        | Op::Nop { .. }
        | Op::ParkReturn { .. }
        | Op::Pop { .. }
        | Op::Print { .. }
        | Op::PropGet { .. }
        | Op::PropGetDynamic { .. }
        | Op::PropGetDynamicSilent { .. }
        | Op::PropGetSilent { .. }
        | Op::PropIncDec { .. }
        | Op::PropIsset { .. }
        | Op::PropIssetDyn { .. }
        | Op::PropIssetFetchGate { .. }
        | Op::PropOpSet { .. }
        | Op::PropSet { .. }
        | Op::PropSetPop { .. }
        | Op::PropUnset { .. }
        | Op::PushConst { .. }
        | Op::PushUndef { .. }
        | Op::StampThrowable { .. }
        | Op::StaticCall { .. }
        | Op::StaticCallArgs { .. }
        | Op::StaticCallDynamic { .. }
        | Op::StaticCallDynamicArgs { .. }
        | Op::StaticCallDynamicMethod { .. }
        | Op::StaticCallDynamicMethodArgs { .. }
        | Op::StaticCallTargetDynamicMethod { .. }
        | Op::StaticCallTargetDynamicMethodArgs { .. }
        | Op::StaticPropGet { .. }
        | Op::StaticPropGetDynName { .. }
        | Op::StaticPropGetDynamic { .. }
        | Op::StaticPropIncDec { .. }
        | Op::StaticPropIncDecDynamic { .. }
        | Op::StaticPropOpSet { .. }
        | Op::StaticPropOpSetDynamic { .. }
        | Op::StaticPropRef { .. }
        | Op::StaticPropSet { .. }
        | Op::StaticPropSetDynName { .. }
        | Op::StaticPropSetDynamic { .. }
        | Op::StaticStore { .. }
        | Op::StoreSuperglobal { .. }
        | Op::StoreVarDyn { .. }
        | Op::Stringify { .. }
        | Op::SuppressBegin { .. }
        | Op::SuppressEnd { .. }
        | Op::Swap { .. }
        | Op::Sweep { .. }
        | Op::This { .. }
        | Op::ThisMethodCall { .. }
        | Op::ThisPropGet { .. }
        | Op::Unary { .. }
        | Op::Yield { .. }
        | Op::YieldFrom { .. } => {}
    }
    e
}

/// Builtins that OBSERVE the caller's scope by name: their presence renounces
/// the whole function (the F2 list).
fn observes_scope(name: &[u8]) -> bool {
    // `debug_zval_refcount` was missing even though it belongs to the F2
    // list — it observes a value's refcount, which is exactly what moving
    // changes. `debug_zval_dump` for the same reason.
    const NAMES: [&[u8]; 9] = [
        b"compact",
        b"extract",
        b"get_defined_vars",
        b"debug_backtrace",
        b"debug_print_backtrace",
        b"debug_zval_refcount",
        b"debug_zval_dump",
        b"func_get_args",
        b"func_get_arg",
    ];
    NAMES.iter().any(|n| name.eq_ignore_ascii_case(n))
}

/// F2 — the conservative scope. Returns `(whole_function_renounced,
/// renounced_slots)`: the first fires on the ways PHP sees ALL the locals
/// outside the flow (eval/include, `$$x`, `compact` & co., generators — the
/// state survives suspension); the second on the individual slots that can
/// become SHARED (`Zval::Ref`): `&$x`, `global $x`, `static $x`, closure
/// `use (&$x)`, `foreach ... as &$v`, by-ref parameters — moving a value out
/// of a shared slot would be observable elsewhere.
fn renounce(func: &Func) -> (bool, Bits) {
    let mut nbits = (func.n_slots + func.max_temps) as usize;
    // The same width defenses as `analyze`.
    for op in &func.ops {
        if let Op::LoadSlot(s) | Op::LoadVar { slot: s, .. } = op {
            nbits = nbits.max(*s as usize + 1);
        }
    }
    let mut slots = Bits::new(nbits.max(func.param_by_ref.len()));
    let mut whole = func.is_generator;
    let mut mark = |s: u32, b: &mut Bits| {
        if (s as usize) < nbits.max(func.param_by_ref.len()) {
            b.set(s);
        }
    };
    for (i, by_ref) in func.param_by_ref.iter().enumerate() {
        if *by_ref {
            mark(i as u32, &mut slots);
        }
    }
    for op in &func.ops {
        match op {
            Op::Eval | Op::Include { .. } => whole = true,
            Op::LoadVarDyn | Op::StoreVarDyn | Op::BindGlobalDyn | Op::GlobalsDynAssign | Op::LoadGlobals => {
                whole = true
            }
            // The constructor arguments of `NewAnonDeferred` are RE-EVALUATED
            // "in the caller's bridged scope" (bytecode.rs §deferred): it reads
            // the locals by NAME at runtime, exactly like `eval`, and used to
            // fall into the wildcard of both functions. `DeclareDeferred` is
            // the same re-lowering route: renounced out of caution, not
            // because it is proven to read the scope.
            Op::NewAnonDeferred { .. } | Op::DeclareDeferred { .. } => whole = true,
            Op::CallBuiltin { name, .. }
            | Op::CallBuiltinSpread { name, .. }
            | Op::CallHostBuiltin { name, .. }
            | Op::CallHostBuiltinRef { name, .. }
            | Op::CallHostBuiltinOut { name, .. }
            | Op::CallHostBuiltinScanf { name, .. }
            | Op::CallBuiltinRef { name, .. }
            | Op::CallBuiltinRefSpread { name, .. }
            | Op::CallBuiltinRefCell { name, .. } => {
                if observes_scope(name) {
                    whole = true;
                }
            }
            Op::PushRef(s) | Op::IterInitRef(s) => mark(*s, &mut slots),
            Op::StaticAlias { slot, .. } => mark(*slot, &mut slots),
            Op::IterNextRef { value, key, .. } => {
                mark(*value, &mut slots);
                if let Some(k) = key {
                    mark(*k, &mut slots);
                }
            }
            Op::BindRef { target, source } => {
                for b in [target, source] {
                    if let DimBase::Local(s) = b {
                        mark(*s, &mut slots);
                    }
                }
            }
            Op::MakeRef { base, .. }
            | Op::PushArgPlace { base, .. }
            | Op::BindRefTo { base, .. }
            | Op::BindRefToChecked { base, .. } => {
                if let FieldBase::Local(s) = base {
                    mark(*s, &mut slots);
                }
            }
            Op::MakeClosure { captures, .. } => {
                for c in captures.iter() {
                    if c.by_ref {
                        mark(c.src, &mut slots);
                    }
                }
            }
            // Exhaustive here too. A new variant that makes a slot SHARED must
            // not be able to slip in silently through the wildcard — the
            // conservative scope is precisely the thing that would not notice
            // it had become less conservative.
            Op::Alloc { .. }
            | Op::AllocDynamic { .. }
            | Op::AllocStatic { .. }
            | Op::ArrayAppendSpread { .. }
            | Op::ArrayInit { .. }
            | Op::ArrayInsert { .. }
            | Op::ArrayPush { .. }
            | Op::AssignOpPath { .. }
            | Op::AssignPath { .. }
            | Op::Binary { .. }
            // Pure-stack form of `Binary(Add)` — operates only on the stack,
            // makes no slot SHARED.
            | Op::BinaryAdd
            // Register forms: they read by VALUE and write whole via the
            // StoreSlot write-through — none of the seven makes a slot SHARED
            // (Ref-handling stays inside the generic funnel, which installs
            // no alias).
            | Op::BinarySS { .. }
            | Op::BinarySSDst { .. }
            | Op::BinarySC { .. }
            | Op::BinarySCDst { .. }
            | Op::BinaryDst { .. }
            | Op::CmpJmpSS { .. }
            | Op::CmpJmpSC { .. }
            // Later superinstruction batches — same reason as the register
            // forms: shared helpers, no alias installed.
            | Op::BinarySTDst { .. }
            | Op::BinaryTC { .. }
            | Op::BinarySCSC { .. }
            | Op::IncDecSlotPop { .. }
            | Op::IncDecSlotJmp { .. }
            | Op::PropGetSlot { .. }
            | Op::PropDimGetConst { .. }
            | Op::PropConcatGate { .. }
            | Op::NsShadowGuard { .. }
            | Op::PropSetPop { .. }
            | Op::StringifySlot { .. }
            | Op::PropGetSlotRecv { .. }
            | Op::BinaryTCPropSetPop { .. }
            | Op::BinarySCSCDst { .. }
            | Op::LoadVarPushConst { .. }
            | Op::Call { .. }
            | Op::CallArgs { .. }
            | Op::CallArrayMultisort { .. }
            | Op::CallNamed { .. }
            | Op::CallNsFallback { .. }
            | Op::CallNsFallbackArgs { .. }
            | Op::CallSpread { .. }
            | Op::CallValue { .. }
            | Op::CallValueArgs { .. }
            | Op::Cast { .. }
            | Op::CatchMatch { .. }
            | Op::CheckArity { .. }
            | Op::ClassConst { .. }
            | Op::ClassConstDyn { .. }
            | Op::ClassConstDynamic { .. }
            | Op::ClassConstFromValue { .. }
            | Op::ClassNameScope { .. }
            | Op::ClassNameStatic { .. }
            | Op::Clone { .. }
            | Op::ClosureStatic { .. }
            | Op::CmpJmp { .. }
            | Op::CmpJmpConst { .. }
            | Op::CoalesceFetchDim { .. }
            | Op::CoerceParam { .. }
            | Op::CoerceParams { .. }
            | Op::ConcatAssignSlot { .. }
            | Op::ConcatN { .. }
            | Op::ConcatNConst { .. }
            | Op::ConstFetch { .. }
            | Op::DeclareClass { .. }
            | Op::DeclareFn { .. }
            | Op::DeclareTrait { .. }
            | Op::DefineConst { .. }
            | Op::DerefTop { .. }
            | Op::Dup { .. }
            | Op::Echo { .. }
            | Op::EmitNotice { .. }
            | Op::EmptyPath { .. }
            | Op::EndFinally { .. }
            | Op::EnumCase { .. }
            | Op::Exit { .. }
            | Op::Fatal { .. }
            | Op::FetchDim { .. }
            | Op::FetchDimList { .. }
            | Op::FieldAssign { .. }
            | Op::FieldAssignOp { .. }
            | Op::FieldEmpty { .. }
            | Op::FieldIncDec { .. }
            | Op::FieldIsset { .. }
            | Op::FieldUnset { .. }
            | Op::FillDefault { .. }
            | Op::HookCall { .. }
            | Op::IncDecGlobal { .. }
            | Op::IncDecPath { .. }
            | Op::IncDecSlot { .. }
            | Op::IncDecSuperglobal { .. }
            | Op::InitProps { .. }
            | Op::InstanceOf { .. }
            | Op::InstanceOfBuiltin { .. }
            | Op::InstanceOfDynamic { .. }
            | Op::InstanceOfStatic { .. }
            | Op::InvokeCtor { .. }
            | Op::InvokeCtorArgs { .. }
            | Op::InvokeMethod { .. }
            | Op::IssetPath { .. }
            | Op::IterInit { .. }
            | Op::IterNext { .. }
            | Op::IterPop { .. }
            | Op::Jump { .. }
            | Op::JumpIfFalse { .. }
            | Op::JumpIfNotNull { .. }
            | Op::JumpIfNull { .. }
            | Op::JumpIfTrue { .. }
            | Op::LoadGlobal { .. }
            | Op::LoadSlot { .. }
            | Op::LoadSuperglobal { .. }
            | Op::LoadVar { .. }
            | Op::MakeFcc { .. }
            | Op::MatchError { .. }
            | Op::MethodCall { .. }
            | Op::MethodCallArgs { .. }
            | Op::MethodCallDynamic { .. }
            | Op::MethodCallDynamicArgs { .. }
            | Op::MethodCallNamed { .. }
            | Op::Nop { .. }
            | Op::ParkJump { .. }
            | Op::ParkReturn { .. }
            | Op::Pop { .. }
            | Op::Print { .. }
            | Op::PropGet { .. }
            | Op::PropGetDynamic { .. }
            | Op::PropGetDynamicSilent { .. }
            | Op::PropGetSilent { .. }
            | Op::PropIncDec { .. }
            | Op::PropIsset { .. }
            | Op::PropIssetDyn { .. }
            | Op::PropIssetFetchGate { .. }
            | Op::PropOpSet { .. }
            | Op::PropSet { .. }
            | Op::PropUnset { .. }
            | Op::PushConst { .. }
            | Op::PushUndef { .. }
            | Op::Ret { .. }
            | Op::Rethrow { .. }
            | Op::StampThrowable { .. }
            | Op::StaticCall { .. }
            | Op::StaticCallArgs { .. }
            | Op::StaticCallDynamic { .. }
            | Op::StaticCallDynamicArgs { .. }
            | Op::StaticCallDynamicMethod { .. }
            | Op::StaticCallDynamicMethodArgs { .. }
            | Op::StaticCallTargetDynamicMethod { .. }
            | Op::StaticCallTargetDynamicMethodArgs { .. }
            | Op::StaticGuard { .. }
            | Op::StaticPropGet { .. }
            | Op::StaticPropGetDynName { .. }
            | Op::StaticPropGetDynamic { .. }
            | Op::StaticPropIncDec { .. }
            | Op::StaticPropIncDecDynamic { .. }
            | Op::StaticPropOpSet { .. }
            | Op::StaticPropOpSetDynamic { .. }
            | Op::StaticPropRef { .. }
            | Op::StaticPropSet { .. }
            | Op::StaticPropSetDynName { .. }
            | Op::StaticPropSetDynamic { .. }
            | Op::StaticStore { .. }
            | Op::StoreGlobal { .. }
            | Op::StoreSlot { .. }
            | Op::StoreSuperglobal { .. }
            | Op::Stringify { .. }
            | Op::SuppressBegin { .. }
            | Op::SuppressEnd { .. }
            | Op::Swap { .. }
            | Op::Sweep { .. }
            | Op::This { .. }
            | Op::ThisMethodCall { .. }
            | Op::ThisPropGet { .. }
            | Op::Throw { .. }
            | Op::Unary { .. }
            | Op::UnsetPath { .. }
            | Op::Yield { .. }
            | Op::YieldFrom { .. } => {}
        }
    }
    (whole, slots)
}

/// Analyzes one compiled function. Cost O(ops × slot-words × iterations):
/// measurement build, simplicity is a virtue.
pub fn analyze(func: &Func) -> Analysis {
    let ops = &func.ops;
    let n = ops.len();
    let park_targets: Vec<usize> = ops
        .iter()
        .filter_map(|o| match o {
            Op::ParkJump(a) => Some(*a as usize),
            _ => None,
        })
        .collect();
    let effects: Vec<Effect> = ops.iter().map(|o| effect(o, &park_targets)).collect();

    // Bitset width: the declared slots plus the register temps, widened if
    // an op references beyond (defensive: never index out of bounds).
    let mut nbits = (func.n_slots + func.max_temps) as usize;
    for e in &effects {
        for s in e.uses.iter().chain(&e.defs).chain(&e.fall_defs) {
            nbits = nbits.max(*s as usize + 1);
        }
    }

    // Exceptional edges: every op inside a protected region may jump to its
    // handler (no per-edge def).
    let mut exc_edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    for r in &func.exc_table {
        let (start, end) = (r.start as usize, (r.end as usize).min(n));
        for slot_edges in exc_edges.iter_mut().take(end).skip(start) {
            slot_edges.push(r.target as usize);
        }
    }

    let mut live_in: Vec<Bits> = (0..n).map(|_| Bits::new(nbits)).collect();
    let mut live_out: Vec<Bits> = (0..n).map(|_| Bits::new(nbits)).collect();

    // Backward fixed point. Termination is guaranteed: the live sets only grow.
    let mut changed = true;
    while changed {
        changed = false;
        for i in (0..n).rev() {
            let e = &effects[i];
            let mut out = Bits::new(nbits);
            if e.fall && i + 1 < n {
                let mut t = live_in[i + 1].clone();
                for d in &e.fall_defs {
                    t.clear(*d);
                }
                out.or_assign(&t);
            }
            for (tgt, edefs) in &e.edges {
                if *tgt < n {
                    let mut t = live_in[*tgt].clone();
                    for d in edefs {
                        t.clear(*d);
                    }
                    out.or_assign(&t);
                }
            }
            // The exceptional edge's contribution must NOT undergo the kill of
            // the defs of `i`. When the exception fires, the def may not have
            // happened yet: a `CallHostBuiltinOut { out_slot: $m }` that
            // throws a TypeError BEFORE writing the out leaves `$m` with the
            // old value, and the `catch` reads it. Merging the exc
            // contribution INTO `out` and then subtracting the defs made `$m`
            // vanish from the previous reader's `live_out` and become
            // "movable": with `TakeSlot` the catch would have seen `Undef`
            // where Zend prints the value. So: `out` (the judge of
            // movability) carries the exc contribution, but `inb` receives it
            // AFTER the kill, never before.
            let mut inb = out.clone();
            for d in &e.defs {
                inb.clear(*d);
            }
            for tgt in &exc_edges[i] {
                if *tgt < n {
                    out.or_assign(&live_in[*tgt]);
                    inb.or_assign(&live_in[*tgt]);
                }
            }
            if e.uses_all {
                inb.set_all();
            }
            for u in &e.uses {
                inb.set(*u);
            }
            if inb != live_in[i] {
                live_in[i] = inb;
                changed = true;
            }
            live_out[i] = out;
        }
    }

    // F2: the conservative scope on top of the F1 dataflow. `in_region[i]` is
    // the renounce on protected regions (a non-local jump can make live what
    // the linear flow gave up for dead — the analysis HERE already models
    // those edges, but F2 caution renounces anyway, and the F1−F2 difference
    // says what it costs).
    let (whole_renounced, slots_renounced) = renounce(func);
    let mut in_region = vec![false; n];
    for r in &func.exc_table {
        let (start, end) = (r.start as usize, (r.end as usize).min(n));
        for f in in_region.iter_mut().take(end).skip(start) {
            *f = true;
        }
    }

    let mut movable = vec![false; n];
    let mut movable_safe = vec![false; n];
    let mut sites_total = 0u64;
    let mut sites_movable = 0u64;
    let mut sites_safe = 0u64;
    for (i, op) in ops.iter().enumerate() {
        let s = match op {
            Op::LoadSlot(s) => *s,
            Op::LoadVar { slot, .. } => *slot,
            _ => continue,
        };
        sites_total += 1;
        if !live_out[i].get(s) {
            movable[i] = true;
            sites_movable += 1;
            if !whole_renounced && !slots_renounced.get(s) && !in_region[i] {
                movable_safe[i] = true;
                sites_safe += 1;
            }
        }
    }
    Analysis { movable, movable_safe, sites_total, sites_movable, sites_safe }
}
