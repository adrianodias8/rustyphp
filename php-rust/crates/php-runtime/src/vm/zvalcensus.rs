//! MECHANISM counters for "the clone that dies immediately" lever.
//!
//! Why it exists: the lever's prediction is signed on a MECHANISM (how many
//! `Rc`-carrying values get materialized from a slot), not on a stopwatch.
//! The mechanism counter comes before the clock — if the count does not
//! move, the lever did not act, and any Δ time comes from something else.
//! Without these numbers the before/after comparison is not defensible.
//!
//! Same convention as `op-census`/`gc-census`: compiled ONLY behind the
//! `zval-census` feature, which no parity build enables. The release binary
//! does not contain this module.

use std::sync::atomic::{AtomicU64, Ordering};

use php_types::Zval;

/// Total materializations from a slot (`read_slot`), any variant.
pub static SLOT_READS: AtomicU64 = AtomicU64::new(0);
/// Those that cloned an `Rc`-carrying variant: the only ones that pay a
/// refcount++ followed by refcount-- when the copy dies immediately.
pub static SLOT_READS_RC: AtomicU64 = AtomicU64::new(0);
/// Materializations avoided by the lever (fast path served by reference).
/// Before the lever it is 0 by construction: the positive control that
/// tells "the lever acted" from "the time changed for another reason".
pub static SLOT_READS_AVOIDED: AtomicU64 = AtomicU64::new(0);

// ----- Liveness, phase F1 -----
/// Executions of `LoadSlot`/`LoadVar` whose site is a LAST USE according to
/// the [`super::liveness`] analysis: reads that a `TakeSlot` lever could
/// move instead of cloning. MEASUREMENT ONLY: no emission changes.
pub static WOULD_TAKE: AtomicU64 = AtomicU64::new(0);
/// The subset of [`WOULD_TAKE`] whose value carries an `Rc`: the NUMERATOR
/// of the three-band rule (compared against `slot_reads_rc`, same run).
pub static WOULD_TAKE_RC: AtomicU64 = AtomicU64::new(0);
/// Static `LoadSlot`/`LoadVar` sites seen by the analysis (once per
/// analyzed function, per process). Advisory: weighs sites, not executions.
pub static SITES_TOTAL: AtomicU64 = AtomicU64::new(0);
/// How many of those sites are last uses.
pub static SITES_MOVABLE: AtomicU64 = AtomicU64::new(0);
// ----- F2: the conservative perimeter (prediction P2) -----
/// Movable executions that SURVIVE the F2 give-up predicates.
pub static WOULD_TAKE_SAFE: AtomicU64 = AtomicU64::new(0);
/// The rc subset of [`WOULD_TAKE_SAFE`]: the P2 numerator against
/// `would_take_rc` (≥60% or the lever is worth less than its complexity).
pub static WOULD_TAKE_SAFE_RC: AtomicU64 = AtomicU64::new(0);
/// The subset of [`WOULD_TAKE_SAFE`] whose value is a STRING: strings have
/// no observable destructors, so this is the part of the channel that a
/// type-restricted `TakeSlot` (F3) would take without touching the order of
/// the `__destruct` calls — the most insidious risk on the list.
pub static WOULD_TAKE_SAFE_STR: AtomicU64 = AtomicU64::new(0);
/// The subset of [`WOULD_TAKE_SAFE`] that at RUNTIME holds a [`Zval::Ref`].
/// The lesson is that the STATIC give-up does not see the runtime type: a
/// slot on the INNER side of a by-ref closure carries a `Ref` that
/// `param_by_ref` does not cover. This counter measures how big that hole
/// is BEFORE writing the opcode: a `TakeSlot` with a type guard would pay it
/// as a fallback, and without the number the F4 positive control (takes +
/// fallback = predicted safe) would be vacuous by construction.
pub static WOULD_TAKE_SAFE_REF: AtomicU64 = AtomicU64::new(0);
/// Sites that remain movable under the F2 perimeter.
pub static SITES_SAFE: AtomicU64 = AtomicU64::new(0);
/// Take-per-type: the ARRAY and OBJECT subsets of [`WOULD_TAKE_SAFE`] —
/// together with `_STR`/`_REF` they complete the per-type split of what a
/// take would avoid (resolves the 0.21 vs 0.4 s conflict on take-str; the
/// semantic veto on containers stands: these are counts ONLY).
pub static WOULD_TAKE_SAFE_ARR: AtomicU64 = AtomicU64::new(0);
pub static WOULD_TAKE_SAFE_OBJ: AtomicU64 = AtomicU64::new(0);

