//! Drop-driven destructors (`PHPR_GC=drop`): the refcount itself reports an
//! object's death, as in Zend, instead of the VM polling for it.
//!
//! The classic path keeps a STRONG clone of every object in `created`, so no
//! object ever dies on its own: every release site notes the value, every
//! statement sweeps the noted candidates, and a candidate whose count fell
//! to the registry's own reference is destructed and cascaded. On a Drupal
//! request that machinery (`Ret`'s frame notes, `Sweep`, the sweep bodies)
//! was ~18 % of the time.
//!
//! Here the registry holds `Weak`s. An object without a destructor frees
//! the moment its last reference drops (`Object::drop`, which already
//! releases handle ids in Zend's postorder). An object whose class has a
//! `__destruct` that has not run yet is resurrected by `Object::drop` under
//! its own handle id and queued (`php_types::pop_pending_dtor`); the VM runs
//! queued destructors at the same safe points the sweep used (the
//! statement-boundary `Op::Sweep`), so destructor timing keeps its
//! granularity. Cycles are what refcounting cannot see: a collection takes
//! a strong snapshot of the weak registry into `created`, offers every live
//! object as a root, and runs the existing trial-deletion collector
//! unchanged — the snapshot is released afterwards. (Designs compared:
//! Bacon–Rajan synchronous cycle collection as in `bacon_rajan_cc`, CPython
//! / `gcmodule`'s refcount + cycle detector, and `rust-cc`'s finalizers with
//! resurrection; this is the last shape on top of the first.)

use super::*;

/// Possible cycle roots (objects that lost a holder but stay alive — Zend's
/// `gc_possible_root`) that trigger an automatic collection; raised by the
/// same step when a collection frees little (Zend's `gc_adjust_threshold`).
/// The classic trigger's 50k (`Vm::GC_CYCLE_THRESHOLD`, see its note on
/// synthetic 10k-root tests such as gh20657-002). Counting registry growth
/// instead re-classified every large acyclic structure (json_decode of a
/// 30k-object tree, x1.3).
const DROP_GC_GROWTH: usize = 50_000;

/// Drop-driven destructors are the default; `PHPR_GC=classic` selects the
/// note/sweep engine.
pub(super) fn gcdrop_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("PHPR_GC").map_or(true, |v| v != "classic"))
}

impl<'m> super::Vm<'m> {
    /// Highest registered object id (the mark `discard_objects_after` cuts at).
    pub(super) fn last_object_id(&self) -> Option<u32> {
        if self.gc_drop {
            self.weak_reg.last_key_value().map(|(id, _)| *id)
        } else {
            self.created.last_key_value().map(|(id, _)| *id)
        }
    }

    /// Discard the objects registered after `mark` without running their
    /// destructors (`get_class_vars`' throwaway instance and what its
    /// prop-init thunk minted): classic drops their store entries; drop
    /// mode marks them destructed so their release frees them unqueued.
    pub(super) fn discard_objects_after(&mut self, mark: Option<u32>) {
        if !self.gc_drop {
            match mark {
                Some(m) => drop(self.created.split_off(&(m + 1))),
                None => self.created.clear(),
            }
            return;
        }
        let tail = match mark {
            Some(m) => self.weak_reg.split_off(&(m + 1)),
            None => std::mem::take(&mut self.weak_reg),
        };
        for w in tail.values() {
            if let Some(o) = w.upgrade() {
                o.borrow().gc.set_destructed(true);
            }
        }
    }

    /// Register a freshly minted object (every allocation site): the classic
    /// strong store + birth seed, or the weak registry + destructor flag.
    pub(super) fn track_object(&mut self, rc: &Rc<RefCell<Object>>) {
        if !self.gc_drop {
            let id = rc.borrow().id;
            self.created.insert(id, Rc::clone(rc));
            self.gc_track(rc);
            return;
        }
        let (id, cid) = {
            let b = rc.borrow();
            (b.id, b.class_id as usize)
        };
        if self.class_has_dtor(cid) {
            rc.borrow().gc.set_has_dtor(true);
        }
        self.weak_reg.insert(id, Rc::downgrade(rc));
    }

    /// A note on a live object in drop mode: count it once as a possible
    /// cycle root (its `cycle_root` bit dedups until the next collection).
    pub(super) fn gc_drop_possible_root(&mut self, rc: &Rc<RefCell<Object>>) {
        if Rc::strong_count(rc) <= 1 {
            return;
        }
        let b = rc.borrow();
        if b.gc.cycle_root() || b.gc.destructed() {
            return;
        }
        b.gc.set_cycle_root(true);
        drop(b);
        self.drop_roots += 1;
        if self.drop_roots >= self.drop_gc_next {
            // Collected at the next safe point (the sweep path).
            self.gc_idle_set([false; 2]);
        }
    }

    /// Whether instances of `cid` have a `__destruct` (memoized per class).
    fn class_has_dtor(&mut self, cid: ClassId) -> bool {
        if cid >= self.dtor_class.len() {
            self.dtor_class.resize(cid + 1, 0);
        }
        match self.dtor_class[cid] {
            1 => false,
            2 => true,
            _ => {
                let has = resolve_method_runtime(&self.classes, cid, b"__destruct").is_some();
                self.dtor_class[cid] = if has { 2 } else { 1 };
                has
            }
        }
    }

