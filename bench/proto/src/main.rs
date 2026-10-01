//! Safe-Rust interpreter layouts, measured in isolation (NOTES.md session 8).
//!
//! The same tiny bytecode (ferro-style fused ops, a 16-byte `Zval` with
//! `Rc` payloads) runs under two VM layouts:
//!   A  today's ferro layout: `Vec<Frame>`, each frame owning its `slots`
//!      and `stack` Vecs (pooled), and every op re-reading `frames[top]`
//!      for `ip` and `func`;
//!   B  one contiguous value stack (slots + operands), frames as plain
//!      index records, and `func`/`ip`/`base` held in locals across ops.
//! No `unsafe` in A, B, B2; B3 (information only) shows what unchecked
//! indexing would add. Workloads mirror bench calls.php: an empty `for`
//! loop and `$x = f($x)` in a loop. PHP 8.5.7 (no opcache, no JIT) on the
//! same box: 2.1 ns per empty iteration, 10.0 ns per call iteration.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

#[allow(dead_code)]
#[derive(Clone, Debug)]
enum Zval {
    Undef,
    Null,
    Bool(bool),
    Long(i64),
    Double(f64),
    Str(Rc<str>),
    Arr(Rc<Vec<Zval>>),
    Obj(Rc<RefCell<Vec<Zval>>>),
}

#[derive(Clone, Copy, Debug)]
enum Op {
    PushConst(u32),
    LoadSlot(u16),
    StoreSlot(u16),
    /// push slot + const (Long fast path)
    AddSC { slot: u16, c: u32 },
    /// if !(slot l < slot r) goto addr
    CmpJmpLtSS { l: u16, r: u16, addr: u32 },
    /// slot += 1; goto addr
    IncSlotJmp { slot: u16, addr: u32 },
    Call { func: u32, argc: u16 },
    Ret,
}

struct Func {
    ops: Vec<Op>,
    consts: Vec<Zval>,
    n_slots: usize,
}

fn add(a: &Zval, b: &Zval) -> Zval {
    match (a, b) {
        (Zval::Long(x), Zval::Long(y)) => match x.checked_add(*y) {
            Some(v) => Zval::Long(v),
            None => Zval::Double(*x as f64 + *y as f64),
        },
        _ => Zval::Null,
    }
}

fn lt(a: &Zval, b: &Zval) -> bool {
    match (a, b) {
        (Zval::Long(x), Zval::Long(y)) => x < y,
        _ => false,
    }
}

// ---------------------------------------------------------------- layout A

struct FrameA<'m> {
    func: &'m Func,
    ip: usize,
    slots: Vec<Zval>,
    stack: Vec<Zval>,
    this: Option<Zval>,
    class: Option<usize>,
    flags: u8,
}

struct VmA<'m> {
    funcs: &'m [Func],
    frames: Vec<FrameA<'m>>,
    pool: Vec<(Vec<Zval>, Vec<Zval>)>,
}

