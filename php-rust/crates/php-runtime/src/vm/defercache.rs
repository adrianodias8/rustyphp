//! Cross-request cache of deferred (late-bound) class declarations.
//!
//! A defer-always unit declares each class through `Vm::run_deferred`, which
//! re-lowers the declaration's snippet against the live class image. The
//! include cache keeps whole units; this keeps each deferred snippet's
//! relocated module in the same `UNIT_CACHE` (under a synthetic key), valid
//! for the VM fingerprint it was lowered under, plus the negative result (a
//! supertype still missing) so the next request goes straight to autoload.

use super::*;

impl<'m> super::Vm<'m> {
    /// Replay a cached deferred declaration: the same double-check as an
    /// include hit (append bases, class remap), then the seed delta and the
    /// linked run. `None` = no usable entry (the caller lowers afresh).
    pub(super) fn defer_cache_hit(
        &mut self,
        dk: &UnitKey,
        fp: u64,
        expr: bool,
        caller: usize,
    ) -> Option<Result<Zval, PhpError>> {
        let cu = unit_cache_get(dk, fp).filter(|cu| cu.main_program.is_none())?;
        let remap_base = (self.classes.len(), self.statics.len());
        let (remap, locals) = if cu.module.elided.is_some() {
            self.unit_class_remap_retained(&cu.module)
        } else {
            self.unit_class_remap(&cu.module)
        };
        if cu.static_off != remap_base.1
            || cu.reserved_base != remap_base.0
            || remap != cu.class_remap
            || locals != cu.new_locals
        {
            return None;
        }
        self.apply_seed_delta(&cu.seed_delta);
        uc_log_flush();
        let parked = self.park_module(cu.module);
        Some(self.run_linked(parked, &locals, None, if expr { Some(caller) } else { None }, remap_base, None))
    }

    /// The miss half of the deferred-declaration cache: relocate the fresh
    /// module (as `drive_unit` does), publish it under `fp`, then run it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn defer_link_publish(
        &mut self,
        dk: UnitKey,
        fp: u64,
        mut module: Module,
        program: &Program,
        elide: Option<usize>,
        seed_delta: Rc<SeedDelta>,
        bridge: Option<usize>,
    ) -> Result<Zval, PhpError> {
        let static_off = self.statics.len();
        let reserved_base = self.classes.len();
        let (class_remap, new_locals) = match (module.elided, elide) {
            (Some(_), Some(seed_len)) => {
                let (prog_remap, retained_remap, locals) = self.unit_remap_elided(program, seed_len);
                debug_assert_eq!(retained_remap.len(), module.classes.len());
                assert_fold_equals_mint(&seed_delta, &locals, &module, &dk.path);
                relocate_module_class_ids(&mut module, &prog_remap, static_off);
                (retained_remap, locals)
            }
            _ => {
                let (remap, locals) = self.unit_class_remap(&module);
                assert_fold_equals_mint(&seed_delta, &locals, &module, &dk.path);
                relocate_module_class_ids(&mut module, &remap, static_off);
                (remap, locals)
            }
        };
        uc_stat(|s| s.parked_modules += 1);
        let rc = Rc::new(module);
        unit_cache_put(
            dk,
            CachedUnit {
                fp,
                static_off,
                reserved_base,
                class_remap,
                new_locals: new_locals.clone(),
                seed_delta,
                module: Rc::clone(&rc),
                owner_epoch: VM_EPOCH.with(|e| e.get()),
                main_program: None,
                main_program_net: 0,
                main_program_net_clamped: false,
                main_put_ordinal: 0,
            },
        );
        let parked: &'m Module = self.park_module(rc);
        uc_log_flush();
        self.run_linked(parked, &new_locals, None, bridge, (reserved_base, static_off), None)
    }

}

thread_local! {
    /// Deferred declarations whose lowering under a fingerprint stopped on a
    /// missing supertype: (key, fp) -> the UndefinedClass it raised.
    static DEFER_NEG: RefCell<HashMap<(UnitKey, u64), (Box<[u8]>, crate::MissingSym, Line)>> =
        RefCell::new(HashMap::default());
}

pub(super) fn defer_neg_get(dk: &UnitKey, fp: u64) -> Option<(Box<[u8]>, crate::MissingSym, Line)> {
    DEFER_NEG.with(|m| m.borrow().get(&(dk.clone(), fp)).cloned())
}

pub(super) fn defer_neg_put(dk: &UnitKey, fp: u64, e: (Box<[u8]>, crate::MissingSym, Line)) {
    DEFER_NEG.with(|m| {
        let mut m = m.borrow_mut();
        // Bounded like the unit cache's ways: a run that keeps minting new
        // fingerprints starts over rather than growing without limit.
        if m.len() >= 1 << 16 {
            m.clear();
        }
        m.insert((dk.clone(), fp), e);
    })
}

/// Unit-cache key of a deferred declaration: the enclosing file, the line,
/// and the snippet's length and 64-bit digest (no mtime: an edited file
/// yields a different snippet).
pub(super) fn defer_unit_key(file: &[u8], line: Line, snippet: &[u8], digest: u64) -> UnitKey {
    let mut path = Vec::with_capacity(file.len() + 24);
    path.extend_from_slice(b"\0defer\0");
    path.extend_from_slice(file);
    path.push(0);
    path.extend_from_slice(&line.to_le_bytes());
    path.extend_from_slice(&digest.to_le_bytes());
    UnitKey { path, mtime: (0, 0), size: snippet.len() as u64, reg_mode: crate::compile::reg_lower::enabled() }
}

