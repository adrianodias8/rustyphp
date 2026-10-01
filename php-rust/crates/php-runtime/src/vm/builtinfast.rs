//! Lean call of a pure value builtin (`is_int`, `strlen`, `substr`, …).
//!
//! `value_builtin_call` first walks a chain of per-name special cases
//! (`var_dump`'s `__debugInfo`, Countable `count`, user stream wrappers,
//! `__toString` precomputes for string-coercing builtins), then
//! `run_value_builtin` flushes diagnostics, sets up the output sinks, calls,
//! flushes again and writes output. For a builtin none of those cases can
//! touch — not in any special list, no pending diagnostic, and no argument
//! that is an object, closure or reference (nor an array, for the
//! string-coercing ones, whose precompute recurses into elements) — every
//! step but the call itself is a no-op. Bench bcall.php: two such calls per
//! iteration cost 127 ns vs PHP's 15.
//!
//! After the call, anything the builtin produced (output, a diagnostic) goes
//! through the same flush/write steps, in the same order.

use super::*;

/// Names whose call `value_builtin_call` special-cases before (or instead
/// of) the plain builtin — the lean path never takes them.
fn value_builtin_special(name: &[u8]) -> bool {
    matches!(
        name,
        b"var_export"
            | b"print_r"
            | b"var_dump"
            | b"file_get_contents"
            | b"file_put_contents"
            | b"chmod"
            | b"touch"
            | b"chown"
            | b"chgrp"
            | b"unlink"
            | b"rename"
            | b"mkdir"
            | b"rmdir"
    ) || super::run::is_user_stream_op(name)
        || super::run::is_user_wrapper_path_op(name)
}

impl<'m> super::Vm<'m> {
    /// `Some(result)` when the lean path applied; `None` leaves everything
    /// untouched for the full `value_builtin_call`.
    pub(super) fn value_builtin_lean(
        &mut self,
        top: usize,
        f: crate::builtin::BuiltinFn,
        name: &[u8],
        args: &[Zval],
    ) -> Option<Result<Zval, PhpError>> {
        if self.diags_rendered != self.diags.len() || value_builtin_special(name) {
            return None;
        }
        let coerces = super::calls::value_builtin_string_coerces(name)
            || super::calls::value_builtin_string_coerces_deep(name, args);
        for a in args {
            match a {
                Zval::Undef | Zval::Null | Zval::Bool(_) | Zval::Long(_) | Zval::Double(_) | Zval::Str(_) => {}
                Zval::Array(_) if !coerces => {}
                _ => return None,
            }
        }
        let mut produced = Vec::new();
        let mut direct = Vec::new();
        let none_dbg = std::collections::HashMap::new();
        let none_str = std::collections::HashMap::new();
        let res = {
            let mut ctx = Ctx {
                out: &mut produced,
                diags: &mut self.diags,
                direct_out: &mut direct,
                debug_info: &none_dbg,
                stringify: &none_str,
            };
            f(args, &mut ctx)
        };
        if !produced.is_empty() || !direct.is_empty() || self.diags_rendered != self.diags.len() {
            let line = self.cur_line(top);
            if let Err(e) = self.flush_diags(line).and_then(|_| self.write_output(&produced)) {
                return Some(Err(e));
            }
            if !direct.is_empty() {
                self.stdout.extend_from_slice(&direct);
                self.rendered.extend_from_slice(&direct);
            }
        }
        Some(res)
    }
}