    /// The sweep entry in drop mode: run every queued destructor (in release
    /// order), then an automatic cycle collection if the registry grew past
    /// its trigger. `resume` is the sweep's own: a destructor started from
    /// `Op::Sweep` runs as a pushed frame and the sweep re-runs after it.
    pub(super) fn gc_drop_sweep(&mut self, resume: Option<(usize, usize)>) -> Result<(), PhpError> {
        php_types::release_quarantined_ids();
        while let Some(o) = php_types::pop_pending_dtor() {
            if self.run_pending_dtor(o, resume)? {
                return Ok(());
            }
        }
        if self.drop_roots >= self.drop_gc_next && self.gc_enabled && !self.gc_collecting {
            let freed = self.collect_cycles()?;
            self.weak_reg.retain(|_, w| w.strong_count() > 0);
            self.drop_gc_next = if freed < Self::GC_ADJUST_TRIGGER {
                self.drop_gc_next.saturating_add(DROP_GC_GROWTH)
            } else {
                DROP_GC_GROWTH
            };
        }
        Ok(())
    }

    /// Run one queued object's destructor. `true` when it was pushed as a
    /// frame (resume mode) — the caller stops and the sweep re-runs later.
    fn run_pending_dtor(&mut self, o: Rc<RefCell<Object>>, resume: Option<(usize, usize)>) -> Result<bool, PhpError> {
        let (id, cid) = {
            let b = o.borrow();
            (b.id, b.class_id as usize)
        };
        // The resurrected allocation replaces the dead one in the registry.
        self.weak_reg.insert(id, Rc::downgrade(&o));
        self.destructed.insert(id);
        o.borrow().gc.set_destructed(true);
        let Some((defc, midx)) = resolve_method_runtime(&self.classes, cid, b"__destruct") else {
            return Ok(false);
        };
        let callee = &self.classes[defc].methods[midx].func;
        let mut frame = Frame::new(callee, self.class_mod(defc));
        frame.this = Some(Zval::Object(o));
        frame.class = Some(defc);
        frame.static_class = Some(cid);
        frame.flags.set(FrameFlags::IN_DESTRUCTOR, true);
        match resume {
            Some((top, ip)) => {
                frame.ret_cell = Some(php_types::zcell(Zval::Null));
                self.frames[top].ip = ip; // re-run Sweep after it returns
                self.frames.push(frame);
                Ok(true)
            }
            None => {
                let baseline = self.frames.len();
                self.frames.push(frame);
                let _ = self.drive_to_return(baseline)?;
                Ok(false)
            }
        }
    }

    /// Before a cycle collection in drop mode: every live registered object
    /// becomes a strong `created` entry and a cycle root, so the classic
    /// collector sees exactly the graph it was written for.
    pub(super) fn gc_drop_seed_collect(&mut self) {
        let live: Vec<(u32, Rc<RefCell<Object>>)> =
            self.weak_reg.iter().filter_map(|(id, w)| w.upgrade().map(|o| (*id, o))).collect();
        for (id, o) in live {
            // Every live object is a root (the `cycle_root` bit may already be
            // set by the possible-root count, which does not use the set).
            o.borrow().gc.set_cycle_root(true);
            self.gc_cycle_roots.insert(id);
            self.created.insert(id, o);
        }
    }

    /// After a drop-mode collection: everything still in the classic store
    /// (survivors, and objects the collection's destructors minted) moves
    /// back to the weak registry, the classic marks and buffers are cleared,
    /// and the strong snapshot is released — an object whose last reference
    /// it was dies through `Object::drop` like any other.
    pub(super) fn gc_drop_release_snapshot(&mut self) {
        let snap = std::mem::take(&mut self.created);
        for slot in self.gc_buf.drain(..).flatten() {
            slot.borrow().gc.clear();
        }
        self.gc_buf_head = 0;
        for (id, o) in &snap {
            let cid = {
                let b = o.borrow();
                b.gc.clear();
                b.gc.set_cycle_root(false);
                b.gc.set_light_demoted(false);
                b.class_id as usize
            };
            if self.class_has_dtor(cid) {
                o.borrow().gc.set_has_dtor(true);
            }
            self.weak_reg.insert(*id, Rc::downgrade(o));
        }
        self.gc_cycle_roots.clear();
        self.gc_light_demoted.clear();
        self.drop_roots = 0;
        drop(snap);
    }

    /// Drop mode at script end: run what is queued, then hand every live
    /// registered object to the classic store so the shutdown destructor
    /// walk (creation order) and the cycle break see them.
    pub(super) fn gc_drop_shutdown_snapshot(&mut self) -> Result<(), PhpError> {
        while let Some(o) = php_types::pop_pending_dtor() {
            self.run_pending_dtor(o, None)?;
        }
        let live: Vec<(u32, Rc<RefCell<Object>>)> =
            self.weak_reg.iter().filter_map(|(id, w)| w.upgrade().map(|o| (*id, o))).collect();
        self.weak_reg.clear();
        for (id, o) in live {
            self.created.insert(id, o);
        }
        Ok(())
    }

    /// Live registered objects (drop mode's `created.len()`).
    pub(super) fn gc_drop_live(&self) -> usize {
        self.weak_reg.values().filter(|w| w.strong_count() > 0).count()
    }
}

/// Discard whatever is still queued for a destructor (a request ending
/// without draining, a VM torn down mid-walk): marked destructed, so the
/// release frees them instead of queueing them again.
pub(super) fn discard_pending_dtors() {
    while let Some(o) = php_types::pop_pending_dtor() {
        o.borrow().gc.set_destructed(true);
    }
}