impl<'m> VmA<'m> {
    fn run(&mut self, entry: usize, args: Vec<Zval>) -> Zval {
        let f = &self.funcs[entry];
        let mut slots = args;
        slots.resize(f.n_slots, Zval::Undef);
        self.frames.push(FrameA { func: f, ip: 0, slots, stack: Vec::new(), this: None, class: None, flags: 0 });
        let baseline = self.frames.len() - 1;
        loop {
            if self.frames.len() > 10_000 {
                panic!("depth");
            }
            let top = self.frames.len() - 1;
            let ip = self.frames[top].ip;
            let func = self.frames[top].func;
            let op = &func.ops[ip];
            self.frames[top].ip = ip + 1;
            match *op {
                Op::PushConst(i) => {
                    let v = func.consts[i as usize].clone();
                    self.frames[top].stack.push(v);
                }
                Op::LoadSlot(s) => {
                    let v = self.frames[top].slots[s as usize].clone();
                    self.frames[top].stack.push(v);
                }
                Op::StoreSlot(s) => {
                    let v = self.frames[top].stack.pop().unwrap();
                    self.frames[top].slots[s as usize] = v;
                }
                Op::AddSC { slot, c } => {
                    let v = add(&self.frames[top].slots[slot as usize], &func.consts[c as usize]);
                    self.frames[top].stack.push(v);
                }
                Op::CmpJmpLtSS { l, r, addr } => {
                    let fr = &self.frames[top];
                    if !lt(&fr.slots[l as usize], &fr.slots[r as usize]) {
                        self.frames[top].ip = addr as usize;
                    }
                }
                Op::IncSlotJmp { slot, addr } => {
                    if let Zval::Long(n) = &mut self.frames[top].slots[slot as usize] {
                        *n += 1;
                    }
                    self.frames[top].ip = addr as usize;
                }
                Op::Call { func: fi, argc } => {
                    let callee = &self.funcs[fi as usize];
                    let (mut slots, stack) = self.pool.pop().unwrap_or_default();
                    slots.resize(callee.n_slots, Zval::Undef);
                    for i in (0..argc as usize).rev() {
                        slots[i] = self.frames[top].stack.pop().unwrap();
                    }
                    self.frames.push(FrameA { func: callee, ip: 0, slots, stack, this: None, class: None, flags: 0 });
                }
                Op::Ret => {
                    let mut dead = self.frames.pop().unwrap();
                    let v = dead.stack.pop().unwrap_or(Zval::Null);
                    let _ = (dead.this.take(), dead.class, dead.flags);
                    dead.slots.clear();
                    dead.stack.clear();
                    if self.pool.len() < 64 {
                        self.pool.push((std::mem::take(&mut dead.slots), std::mem::take(&mut dead.stack)));
                    }
                    if self.frames.len() == baseline {
                        return v;
                    }
                    self.frames.last_mut().unwrap().stack.push(v);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- layout B

struct FrameB<'m> {
    func: &'m Func,
    ret_ip: usize,
    base: usize,
}

struct VmB<'m> {
    funcs: &'m [Func],
    stack: Vec<Zval>,
    frames: Vec<FrameB<'m>>,
}

impl<'m> VmB<'m> {
    fn run(&mut self, entry: usize, args: Vec<Zval>) -> Zval {
        let mut func: &'m Func = &self.funcs[entry];
        let mut base = self.stack.len();
        self.stack.extend(args);
        self.stack.resize(base + func.n_slots, Zval::Undef);
        let mut ip = 0usize;
        let depth0 = self.frames.len();
        loop {
            let op = func.ops[ip];
            ip += 1;
            match op {
                Op::PushConst(i) => self.stack.push(func.consts[i as usize].clone()),
                Op::LoadSlot(s) => {
                    let v = self.stack[base + s as usize].clone();
                    self.stack.push(v);
                }
                Op::StoreSlot(s) => {
                    let v = self.stack.pop().unwrap();
                    self.stack[base + s as usize] = v;
                }
                Op::AddSC { slot, c } => {
                    let v = add(&self.stack[base + slot as usize], &func.consts[c as usize]);
                    self.stack.push(v);
                }
                Op::CmpJmpLtSS { l, r, addr } => {
                    if !lt(&self.stack[base + l as usize], &self.stack[base + r as usize]) {
                        ip = addr as usize;
                    }
                }
                Op::IncSlotJmp { slot, addr } => {
                    if let Zval::Long(n) = &mut self.stack[base + slot as usize] {
                        *n += 1;
                    }
                    ip = addr as usize;
                }
                Op::Call { func: fi, argc } => {
                    let callee = &self.funcs[fi as usize];
                    if self.frames.len() > 10_000 {
                        panic!("depth");
                    }
                    // The arguments already sit on top of the stack: they
                    // become the callee's first slots in place.
                    let nbase = self.stack.len() - argc as usize;
                    self.stack.resize(nbase + callee.n_slots, Zval::Undef);
                    self.frames.push(FrameB { func, ret_ip: ip, base });
                    func = callee;
                    ip = 0;
                    base = nbase;
                }
                Op::Ret => {
                    let v = self.stack.pop().unwrap_or(Zval::Null);
                    self.stack.truncate(base);
                    if self.frames.len() == depth0 {
                        return v;
                    }
                    let fr = self.frames.pop().unwrap();
                    func = fr.func;
                    ip = fr.ret_ip;
                    base = fr.base;
                    self.stack.push(v);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- layout B2: ops slice and consts in locals

struct FrameB2<'m> {
    func: &'m Func,
    ret_ip: usize,
    base: usize,
}

struct VmB2<'m> {
    funcs: &'m [Func],
    stack: Vec<Zval>,
    frames: Vec<FrameB2<'m>>,
}

impl<'m> VmB2<'m> {
    fn run(&mut self, entry: usize, args: Vec<Zval>) -> Zval {
        let mut func: &'m Func = &self.funcs[entry];
        let mut base = self.stack.len();
        self.stack.extend(args);
        self.stack.resize(base + func.n_slots, Zval::Undef);
        let mut ip = 0usize;
        let mut ops: &'m [Op] = &func.ops;
        let mut consts: &'m [Zval] = &func.consts;
        let depth0 = self.frames.len();
        loop {
            let op = ops[ip];
            ip += 1;
            match op {
                Op::PushConst(i) => self.stack.push(consts[i as usize].clone()),
                Op::LoadSlot(s) => {
                    let v = self.stack[base + s as usize].clone();
                    self.stack.push(v);
                }
                Op::StoreSlot(s) => {
                    let v = self.stack.pop().unwrap();
                    self.stack[base + s as usize] = v;
                }
                Op::AddSC { slot, c } => {
                    let v = add(&self.stack[base + slot as usize], &consts[c as usize]);
                    self.stack.push(v);
                }
                Op::CmpJmpLtSS { l, r, addr } => {
                    if !lt(&self.stack[base + l as usize], &self.stack[base + r as usize]) {
                        ip = addr as usize;
                    }
                }
                Op::IncSlotJmp { slot, addr } => {
                    if let Zval::Long(n) = &mut self.stack[base + slot as usize] {
                        *n += 1;
                    }
                    ip = addr as usize;
                }
                Op::Call { func: fi, argc } => {
                    let callee = &self.funcs[fi as usize];
                    if self.frames.len() > 10_000 {
                        panic!("depth");
                    }
                    // The arguments already sit on top of the stack: they
                    // become the callee's first slots in place.
                    let nbase = self.stack.len() - argc as usize;
                    self.stack.resize(nbase + callee.n_slots, Zval::Undef);
                    self.frames.push(FrameB2 { func, ret_ip: ip, base });
                    func = callee;
                    ops = &callee.ops;
                    consts = &callee.consts;
                    ip = 0;
                    base = nbase;
                }
                Op::Ret => {
                    let v = self.stack.pop().unwrap_or(Zval::Null);
                    self.stack.truncate(base);
                    if self.frames.len() == depth0 {
                        return v;
                    }
                    let fr = self.frames.pop().unwrap();
                    func = fr.func;
                    ops = &func.ops;
                    consts = &func.consts;
                    ip = fr.ret_ip;
                    base = fr.base;
                    self.stack.push(v);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- layout B3 (INFO ONLY, unsafe): B2 with unchecked op/slot access

struct FrameB3<'m> {
    func: &'m Func,
    ret_ip: usize,
    base: usize,
}

struct VmB3<'m> {
    funcs: &'m [Func],
    stack: Vec<Zval>,
    frames: Vec<FrameB3<'m>>,
}

impl<'m> VmB3<'m> {
    fn run(&mut self, entry: usize, args: Vec<Zval>) -> Zval {
        let mut func: &'m Func = &self.funcs[entry];
        let mut base = self.stack.len();
        self.stack.extend(args);
        self.stack.resize(base + func.n_slots, Zval::Undef);
        let mut ip = 0usize;
        let mut ops: &'m [Op] = &func.ops;
        let mut consts: &'m [Zval] = &func.consts;
        let depth0 = self.frames.len();
        loop {
            // SAFETY: the prototype's programs are well-formed (ip < len).
            let op = unsafe { *ops.get_unchecked(ip) };
            ip += 1;
            match op {
                Op::PushConst(i) => self.stack.push(consts[i as usize].clone()),
                Op::LoadSlot(s) => {
                    // SAFETY: slots are within the frame (prototype programs).
                    let v = unsafe { self.stack.get_unchecked(base + s as usize) }.clone();
                    self.stack.push(v);
                }
                Op::StoreSlot(s) => {
                    let v = self.stack.pop().unwrap();
                    self.stack[base + s as usize] = v;
                }
                Op::AddSC { slot, c } => {
                    let v = add(&self.stack[base + slot as usize], &consts[c as usize]);
                    self.stack.push(v);
                }
                Op::CmpJmpLtSS { l, r, addr } => {
                    // SAFETY: as above.
                    let (a, b) = unsafe { (self.stack.get_unchecked(base + l as usize), self.stack.get_unchecked(base + r as usize)) };
                    if !lt(a, b) {
                        ip = addr as usize;
                    }
                }
                Op::IncSlotJmp { slot, addr } => {
                    if let Zval::Long(n) = &mut self.stack[base + slot as usize] {
                        *n += 1;
                    }
                    ip = addr as usize;
                }
                Op::Call { func: fi, argc } => {
                    let callee = &self.funcs[fi as usize];
                    if self.frames.len() > 10_000 {
                        panic!("depth");
                    }
                    // The arguments already sit on top of the stack: they
                    // become the callee's first slots in place.
                    let nbase = self.stack.len() - argc as usize;
                    self.stack.resize(nbase + callee.n_slots, Zval::Undef);
                    self.frames.push(FrameB3 { func, ret_ip: ip, base });
                    func = callee;
                    ops = &callee.ops;
                    consts = &callee.consts;
                    ip = 0;
                    base = nbase;
                }
                Op::Ret => {
                    let v = self.stack.pop().unwrap_or(Zval::Null);
                    self.stack.truncate(base);
                    if self.frames.len() == depth0 {
                        return v;
                    }
                    let fr = self.frames.pop().unwrap();
                    func = fr.func;
                    ops = &func.ops;
                    consts = &func.consts;
                    ip = fr.ret_ip;
                    base = fr.base;
                    self.stack.push(v);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- programs

fn programs() -> Vec<Func> {
    vec![
        // 0: empty loop. slots: 0 = $n, 1 = $i
        Func {
            ops: vec![
                Op::PushConst(0),
                Op::StoreSlot(1),
                Op::CmpJmpLtSS { l: 1, r: 0, addr: 4 },
                Op::IncSlotJmp { slot: 1, addr: 2 },
                Op::PushConst(1),
                Op::Ret,
            ],
            consts: vec![Zval::Long(0), Zval::Null],
            n_slots: 2,
        },
        // 1: f($a) { return $a + 1; }
        Func { ops: vec![Op::AddSC { slot: 0, c: 0 }, Op::Ret], consts: vec![Zval::Long(1)], n_slots: 1 },
        // 2: call loop. slots: 0 = $n, 1 = $i, 2 = $x;  for (...) { $x = f($x); } return $x;
        Func {
            ops: vec![
                Op::PushConst(0),
                Op::StoreSlot(1),
                Op::PushConst(0),
                Op::StoreSlot(2),
                Op::CmpJmpLtSS { l: 1, r: 0, addr: 9 },
                Op::LoadSlot(2),
                Op::Call { func: 1, argc: 1 },
                Op::StoreSlot(2),
                Op::IncSlotJmp { slot: 1, addr: 4 },
                Op::LoadSlot(2),
                Op::Ret,
            ],
            consts: vec![Zval::Long(0)],
            n_slots: 3,
        },
    ]
}

fn time<F: FnMut() -> Zval>(label: &str, n: i64, mut f: F) {
    let t = Instant::now();
    let r = f();
    let ns = t.elapsed().as_nanos() as f64 / n as f64;
    println!("{label:28} {ns:6.2} ns/iter  ({r:?})");
}

fn main() {
    let funcs = programs();
    let n_empty: i64 = std::env::var("N_EMPTY").ok().and_then(|v| v.parse().ok()).unwrap_or(200_000_000);
    let n_call: i64 = std::env::var("N_CALL").ok().and_then(|v| v.parse().ok()).unwrap_or(30_000_000);
    for _ in 0..2 {
        let mut a = VmA { funcs: &funcs, frames: Vec::new(), pool: Vec::new() };
        time("A empty loop", n_empty, || a.run(0, vec![Zval::Long(n_empty)]));
        time("A call loop", n_call, || a.run(2, vec![Zval::Long(n_call)]));
        let mut b = VmB { funcs: &funcs, stack: Vec::with_capacity(1024), frames: Vec::new() };
        time("B empty loop", n_empty, || b.run(0, vec![Zval::Long(n_empty)]));
        time("B call loop", n_call, || b.run(2, vec![Zval::Long(n_call)]));
        let mut b2 = VmB2 { funcs: &funcs, stack: Vec::with_capacity(1024), frames: Vec::new() };
        time("B2 empty loop", n_empty, || b2.run(0, vec![Zval::Long(n_empty)]));
        time("B2 call loop", n_call, || b2.run(2, vec![Zval::Long(n_call)]));
        let mut b3 = VmB3 { funcs: &funcs, stack: Vec::with_capacity(1024), frames: Vec::new() };
        time("B3 empty loop (unsafe)", n_empty, || b3.run(0, vec![Zval::Long(n_empty)]));
        time("B3 call loop (unsafe)", n_call, || b3.run(2, vec![Zval::Long(n_call)]));
    }
}
