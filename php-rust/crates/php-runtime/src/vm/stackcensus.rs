//! Census of the OPERAND-STACK MECHANICS, per OPCODE SITE and per PRIMITIVE.
//!
//! Why it exists: the inline-aware profile attributes ~26.6% of phpr time
//! to the `Vec<Zval>` accessors inside `run_loop`, but sample attribution
//! on inlined symbols is admittedly porous (the boundary with the 21.2%
//! dispatch leaks in both directions; a factor-~2 error of UNKNOWN SIGN).
//! The only arbiter is the COUNT: how many stack transits
//! (push/pop/peek/len/elem) each judge op REALLY performs per iteration.
//! Deriving expectations via share%×T is FORBIDDEN: the cost per transit is
//! trusted ONLY from a Δ_A/B ÷ counted transits.
//!
//! Same convention as `zvalcensus`: ONLY behind `zval-census`, no parity
//! build enables it; the dump is appended to the `PHPR_ZVAL_CENSUS` file
//! (its own `stackcensus …` line, the historical line stays byte-identical).

use std::sync::atomic::{AtomicU64, Ordering};

/// Instrumented opcode sites: the ops of the prop.php judge loop (13 kinds)
/// + `Other` for any out-of-perimeter transit one might want to tag.
#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Site {
    Pop = 0,
    Swap = 1,
    LoadSlot = 2,
    LoadVar = 3,
    PushConst = 4,
    IncDecSlot = 5,
    BinaryAdd = 6,
    BinaryDst = 7,
    CmpJmpSC = 8,
    Sweep = 9,
    Jump = 10,
    PropGet = 11,
    PropSet = 12,
    Other = 13,
    /// The RMW-on-slot fusion (LoadSlot;Swap;BinaryDst).
    BinarySTDst = 14,
}
pub const SITES: usize = 15;
pub const SITE_NAMES: [&str; SITES] = [
    "Pop", "Swap", "LoadSlot", "LoadVar", "PushConst", "IncDecSlot",
    "BinaryAdd", "BinaryDst", "CmpJmpSC", "Sweep", "Jump", "PropGet",
    "PropSet", "Other", "BinarySTDst",
];

/// Stack primitives at SOURCE level (the profile's `as_slice`/`ptr::read`
/// are the compiled internals of these): `Push` = `Vec::push`,
/// `Pop` = `Vec::pop` (+`expect`), `Peek` = `last`/`last_mut`,
/// `Len` = explicit `len()`, `Elem` = element access (`swap`, index).
#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Prim {
    Push = 0,
    Pop = 1,
    Peek = 2,
    Len = 3,
    Elem = 4,
    // Drop census: the end of life of a Zval in the arm, per SPECIES — the
    // species is the channel of the scalar fast-out lever. Classified with
    // the EXISTING `is_gc_container` predicate (never a second predicate).
    DropS = 5,
    DropC = 6,
}
pub const PRIMS: usize = 7;
pub const PRIM_NAMES: [&str; PRIMS] =
    ["push", "pop", "peek", "len", "elem", "drop_s", "drop_c"];

/// Executions per opcode site (closes the ledger: transits = Σ counts and
/// the counts↔dump assert compares ops/iter against the `{main}` dump).
static OPS: [AtomicU64; SITES] = [const { AtomicU64::new(0) }; SITES];
/// Transits per (site, primitive).
static ST: [[AtomicU64; PRIMS]; SITES] = [const { [const { AtomicU64::new(0) }; PRIMS] }; SITES];

/// One execution of the `site` arm.
#[inline]
pub fn note_op(site: Site) {
    OPS[site as usize].fetch_add(1, Ordering::Relaxed);
}

/// `n` transits of primitive `prim` at site `site` (on the executed PATH:
/// fast and slow paths are counted where they run, never statically).
#[inline]
pub fn note(site: Site, prim: Prim, n: u64) {
    ST[site as usize][prim as usize].fetch_add(n, Ordering::Relaxed);
}

/// One Zval end of life at site `site`, classified by species (drop
/// census). Noted at the point of the arm where the value's life ends
/// (discarded pop, consumed operand, overwritten target, temporary at end of
/// arm) — on the EXECUTED path.
#[inline]
pub fn note_drop(site: Site, is_container: bool) {
    let p = if is_container { Prim::DropC } else { Prim::DropS };
    ST[site as usize][p as usize].fetch_add(1, Ordering::Relaxed);
}

/// Dump lines: one per executed site (`stackcensus site=… ops=… push=…`)
/// + one line of per-primitive totals.
pub fn dump_lines() -> String {
    let mut out = String::new();
    let mut tot = [0u64; PRIMS];
    for s in 0..SITES {
        let ops = OPS[s].load(Ordering::Relaxed);
        let row: Vec<u64> = (0..PRIMS).map(|p| ST[s][p].load(Ordering::Relaxed)).collect();
        for (p, v) in row.iter().enumerate() {
            tot[p] += v;
        }
        if ops == 0 && row.iter().all(|&v| v == 0) {
            continue;
        }
        out.push_str(&format!(
            "stackcensus site={} ops={} push={} pop={} peek={} len={} elem={} drop_s={} drop_c={}\n",
            SITE_NAMES[s], ops, row[0], row[1], row[2], row[3], row[4], row[5], row[6]
        ));
    }
    out.push_str(&format!(
        "stackcensus_tot push={} pop={} peek={} len={} elem={} drop_s={} drop_c={}",
        tot[0], tot[1], tot[2], tot[3], tot[4], tot[5], tot[6]
    ));
    out
}
