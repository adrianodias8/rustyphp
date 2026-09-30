| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 9.4 |
| `RefCell` borrow / borrow_mut checks | 0.3 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 1.2 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 2.4 |
| string hashing / comparison / copying / formatting | 0.4 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 52.2 |
| argument passing into builtins (lookup by name, pre-call checks) | 0.7 |
| the builtin bodies themselves | 0.0 |
| parser / HIR / compile (script + prelude + include units), per run | 1.0 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 18.5 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 12.3 |
| other: GC bookkeeping and the per-statement destructor sweep | 1.0 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 0.3 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 0.0 |
| other: process startup, libc, std I/O | 0.2 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 9.4%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 5.8%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 1.7%
  - `core::ptr::drop_glue::<alloc::vec::Vec<php_types::zval::Zval>>` 0.6%
  - `core::ptr::drop_glue::<core::option::Option<php_types::zval::Zval>>` 0.2%
  - `<php_types::zstr::ZStr as core::ops::drop::Drop>::drop` 0.2%
  - `<alloc::rc::Rc<php_types::array::PhpArray>>::make_mut` 0.2%
  - `<alloc::rc::RcInner<php_types::array::PhpArray> as alloc::rc::RcInnerPtr>::dec_strong` 0.1%
  - `<alloc::rc::RcInner<php_types::array::PhpArray> as alloc::rc::RcInnerPtr>::strong` 0.1%
- **refcell** 0.3%
  - `<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::object::Object>>> as core::ops::try_trait::Try>::b...` 0.1%
  - `core::ptr::drop_glue::<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::zval::Zval>>>>` 0.1%
- **alloc** 1.2%
  - `(kernel) page fault / mm: el0_da` 0.4%
  - `(kernel) page fault / mm: do_mem_abort` 0.1%
  - `<alloc::raw_vec::RawVecInner>::reserve` 0.1%
  - `_mi_unchecked_ptr_page` 0.1%
  - `<alloc::raw_vec::RawVecInner>::current_memory` 0.1%
  - `mi_free` 0.1%
  - `<alloc::raw_vec::RawVecInner>::finish_grow` 0.1%
  - `mi_page_xthread_id` 0.1%
- **hash** 2.4%
  - `<php_types::array::PhpArray>::get` 0.8%
  - `<php_types::array::KeyIndex>::lookup` 0.6%
  - `<php_types::array::PhpArray>::set_returning_displaced` 0.5%
  - `<php_types::array::PhpArray>::insert` 0.1%
  - `<php_types::array::KeyIndex>::rebuild` 0.1%
  - `<php_types::array::Key as core::cmp::PartialEq>::eq` 0.1%
  - `<php_types::array::KeyIndex>::raw_insert` 0.1%
- **string** 0.4%
  - `memcmp` 0.1%
  - `bcmp@plt` 0.1%
  - `<[u8] as core::cmp::PartialEq<[u8` 0.1%
- **dispatch** 52.2%
  - `<php_runtime::vm::Vm>::run_loop` 18.7%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 5.2%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 4.0%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 2.3%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 2.3%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 2.3%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::capacity` 2.2%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::pop` 1.6%
- **builtin_args** 0.7%
  - `<php_runtime::vm::Vm>::run_value_builtin` 0.3%
  - `<php_runtime::vm::Vm>::value_builtin_call` 0.1%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_runtime::builtin::Builtin)>>::len` 0.1%
  - `<php_runtime::vm::Vm>::compute_stringify` 0.1%
  - `<core::slice::iter::Iter<php_types::zval::Zval> as core::iter::traits::iterator::Iterator>::any::<<php_runtime::vm::V...` 0.1%
- **compile** 1.0%
  - `under php_runtime::lower::lower_source` 0.3%
  - `under php_runtime::compile::compile_program` 0.2%
  - `under php_runtime::lower::lower_source_impl` 0.1%
  - `under mago_syntax::parser::parse_file_content` 0.1%
  - `under <php_runtime::lower::lower_prelude_uncached as core::ops::function::FnOnce<()>>::call_once` 0.1%
- **operators** 18.5%
  - `<php_runtime::vm::Vm>::binary_value_ab` 5.1%
  - `php_runtime::vm::run::binary_fast` 3.1%
  - `php_runtime::vm::apply_binop` 1.6%
  - `php_types::ops::binop_nums` 1.5%
  - `php_types::ops::try_to_number` 1.0%
  - `<php_runtime::vm::Vm>::binary_value_ab::{closure#0}` 1.0%
  - `php_types::convert::to_bool` 1.0%
  - `<php_runtime::vm::Vm>::apply_binop_ovl` 0.7%
- **vm_body** 12.3%
  - `php_runtime::vm::path_apply` 4.9%
  - `php_runtime::vm::oop::deref_object` 2.4%
  - `<php_runtime::vm::Vm>::path_op` 1.1%
  - `php_runtime::vm::path_walk` 0.8%
  - `php_runtime::vm::apply_last` 0.8%
  - `php_runtime::vm::arrays::coerce_key_silent` 0.5%
  - `php_runtime::vm::arrays::read_slot` 0.5%
  - `<php_runtime::vm::Vm>::write_output` 0.3%
- **gc** 1.0%
  - `<php_runtime::vm::Vm>::gc_note_frame` 0.4%
  - `<php_types::zval::Zval>::is_gc_container` 0.4%
  - `<php_runtime::vm::Vm>::sweep_idle` 0.2%
- **symtab** 0.3%
  - `<hashbrown::raw::RawTableInner>::is_empty_singleton` 0.1%
  - `<hashbrown::control::bitmask::BitMask>::lowest_set_bit` 0.1%
  - `core::mem::replace::<std::collections::hash::map::HashMap<u32, php_types::zstr::ZStr>>` 0.1%
- **startup_io** 0.2%
  - `<std::sys::thread_local::native::lazy::Storage<core::cell::Cell<(u64, u64)>, !>>::get_or_init::<<std::hash::random::R...` 0.1%
  - `[ld-linux-aarch64.so.1]` 0.1%

Top leaf frames (self time, % of all samples):

- `<php_runtime::vm::Vm>::run_loop` 17.4%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 5.8%
- `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 5.2%
- `php_runtime::vm::path_apply` 4.5%
- `<php_runtime::vm::Vm>::binary_value_ab` 4.2%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 4.0%
- `php_runtime::vm::run::binary_fast` 3.1%
- `<alloc::raw_vec::RawVecInner>::capacity` 2.5%
- `php_runtime::vm::oop::deref_object` 2.4%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 2.3%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 2.3%
- `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 2.3%
- `<core::result::Result<php_types::zval::Zval, php_types::diag::PhpError> as core::ops::try_trait::Try>::branch` 1.7%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 1.7%
- `<alloc::vec::Vec<php_types::zval::Zval>>::pop` 1.6%
- `php_runtime::vm::apply_binop` 1.6%
- `core::ptr::read::<php_types::zval::Zval>` 1.2%
- `<alloc::vec::Vec<php_types::zval::Zval>>::as_mut_slice` 1.2%
- `php_types::ops::try_to_number` 1.0%
- `<php_runtime::vm::Vm>::binary_value_ab::{closure#0}` 1.0%
- `php_types::convert::to_bool` 1.0%
- `<php_runtime::vm::Frame>::with_buffers` 1.0%
- `core::ptr::read::<php_runtime::vm::Frame>` 0.9%
- `<php_runtime::bytecode::Const>::to_zval` 0.8%
- `<php_runtime::vm::Vm>::reg_store_slot` 0.8%
