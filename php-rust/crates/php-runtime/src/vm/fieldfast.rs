//! Single-walk fast path for `isset()` on a field path (`Op::FieldIsset`).
//!
//! The full handler runs up to five walks of the path — the magic probe,
//! the ArrayAccess leaf and nested checks (two `field_value` walks each), the
//! lazy-root check, then the read — so that `__isset`/`__get`, ArrayAccess
//! and lazy objects get their protocols. Op-time census on a warm Drupal
//! front page: 6,419 `FieldIsset` at 322 ns each, 11 % of the request.
//!
//! This walk follows the magic probe's per-step semantics (one silent
//! `field_get` hop per step) and returns `None` — having only read — the
//! moment any of those protocols could apply: a lazy or proxy object, a
//! property where `__isset`/`__get` would be dispatched, an object at an
//! `Index` step (ArrayAccess decides there), or a dynamic/append step. The
//! caller then runs the unchanged full sequence.

use super::*;

impl<'m> super::Vm<'m> {
    pub(super) fn field_isset_fast(
        &self,
        base: FieldBase,
        top: usize,
        steps: &[FieldStep],
        keys: &[Zval],
    ) -> Option<bool> {
        if steps.iter().any(|s| matches!(s, FieldStep::PropDyn | FieldStep::Append)) {
            return None;
        }
        let cur = self.frames[top].class;
        let mut v = self.base_field_cell(base, top)?.deref_clone();
        let mut ks = keys.to_vec().into_iter();
        for step in steps {
            match step {
                FieldStep::Prop(n) => {
                    let Some(o) = deref_object(&v) else {
                        // A property of a non-object reads as missing on
                        // every path.
                        return Some(false);
                    };
                    {
                        let b = o.borrow();
                        if b.lazy.is_some() || b.proxy_instance.is_some() {
                            return None;
                        }
                    }
                    if self.magic_applies(&o, n, cur, MagicKind::Isset, b"__isset").is_some()
                        || self.magic_applies(&o, n, cur, MagicKind::Get, b"__get").is_some()
                    {
                        return None;
                    }
                }
                FieldStep::Index => {
                    if deref_object(&v).is_some() {
                        return None;
                    }
                }
                FieldStep::PropDyn | FieldStep::Append => return None,
            }
            let fs = FieldScope { classes: &self.classes, scope: cur };
            match field_get(&v, std::slice::from_ref(step), &mut ks, fs) {
                Some(next) => v = next,
                None => return Some(false),
            }
        }
        Some(!matches!(v, Zval::Null | Zval::Undef))
    }
}

impl<'m> super::Vm<'m> {
    /// The whole `Op::FieldIsset` handler, out of line: `run_loop` is
    /// layout-sensitive, and growing this arm in place (the fast path) cost
    /// 3-8 % on unrelated ops (bisected, NOTES.md session 8). The arm is now
    /// one call; the fast path runs first, then the full protocol sequence.
    #[inline(never)]
    pub(super) fn field_isset_op(
        &mut self,
        top: usize,
        base: FieldBase,
        steps: &[FieldStep],
    ) -> Result<bool, PhpError> {
        let keys = self.pop_field_keys(top, steps);
        if let Some(set) = self.field_isset_fast(base, top, steps, &keys) {
            return Ok(set);
        }
        // Magic protocol at ANY property step along the path
        // (`isset($o->magic['k'])`, gh18038 / bug40833; and a magic
        // leaf one hop in: `isset($block->block_type->uses_context)`,
        // WP_Block_Type) — see field_magic_probe.
        if let Some(set) = self.field_magic_probe(base, top, steps, &keys, false)? {
            return Ok(set);
        }
        // A final Index on an ArrayAccess object is the protocol:
        // `isset($this->coll[0])` = offsetExists (no offsetGet),
        // mirroring Op::IssetPath's single-step arm.
        if let Some((recv, key)) = self.field_aa_leaf(base, top, steps, &keys) {
            let r = self.call_method_sync(recv, b"offsetExists", vec![key])?;
            let set = convert::is_true_silent(&r.deref_clone());
            return Ok(set);
        }
        // Nested Index run on an ArrayAccess property
        // (`isset($this->data['a']['b'])`): BP_VAR_IS walk.
        if let Some(res) =
            self.field_aa_walk(base, top, steps, &keys, super::IsMode::Exists)?
        {
            let set = match res {
                super::DimIsLeaf::Missing => false,
                super::DimIsLeaf::Aa(recv, key) => {
                    let r =
                        self.call_method_sync(recv, b"offsetExists", vec![key])?;
                    convert::is_true_silent(&r.deref_clone())
                }
                super::DimIsLeaf::Verdict(set) => set,
            };
            return Ok(set);
        }
        // A lazy base initializes and the walk roots at the realized
        // object (isset through a wrapper reads the instance).
        if let Some(root) = self.field_lazy_root(base, top, steps, &keys, false)? {
            let fs = FieldScope { classes: &self.classes, scope: self.frames[top].class };
            let set = matches!(
                field_get(&root, steps, &mut keys.into_iter(), fs),
                Some(v) if !matches!(v, Zval::Null | Zval::Undef)
            );
            return Ok(set);
        }
        let set = matches!(
            self.field_value(base, top, steps, keys),
            Some(v) if !matches!(v, Zval::Null | Zval::Undef)
        );
        Ok(set)
    }
}
