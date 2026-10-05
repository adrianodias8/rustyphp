//! Name interning at emission (PARITY_PLAN.md stage 1, item 1): the names an
//! op carries share one allocation per thread with the class tables' keys
//! ([`php_types::NameTable`]), so the runtime finds them by pointer.

use crate::bytecode::Op;

/// Replace the property or method name an op carries by its interned
/// allocation.
pub(super) fn intern_op_names(op: &mut Op) {
    let name = match op {
        Op::MethodCall { method, .. }
        | Op::ThisMethodCall { method, .. }
        | Op::MethodCallArgs { method, .. }
        | Op::MethodCallNamed { method, .. }
        | Op::StaticCall { method, .. }
        | Op::StaticCallArgs { method, .. }
        | Op::ClosureStatic { method, .. }
        | Op::StaticCallDynamic { method, .. }
        | Op::StaticCallDynamicArgs { method } => method,
        Op::PropGet { name, .. }
        | Op::ThisPropGet { name, .. }
        | Op::PropSet { name, .. }
        | Op::PropOpSet { name, .. }
        | Op::PropIncDec { name, .. }
        | Op::PropIsset { name, .. }
        | Op::PropIssetFetchGate { name }
        | Op::PropGetSilent { name }
        | Op::PropUnset { name }
        | Op::PropConcatGate { name, .. }
        | Op::PropGetSlot { name, .. }
        | Op::PropDimGetConst { name, .. }
        | Op::PropSetPop { name, .. }
        | Op::PropGetSlotRecv { name, .. }
        | Op::BinaryTCPropSetPop { name, .. } => name,
        _ => return,
    };
    *name = php_types::intern_rc(name);
}