std::thread_local! {
    /// Per-function cache of the last-use analysis. Key: (address of the
    /// `Func`, address of its `ops`, length) — the double anchor turns an
    /// address-reuse collision into a double-coincidence event, acceptable
    /// in a measurement-only build.
    static LIVENESS: std::cell::RefCell<
        std::collections::HashMap<(usize, usize, usize), std::rc::Rc<super::liveness::Analysis>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Note an execution of `LoadSlot`/`LoadVar` at op `ip` of `func`, BEFORE
/// the materialization (the cell is still in the slot). Analyzes the
/// function on first visit and counts whether this site is a last use.
#[inline]
pub fn note_slot_load_site(func: &crate::bytecode::Func, ip: usize, cell: &Zval) {
    let key = (
        func as *const _ as usize,
        func.ops.as_ptr() as usize,
        func.ops.len(),
    );
    let analysis = LIVENESS.with(|c| {
        let mut c = c.borrow_mut();
        std::rc::Rc::clone(c.entry(key).or_insert_with(|| {
            let a = super::liveness::analyze(func);
            SITES_TOTAL.fetch_add(a.sites_total, Ordering::Relaxed);
            SITES_MOVABLE.fetch_add(a.sites_movable, Ordering::Relaxed);
            SITES_SAFE.fetch_add(a.sites_safe, Ordering::Relaxed);
            std::rc::Rc::new(a)
        }))
    });
    if analysis.movable.get(ip).copied().unwrap_or(false) {
        WOULD_TAKE.fetch_add(1, Ordering::Relaxed);
        if zval_holds_rc(cell) {
            WOULD_TAKE_RC.fetch_add(1, Ordering::Relaxed);
        }
        if analysis.movable_safe.get(ip).copied().unwrap_or(false) {
            WOULD_TAKE_SAFE.fetch_add(1, Ordering::Relaxed);
            if zval_holds_rc(cell) {
                WOULD_TAKE_SAFE_RC.fetch_add(1, Ordering::Relaxed);
            }
            if matches!(cell, Zval::Str(_)) {
                WOULD_TAKE_SAFE_STR.fetch_add(1, Ordering::Relaxed);
            }
            // The hole the static give-up does not see.
            if matches!(cell, Zval::Ref(_)) {
                WOULD_TAKE_SAFE_REF.fetch_add(1, Ordering::Relaxed);
            }
            // Take-per-type: containers, counts ONLY.
            if matches!(cell, Zval::Array(_)) {
                WOULD_TAKE_SAFE_ARR.fetch_add(1, Ordering::Relaxed);
            }
            if matches!(cell, Zval::Object(_)) {
                WOULD_TAKE_SAFE_OBJ.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Does the value carry an `Rc`? Only for these variants do clone/drop cost
/// a refcount update; on the others they are a word copy. Discriminates the
/// `slot_reads_rc` and `would_take_rc` numerators.
pub(super) fn zval_holds_rc(v: &Zval) -> bool {
    match v {
        Zval::Undef | Zval::Null | Zval::Bool(_) | Zval::Long(_) | Zval::Double(_) => false,
        // `Ref` clones the INNER value: the cost is there, not in the wrapper.
        Zval::Ref(r) => zval_holds_rc(&r.borrow()),
        _ => true,
    }
}

// ----- Dynamic species×site×channel census on the PROPERTY path. It
// arbitrates THREE pre-registered predictions (P1 value species, P2
// receiver channel, P3 gc_note attribution) — written BEFORE these
// counters. Same convention as the rest of the module: instrumentation
// builds ONLY.

/// Values that went through the property READ channel (`PropGet`/
/// `ThisPropGet`, IC-hit + fallback + fast-path + general read). P1.
pub static PROPGET_VAL: AtomicU64 = AtomicU64::new(0);
/// The refcounted subset ([`zval_holds_rc`]) of [`PROPGET_VAL`].
pub static PROPGET_VAL_RC: AtomicU64 = AtomicU64::new(0);
/// Values that went through the property WRITE channel (`PropSet`, IC-hit +
/// fast-path + prop_init). P1.
pub static PROPSET_VAL: AtomicU64 = AtomicU64::new(0);
/// The refcounted subset of [`PROPSET_VAL`].
pub static PROPSET_VAL_RC: AtomicU64 = AtomicU64::new(0);
/// Operands (lhs+rhs) that went through `BinaryDst`. P1.
pub static BINDST_OPND: AtomicU64 = AtomicU64::new(0);
/// The refcounted subset of [`BINDST_OPND`].
pub static BINDST_OPND_RC: AtomicU64 = AtomicU64::new(0);

/// RECEIVER channel (P2): clones of an `Rc<Object>` handle made by
/// `LoadVar`/`LoadSlot` (the push of `$o` onto the stack via `read_slot`).
pub static RECV_CLONE_LOAD: AtomicU64 = AtomicU64::new(0);
/// RECEIVER channel (P2): `obj.deref_clone()` inside `PropGet`/`PropSet`/
/// fallback when the target is an `Object` (Rc bump of the receiver).
/// The matching DROPs have no countable site (end of arm): by conservation
/// drop_handle = clone_handle on a stationary micro-benchmark.
pub static RECV_CLONE_PROP: AtomicU64 = AtomicU64::new(0);
/// `Op::Pop` dropping an `Object` handle (the part of the receiver traffic
/// that dies explicitly on the stack, with its `gc_note`).
pub static RECV_DROP_POP: AtomicU64 = AtomicU64::new(0);

/// Every call to `Vm::gc_note` (counted IN the body: catches all sites). P3.
pub static GCNOTE_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Calls to `gc_note` with a NON-refcounted argument (the `_ => {}` arm:
/// the cost is the call+match, not the bookkeeping).
pub static GCNOTE_SCALAR: AtomicU64 = AtomicU64::new(0);
/// Calls to `gc_note` with an `Object` argument (borrow + DESTRUCTED flag +
/// possible insert into gc_buf).
pub static GCNOTE_OBJ: AtomicU64 = AtomicU64::new(0);
/// Calls to `gc_note` with `is_gc_container()` TRUE (they pay
/// `gc_note_slow`); the complement to `GCNOTE_TOTAL` pays only the inline
/// guard. Feeds the `note` channel of the movement partition.
pub static GCNOTE_CONT: AtomicU64 = AtomicU64::new(0);
/// The calls tagged at the `Op::Pop` site (one per successful pop).
pub static GCNOTE_SITE_POP: AtomicU64 = AtomicU64::new(0);
/// The calls tagged at the `PropSet` site on the OLD overwritten value.
/// The residual `total - pop - propset_old` = other sites (Sweep, teardown, …).
pub static GCNOTE_SITE_PROPSET_OLD: AtomicU64 = AtomicU64::new(0);

/// Species×channel on the property path: `chan` 0=PropGet, 1=PropSet,
/// 2=BinaryDst (operand).
#[inline]
pub fn note_prop_val(chan: u8, v: &Zval) {
    REGISTERED.call_once(|| unsafe {
        libc::atexit(dump_at_exit);
    });
    let rc = zval_holds_rc(v);
    let (t, trc) = match chan {
        0 => (&PROPGET_VAL, &PROPGET_VAL_RC),
        1 => (&PROPSET_VAL, &PROPSET_VAL_RC),
        _ => (&BINDST_OPND, &BINDST_OPND_RC),
    };
    t.fetch_add(1, Ordering::Relaxed);
    if rc {
        trc.fetch_add(1, Ordering::Relaxed);
    }
}

/// `LoadVar`/`LoadSlot`: the cell about to be cloned onto the stack.
#[inline]
pub fn note_recv_load(cell: &Zval) {
    if matches!(cell, Zval::Object(_)) {
        RECV_CLONE_LOAD.fetch_add(1, Ordering::Relaxed);
    }
}

/// `PropGet`/`PropSet`/fallback: the target JUST cloned with `deref_clone`.
#[inline]
pub fn note_recv_clone_prop(target: &Zval) {
    if matches!(target, Zval::Object(_)) {
        RECV_CLONE_PROP.fetch_add(1, Ordering::Relaxed);
    }
}

/// `Op::Pop`: the value just popped (about to be `gc_note`'d).
#[inline]
pub fn note_pop(v: &Zval) {
    GCNOTE_SITE_POP.fetch_add(1, Ordering::Relaxed);
    if matches!(v, Zval::Object(_)) {
        RECV_DROP_POP.fetch_add(1, Ordering::Relaxed);
    }
}

/// Body of `Vm::gc_note`: every call, with the species of the argument.
#[inline]
pub fn note_gcnote(v: &Zval) {
    REGISTERED.call_once(|| unsafe {
        libc::atexit(dump_at_exit);
    });
    GCNOTE_TOTAL.fetch_add(1, Ordering::Relaxed);
    if v.is_gc_container() {
        GCNOTE_CONT.fetch_add(1, Ordering::Relaxed);
    }
    match v {
        Zval::Undef | Zval::Null | Zval::Bool(_) | Zval::Long(_) | Zval::Double(_) => {
            GCNOTE_SCALAR.fetch_add(1, Ordering::Relaxed);
        }
        Zval::Object(_) => {
            GCNOTE_OBJ.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }
}

/// `PropSet` site: the `gc_note` on the old overwritten value.
#[inline]
pub fn note_gcnote_site_propset_old() {
    GCNOTE_SITE_PROPSET_OLD.fetch_add(1, Ordering::Relaxed);
}

/// SEPARATE property-path line (the historical line's format stays intact:
/// the figures gate parses it as is).
pub fn dump_line_s101() -> String {
    format!(
        "zvalcensus_s101 propget_val={} propget_val_rc={} propset_val={} propset_val_rc={} bindst_opnd={} bindst_opnd_rc={} recv_clone_load={} recv_clone_prop={} recv_drop_pop={} gcnote_total={} gcnote_scalar={} gcnote_obj={} gcnote_site_pop={} gcnote_site_propset_old={}",
        PROPGET_VAL.load(Ordering::Relaxed),
        PROPGET_VAL_RC.load(Ordering::Relaxed),
        PROPSET_VAL.load(Ordering::Relaxed),
        PROPSET_VAL_RC.load(Ordering::Relaxed),
        BINDST_OPND.load(Ordering::Relaxed),
        BINDST_OPND_RC.load(Ordering::Relaxed),
        RECV_CLONE_LOAD.load(Ordering::Relaxed),
        RECV_CLONE_PROP.load(Ordering::Relaxed),
        RECV_DROP_POP.load(Ordering::Relaxed),
        GCNOTE_TOTAL.load(Ordering::Relaxed),
        GCNOTE_SCALAR.load(Ordering::Relaxed),
        GCNOTE_OBJ.load(Ordering::Relaxed),
        GCNOTE_SITE_POP.load(Ordering::Relaxed),
        GCNOTE_SITE_PROPSET_OLD.load(Ordering::Relaxed),
    )
}

static REGISTERED: std::sync::Once = std::sync::Once::new();

extern "C" fn dump_at_exit() {
    dump_exit();
}

#[inline]
pub fn note_slot_read(is_rc: bool) {
    // The dump registers itself at the first note, so the module is
    // self-contained and no binary's `main` needs touching. The cost of the
    // `Once` exists only in instrumentation builds.
    REGISTERED.call_once(|| unsafe {
        libc::atexit(dump_at_exit);
    });
    SLOT_READS.fetch_add(1, Ordering::Relaxed);
    if is_rc {
        SLOT_READS_RC.fetch_add(1, Ordering::Relaxed);
    }
}

#[inline]
pub fn note_avoided() {
    SLOT_READS_AVOIDED.fetch_add(1, Ordering::Relaxed);
}

/// Single line, bare-ascii format (no thousands separators), so the raw
/// enters the figures gate's corpus without post-processing.
pub fn dump_line() -> String {
    format!(
        "zvalcensus slot_reads={} slot_reads_rc={} slot_reads_avoided={} would_take={} would_take_rc={} would_take_safe={} would_take_safe_rc={} would_take_safe_str={} would_take_safe_ref={} sites_total={} sites_movable={} sites_safe={}",
        SLOT_READS.load(Ordering::Relaxed),
        SLOT_READS_RC.load(Ordering::Relaxed),
        SLOT_READS_AVOIDED.load(Ordering::Relaxed),
        WOULD_TAKE.load(Ordering::Relaxed),
        WOULD_TAKE_RC.load(Ordering::Relaxed),
        WOULD_TAKE_SAFE.load(Ordering::Relaxed),
        WOULD_TAKE_SAFE_RC.load(Ordering::Relaxed),
        WOULD_TAKE_SAFE_STR.load(Ordering::Relaxed),
        WOULD_TAKE_SAFE_REF.load(Ordering::Relaxed),
        SITES_TOTAL.load(Ordering::Relaxed),
        SITES_MOVABLE.load(Ordering::Relaxed),
        SITES_SAFE.load(Ordering::Relaxed),
    )
}

/// "Hint-check without clone" lever — MECHANISM counters (same convention
/// as the slot-read counters: the positive control tells "the lever acted"
/// from "the time changed for another reason").
/// Calls to `coerce_or_check_hint` (any outcome).
pub static HINT_CHECKS: AtomicU64 = AtomicU64::new(0);
/// The subset whose value carries an `Rc` ([`zval_holds_rc`]): before the
/// lever EACH of them pays a `deref_clone` that dies at the end of the check.
pub static HINT_CHECKS_RC: AtomicU64 = AtomicU64::new(0);
/// Checks served by the borrow-first path WITHOUT a clone. Before the lever
/// it is 0 by construction.
pub static HINT_AVOIDED: AtomicU64 = AtomicU64::new(0);

/// Note a call to `coerce_or_check_hint` with the incoming value.
#[inline]
pub fn note_hint_check(v: &Zval) {
    REGISTERED.call_once(|| unsafe {
        libc::atexit(dump_at_exit);
    });
    HINT_CHECKS.fetch_add(1, Ordering::Relaxed);
    if zval_holds_rc(v) {
        HINT_CHECKS_RC.fetch_add(1, Ordering::Relaxed);
    }
}

/// Note a check served without a clone (only the build with the lever
/// touches it).
#[inline]
pub fn note_hint_avoided() {
    HINT_AVOIDED.fetch_add(1, Ordering::Relaxed);
}

/// SEPARATE hint-check line (the historical lines stay byte-identical).
pub fn dump_line_s140() -> String {
    format!(
        "zvalcensus_s140 hint_checks={} hint_checks_rc={} hint_avoided={}",
        HINT_CHECKS.load(Ordering::Relaxed),
        HINT_CHECKS_RC.load(Ordering::Relaxed),
        HINT_AVOIDED.load(Ordering::Relaxed),
    )
}

/// Writes the counters at process end. `PHPR_ZVAL_CENSUS` is a **path**: the
/// line is APPENDED to that file.
///
/// Why not stderr (measured, not feared): the real workload spawns child
/// processes and the tests capture their stderr — the child's census line
/// turned into a `PHPUnit\Framework\Exception` and the first measurement
/// came out with 15 spurious errors. An instrument that talks on the channel
/// the measured system reads does not measure: it participates. Appending
/// from several processes is intended: the workload sum includes the
/// children, which execute as much PHP as the parent.
///
/// The env is read HERE, at process end, never on the hot path.
pub fn dump_exit() {
    use std::io::Write;
    let Some(path) = std::env::var_os("PHPR_ZVAL_CENSUS") else { return };
    if path.is_empty() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{}", dump_line());
        let _ = writeln!(f, "{}", dump_line_s101());
        // Container notes (NEW line, the historical property-path line stays
        // byte-identical).
        let _ = writeln!(
            f,
            "zvalcensus_s145 gcnote_cont={}",
            GCNOTE_CONT.load(Ordering::Relaxed)
        );
        // Hint-check counter line — NEW line.
        let _ = writeln!(f, "{}", dump_line_s140());
        // Take-per-type (NEW line; the historical lines stay byte-identical
        // — arr/obj next to the historical line's str/ref).
        let _ = writeln!(
            f,
            "zvalcensus_s147 would_take_safe_arr={} would_take_safe_obj={}",
            WOULD_TAKE_SAFE_ARR.load(Ordering::Relaxed),
            WOULD_TAKE_SAFE_OBJ.load(Ordering::Relaxed),
        );
        // Inline array-teardown mechanism counters — NEW line, ONLY when the
        // build also mounts mem-census (the symbols do not exist otherwise).
        #[cfg(feature = "mem-census")]
        {
            let (ra, re_, rt) = php_types::memcensus::rd1_counters();
            let _ =
                writeln!(f, "zvalcensus_s142 rd1_arrays={ra} rd1_elems={re_} rd1_tombs={rt}");
        }
        // Operand-stack census lines (separate module).
        let _ = writeln!(f, "{}", super::stackcensus::dump_lines());
        // Alloc leg by DIRECT mem-census — bytes and COUNTS from the counting
        // global_allocator (0/0 if this build does not mount CountingMi: the
        // field also says WHICH build wrote it).
        let (ab, fb) = php_types::memcensus::alloc_counters();
        let (an, fn_) = php_types::memcensus::alloc_event_counters();
        let _ = writeln!(
            f,
            "alloccensus galloc_bytes={ab} gfree_bytes={fb} galloc_n={an} gfree_n={fn_}"
        );
        // Realloc DISAGGREGATED + size-class histogram of the pure allocs —
        // NEW lines, the historical line stays byte-identical (galloc/gfree
        // NO LONGER include reallocs).
        let (rn, ro, rnew) = php_types::memcensus::realloc_counters();
        let _ = writeln!(f, "realloccensus n={rn} old_bytes={ro} new_bytes={rnew}");
        let h = php_types::memcensus::alloc_histogram();
        let _ = writeln!(
            f,
            "allochist le16={} le32={} le48={} le64={} le96={} le128={} le256={} le512={} le1k={} le4k={} gt4k={}",
            h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10]
        );
        // The measured free histogram — NEW line, the historical lines stay
        // byte-identical.
        let fh = php_types::memcensus::free_histogram();
        let _ = writeln!(
            f,
            "freehist le16={} le32={} le48={} le64={} le96={} le128={} le256={} le512={} le1k={} le4k={} gt4k={}",
            fh[0], fh[1], fh[2], fh[3], fh[4], fh[5], fh[6], fh[7], fh[8], fh[9], fh[10]
        );
        // The arity seen by bind_params — NEW line, the historical lines
        // stay byte-identical.
        let ar = php_types::memcensus::arity_histogram();
        let _ = writeln!(
            f,
            "argarity a0={} a1={} a2={} a3={} a4={} ge5={}",
            ar[0], ar[1], ar[2], ar[3], ar[4], ar[5]
        );
    }
}
