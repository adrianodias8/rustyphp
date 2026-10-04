//! Operand-kind census (ZEND_VM_NOTES.md, step 2): for every dispatched op,
//! which kind of producer made each stack operand it consumed — a local
//! read (CV), a literal (CONST), `$this`, a `Dup`, or any other op (TMP) —
//! split by value class, plus the Zval clones and string-handle drops that
//! happened while the op ran. For the Rc-carrying CV/CONST/THIS operands it
//! also records the operand's FATE: the source's refcount after the op
//! against its value right after the load tells "the consumer dropped the
//! copy" (a clone/drop pair that borrowing the operand would remove) from
//! "the consumer kept it" (Zend would ZVAL_COPY there too).
//!
//! Method: a shadow of every frame's operand stack, reconciled before each
//! dispatch against the real stacks by value identity (tag + pointer, or the
//! scalar bits). Entries that disappeared were consumed by the previous op,
//! new ones were produced by it. Only identities are stored, never
//! dereferenced; fate reads go through the live source (slot, literal table,
//! `$this`). A freed-and-reused address at the same stack position, or a
//! generator swapping frames, can mis-attribute a few entries: noise, not
//! bias. Compiled only with both `op-census` and `mem-census`.

use std::cell::RefCell;
use std::sync::atomic::Ordering::Relaxed;

use super::{op_index, OP_NAMES, N_OPS};
use crate::vm::Frame;
use crate::bytecode::{Const, DimBase, FieldBase, Op};
use php_types::Zval;

pub const KINDS: [&str; 5] = ["CV", "CONST", "TMP", "DUP", "THIS"];
const CV: u8 = 0;
const CONST: u8 = 1;
const TMP: u8 = 2;
const DUP: u8 = 3;
const THIS: u8 = 4;
pub const CLASSES: [&str; 5] = ["scalar", "str", "arr", "obj", "ref"];
/// Fate of an Rc-carrying CV/CONST/THIS operand.
const FATES: [&str; 3] = ["dropped", "kept", "unknown"];

#[derive(Clone, Copy)]
struct Ent {
    id: u64,
    tag: u8,
    kind: u8,
    class: u8,
    /// Slot (CV) or literal index (CONST) of the source; unused otherwise.
    src: u32,
    /// Source refcount right after the load (0 for scalars).
    c0: u32,
}

