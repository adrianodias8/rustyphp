//! Call and return without moving the ~180-byte `Frame` by value
//! (PARITY_PLAN.md stage 1, item 2). Zend builds each call frame in place on
//! its VM stack (Zend/zend_compile.h:625-635) and releases its CVs in place
//! (`i_free_compiled_variables`, Zend/zend_execute.c:4271-4280).

use super::*;

impl<'m> super::Vm<'m> {
    /// `Ret` of a frame with no iterators, cold extension or dynamic
    /// variables (every plain function and method): the GC notes of
    /// [`Vm::gc_note_frame`] (slots, operand stack, `$this`), then
    /// [`Vm::recycle_frame`]'s release order (slots, operand stack, `$this`,
    /// the residual fields by `truncate`), buffers back to the pool.
    #[inline]
    pub(super) fn ret_in_place(&mut self, top: usize) {
        let mut slots = std::mem::take(&mut self.frames[top].slots);
        let mut stack = std::mem::take(&mut self.frames[top].stack);
        for v in &slots {
            self.gc_note(v);
        }
        for v in &stack {
            self.gc_note(v);
        }
        let this = self.frames[top].this.take();
        if let Some(t) = &this {
            self.gc_note_this(t);
        }
        slots.clear();
        stack.clear();
        drop(this);
        self.frames.truncate(top);
        self.frame_pool.put(slots, stack);
    }

    /// The method fast path's frame entry (`methodcall_fast`, no deferred
    /// argument places): the callee frame is born inside `frames`, the
    /// arguments move from the caller's operand stack into its leading slots
    /// and the receiver into `$this`, in the order of the by-value path (pops
    /// in reverse, receiver, then the decays front to back).
    pub(super) fn methodcall_enter_in_place(
        &mut self,
        top: usize,
        n: usize,
        callee: &'m Func,
        m: &'m Module,
        (defc, cid): (ClassId, ClassId),
        deref: bool,
    ) {
        let (slots, stack) = self.frame_pool.take();
        self.frames.push(Frame::with_buffers(callee, m, slots, stack));
        let (caller, entered) = self.frames.split_at_mut(top + 1);
        let cf = &mut entered[0];
        // Value-context copy of a `&m()` return (WP-53).
        if deref && callee.by_ref && !callee.is_generator {
            cf.flags.set(FrameFlags::RET_DEREF, true);
        }
        #[cfg(feature = "mem-census")]
        php_types::memcensus::arity_note(n);
        cf.argc = n as u32;
        for i in (0..n).rev() {
            let a = caller[top].stack.pop().expect("MethodCall argument");
            zset(&mut cf.slots[i], a);
        }
        let recv = caller[top].stack.pop().expect("MethodCall receiver");
        for i in 0..n {
            let a = std::mem::replace(&mut cf.slots[i], Zval::Undef);
            zset(&mut cf.slots[i], decay_arg(a));
        }
        cf.this = Some(recv);
        cf.class = Some(defc);
        cf.static_class = Some(cid); // LSB = the receiver's class
        // Exact arity by admission ⇒ the `CheckArity` at the head of the
        // body cannot fail (argc == n_params ≥ required): enter at ip=1.
        if matches!(callee.ops.first(), Some(Op::CheckArity { .. })) {
            cf.ip = 1;
        }
        if log::log_enabled!(target: "phpr::call", log::Level::Trace) {
            log::trace!(
                target: "phpr::call",
                "enter {}() (depth {})",
                String::from_utf8_lossy(&callee.name),
                self.frames.len()
            );
        }
    }

    /// `Ret`'s declared-return-type check (`RS_HINT`): a value that already
    /// satisfies the hint passes untouched; otherwise the function's own
    /// unit's strict mode governs the coercion or the TypeError.
    #[inline(never)]
    pub(super) fn ret_hint_check(&mut self, top: usize, func: &'m Func, ret: Zval) -> Result<Zval, PhpError> {
        let hint = func.ret_hint.as_ref().expect("RS_HINT implies ret_hint");
        if hint_accepts_as_is(&self.classes, &ret, hint) {
            return Ok(ret);
        }
        let strict = self.frames[top].module.strict;
        match self.coerce_or_check_hint(ret, hint, strict) {
            Ok(c) => Ok(c),
            Err(given) => Err(self.return_type_error(func, hint, &given)),
        }
    }
}

/// Whether `v` satisfies `hint` as it is: [`Vm::coerce_or_check_hint`] would
/// return it unchanged, with no diagnostic and no user code run (Zend's
/// `ZEND_TYPE_CONTAINS_CODE` test before `zend_verify_arg_type`'s slow path).
/// Conservative: `false` only sends the caller to the full check.
#[inline]
pub(super) fn hint_accepts_as_is(classes: &[&CompiledClass], v: &Zval, hint: &crate::hir::TypeHint) -> bool {
    use crate::hir::{HintKind, ScalarType};
    match (v, &hint.kind) {
        (Zval::Null, _) => hint.nullable,
        (Zval::Long(_), HintKind::Scalar(ScalarType::Int))
        | (Zval::Double(_), HintKind::Scalar(ScalarType::Float))
        | (Zval::Str(_), HintKind::Scalar(ScalarType::String))
        | (Zval::Bool(_), HintKind::Scalar(ScalarType::Bool))
        | (Zval::Array(_), HintKind::Array)
        | (Zval::Object(_) | Zval::Closure(_) | Zval::Generator(_), HintKind::Object) => true,
        (Zval::Object(o), HintKind::Class(name)) => {
            classes.get(o.borrow().class_id as usize).is_some_and(|c| *c.name == **name)
        }
        _ => false,
    }
}