struct State {
    shadow: Vec<Vec<Ent>>,
    /// (op index, depth, CV slot / literal index of the op's operand fields,
    /// kinds of the operands the op embeds)
    last: Option<(usize, usize, u32, u32, &'static str)>,
    /// Stack operands consumed by the op being attributed, bottom first.
    cur: Vec<(u8, u8)>,
    /// (op, embedded kinds, consumed kinds) -> executions.
    combos: std::collections::HashMap<(usize, &'static str, Vec<(u8, u8)>), u64>,
    consumed: Vec<[[u64; 5]; 5]>,
    fate: Vec<[[u64; 3]; 5]>,
    clones: Vec<[u64; 6]>,
    str_drops: Vec<u64>,
    snap_clones: [u64; 6],
    snap_drops: u64,
}

thread_local! {
    static ST: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn ident(v: &Zval) -> (u64, u8, u8) {
    match v {
        Zval::Undef => (0, 0, 0),
        Zval::Null => (0, 1, 0),
        Zval::Bool(b) => (*b as u64, 2, 0),
        Zval::Long(n) => (*n as u64, 3, 0),
        Zval::Double(d) => (d.to_bits(), 4, 0),
        Zval::Str(s) => (php_types::ZStr::as_ptr(s) as u64, 5, 1),
        Zval::Array(a) => (std::rc::Rc::as_ptr(a) as u64, 6, 2),
        Zval::Object(o) => (std::rc::Rc::as_ptr(o) as *const u8 as u64, 7, 3),
        Zval::Closure(c) => (std::rc::Rc::as_ptr(c) as *const u8 as u64, 8, 3),
        Zval::Generator(g) => (std::rc::Rc::as_ptr(g) as *const u8 as u64, 9, 3),
        Zval::Resource(r) => (std::rc::Rc::as_ptr(r) as *const u8 as u64, 10, 3),
        Zval::WeakHandle(_, id) => (*id as u64, 11, 0),
        Zval::ArgPlace(p) => (std::rc::Rc::as_ptr(p) as *const u8 as u64, 12, 3),
        Zval::Ref(r) => (std::rc::Rc::as_ptr(r) as *const u8 as u64, 13, 4),
    }
}

fn refcount(v: &Zval) -> u32 {
    use std::rc::Rc;
    (match v {
        Zval::Str(s) => php_types::zstr_refcount(s),
        Zval::Array(a) => Rc::strong_count(a),
        Zval::Object(o) => Rc::strong_count(o),
        Zval::Closure(c) => Rc::strong_count(c),
        Zval::Generator(g) => Rc::strong_count(g),
        Zval::Resource(r) => Rc::strong_count(r),
        Zval::ArgPlace(p) => Rc::strong_count(p),
        Zval::Ref(r) => Rc::strong_count(r),
        _ => 0,
    }) as u32
}

/// The live source value of an operand, if it still holds the same identity:
/// its refcount now. A slot holding a reference is looked through (LoadVar
/// pushes the referenced value).
fn source_rc(e: &Ent, fr: &Frame) -> Option<u32> {
    let probe = |v: &Zval| -> Option<u32> {
        let (id, tag, _) = ident(v);
        (id == e.id && tag == e.tag).then(|| refcount(v))
    };
    match e.kind {
        CV => match fr.slots.get(e.src as usize)? {
            Zval::Ref(r) => probe(&r.borrow()),
            v => probe(v),
        },
        CONST => match fr.func.consts.get(e.src as usize)? {
            // Same allocation as the pushed copy: compare the pointer, read
            // the shared count (no clone, so the census itself moves nothing).
            Const::Str(s) => (e.tag == 5 && php_types::ZStr::as_ptr(s) as u64 == e.id)
                .then(|| php_types::zstr_refcount(s) as u32),
            _ => None,
        },
        THIS => probe(fr.this.as_ref()?),
        _ => None,
    }
}

/// Kind of the `k`-th value pushed by op `li` (whose operand fields are `a`, `b`).
fn producer(li: usize, k: usize, a: u32, b: u32) -> (u8, u32) {
    match OP_NAMES[li] {
        "LoadVar" | "LoadSlot" => (CV, a),
        "PushConst" => (CONST, a),
        "LoadVarPushConst" if k == 0 => (CV, a),
        "LoadVarPushConst" => (CONST, b),
        "Dup" => (DUP, 0),
        "This" => (THIS, 0),
        _ => (TMP, 0),
    }
}

fn operand_fields(op: &Op) -> (u32, u32) {
    match op {
        Op::LoadVar { slot, .. } => (*slot as u32, 0),
        Op::LoadSlot(s) => (*s as u32, 0),
        Op::PushConst(i) => (*i as u32, 0),
        Op::LoadVarPushConst { slot, cidx } => (*slot as u32, *cidx as u32),
        _ => (0, 0),
    }
}

/// Operands an op carries in its own fields (Zend would see them as op1/op2
/// of a specialised handler): the base of a path, a literal name.
fn embedded(op: &Op) -> &'static str {
    let fb = |b: &FieldBase| match b {
        FieldBase::Local(_) => "base=CV",
        FieldBase::This => "base=THIS",
        _ => "base=global",
    };
    let db = |b: &DimBase| match b {
        DimBase::Local(_) => "base=CV",
        _ => "base=global",
    };
    match op {
        Op::FieldIsset { base, .. } | Op::FieldAssign { base, .. } => fb(base),
        Op::FieldUnset { base, .. } | Op::FieldEmpty { base, .. } => fb(base),
        Op::AssignPath { base, .. } | Op::IssetPath { base, .. } => db(base),
        Op::ThisPropGet { .. } | Op::PropSetPop { .. } => "this, name=CONST",
        Op::MethodCall { .. } | Op::CallNsFallback { .. } => "name=CONST",
        Op::StoreSlot(_) => "dst=CV",
        Op::LoadVar { .. } | Op::LoadSlot(_) => "src=CV",
        Op::PushConst(_) => "src=CONST",
        Op::CmpJmpSC { .. } => "CV, CONST",
        Op::CmpJmpSS { .. } => "CV, CV",
        _ => "",
    }
}

impl State {
    fn new() -> State {
        State {
            shadow: Vec::new(),
            last: None,
            cur: Vec::new(),
            combos: std::collections::HashMap::new(),
            consumed: vec![[[0; 5]; 5]; N_OPS],
            fate: vec![[[0; 3]; 5]; N_OPS],
            clones: vec![[0; 6]; N_OPS],
            str_drops: vec![0; N_OPS],
            snap_clones: [0; 6],
            snap_drops: 0,
        }
    }

    fn consume(&mut self, li: usize, e: Ent, fr: Option<&Frame>) {
        self.consumed[li][e.kind as usize][e.class as usize] += 1;
        self.cur.push((e.kind, e.class));
        if e.class == 0 || !matches!(e.kind, CV | CONST | THIS) {
            return;
        }
        let f = match fr.and_then(|fr| source_rc(&e, fr)) {
            Some(c1) if c1 < e.c0 => 0,
            Some(_) => 1,
            None => 2,
        };
        self.fate[li][e.kind as usize][f] += 1;
    }

    fn reconcile(&mut self, d: usize, fr: &Frame, li: usize, a: u32, b: u32) {
        let real = &fr.stack;
        let sh = std::mem::take(&mut self.shadow[d]);
        let mut p = 0;
        while p < sh.len() && p < real.len() {
            let (id, tag, _) = ident(&real[p]);
            if sh[p].id != id || sh[p].tag != tag {
                break;
            }
            p += 1;
        }
        for e in &sh[p..] {
            self.consume(li, *e, Some(fr));
        }
        let mut sh = sh;
        sh.truncate(p);
        for (k, v) in real[p..].iter().enumerate() {
            let (id, tag, class) = ident(v);
            let (kind, src) = producer(li, k, a, b);
            // The count is the allocation's, shared by the source and the copy.
            let c0 = if class == 0 { 0 } else { refcount(v) };
            sh.push(Ent { id, tag, kind, class, src, c0 });
        }
        self.shadow[d] = sh;
    }

    fn note(&mut self, op: &Op, frames: &[Frame], top: usize) {
        let (c0, c1, c2, c3, c4, c5) = php_types::memcensus::s145_counters();
        let now = [c0, c1, c2, c3, c4, c5];
        let drops = php_types::ZSTR_DROPS.load(Relaxed);
        if let Some((li, ld, a, b, emb)) = self.last {
            for c in 0..6 {
                self.clones[li][c] += now[c] - self.snap_clones[c];
            }
            self.str_drops[li] += drops - self.snap_drops;
            // Frames that vanished (Ret, unwinding): everything left was consumed.
            while self.shadow.len() > frames.len() {
                let sh = self.shadow.pop().unwrap_or_default();
                for e in sh {
                    self.consume(li, e, None);
                }
            }
            while self.shadow.len() < frames.len() {
                self.shadow.push(Vec::new());
            }
            if ld < frames.len() {
                self.reconcile(ld, &frames[ld], li, a, b);
            }
            if top != ld {
                self.reconcile(top, &frames[top], li, a, b);
            }
            let cur = std::mem::take(&mut self.cur);
            *self.combos.entry((li, emb, cur)).or_insert(0) += 1;
        } else {
            self.shadow.resize(frames.len(), Vec::new());
        }
        let (a, b) = operand_fields(op);
        self.last = Some((op_index(op), top, a, b, embedded(op)));
        self.snap_clones = now;
        self.snap_drops = drops;
    }

    fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut o = String::new();
        let n: Vec<u64> = (0..N_OPS)
            .map(|i| self.consumed[i].iter().flatten().sum::<u64>() + self.clones[i].iter().sum::<u64>())
            .collect();
        let mut idx: Vec<usize> = (0..N_OPS).filter(|&i| n[i] > 0).collect();
        idx.sort_by_key(|&i| std::cmp::Reverse(n[i]));
        let _ = writeln!(o, "== PHPR_OPND_CENSUS ==");
        let _ = writeln!(o, "op\tkind\tscalar\tstr\tarr\tobj\tref\tdropped\tkept\tunknown");
        for &i in &idx {
            for k in 0..5 {
                let c = self.consumed[i][k];
                if c.iter().sum::<u64>() == 0 {
                    continue;
                }
                let f = self.fate[i][k];
                let _ = writeln!(
                    o,
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    OP_NAMES[i], KINDS[k], c[0], c[1], c[2], c[3], c[4], f[0], f[1], f[2]
                );
            }
        }
        let _ = writeln!(o, "-- clones (scalar str arr obj ref rcother) and string drops per op --");
        for &i in &idx {
            let c = self.clones[i];
            if c.iter().sum::<u64>() + self.str_drops[i] == 0 {
                continue;
            }
            let _ = writeln!(
                o,
                "clones\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tstrdrops\t{}",
                OP_NAMES[i], c[0], c[1], c[2], c[3], c[4], c[5], self.str_drops[i]
            );
        }
        let _ = writeln!(o, "-- operand-kind combinations: op, embedded, consumed (bottom first) --");
        let mut rows: Vec<_> = self.combos.iter().collect();
        rows.sort_by(|a, b| (a.0 .0, std::cmp::Reverse(a.1)).cmp(&(b.0 .0, std::cmp::Reverse(b.1))));
        for ((i, emb, kinds), n) in rows {
            let ks: Vec<String> = kinds
                .iter()
                .map(|&(k, c)| format!("{}:{}", KINDS[k as usize], CLASSES[c as usize]))
                .collect();
            let _ = writeln!(o, "combo\t{}\t{}\t{}\t{}", OP_NAMES[*i], n, emb, ks.join(","));
        }
        let _ = FATES;
        o
    }
}

/// Before each dispatch (census builds, armed census only).
#[cold]
pub fn note(op: &Op, frames: &[Frame], top: usize) {
    ST.with(|s| s.borrow_mut().get_or_insert_with(State::new).note(op, frames, top));
}

/// Append the table to `$PHPR_OPND_CENSUS` (or stderr) and reset.
pub fn dump() {
    let Some(st) = ST.with(|s| s.borrow_mut().take()) else { return };
    let report = st.render();
    match std::env::var("PHPR_OPND_CENSUS") {
        Ok(p) if p.starts_with('/') => {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
                let _ = f.write_all(report.as_bytes());
            }
        }
        _ => eprint!("{report}"),
    }
}
