| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 10.5 |
| `RefCell` borrow / borrow_mut checks | 5.4 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 7.5 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 2.2 |
| string hashing / comparison / copying / formatting | 5.0 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 32.8 |
| argument passing into builtins (lookup by name, pre-call checks) | 0.0 |
| the builtin bodies themselves | 0.0 |
| parser / HIR / compile (script + prelude + include units), per run | 1.6 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 3.8 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 22.2 |
| other: GC bookkeeping and the per-statement destructor sweep | 3.7 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 5.3 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 0.0 |
| other: process startup, libc, std I/O | 0.1 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 10.5%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 4.8%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 2.5%
  - `core::ptr::drop_glue::<alloc::vec::Vec<php_types::zval::Zval>>` 1.0%
  - `core::ptr::drop_glue::<core::option::Option<php_types::zval::Zval>>` 0.4%
  - `<php_types::array::PhpArray as core::ops::drop::Drop>::drop` 0.3%
  - `core::ptr::drop_glue::<[php_types::zval::Zval]>` 0.2%
  - `core::ptr::drop_glue::<php_types::array::Repr>` 0.2%
  - `<php_types::zval::Zval>::deref_clone` 0.1%
- **refcell** 5.4%
  - `<core::cell::BorrowRef>::new` 1.5%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::inc_strong` 0.5%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::dec_strong` 0.4%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Mut, u32, alloc::rc::Rc<core::cell...` 0.3%
  - `<core::cell::BorrowRef as core::ops::drop::Drop>::drop` 0.3%
  - `core::ptr::drop_glue::<alloc::vec::drain::Drain<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::obj...` 0.2%
  - `core::mem::replace::<core::slice::iter::Iter<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::object...` 0.2%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::as_ref` 0.2%
- **alloc** 7.5%
  - `mi_theap_malloc_zero_aligned_at` 1.1%
  - `mi_free_block_local` 0.9%
  - `_mi_unchecked_ptr_page` 0.7%
  - `mi_free` 0.7%
  - `mi_page_malloc_zero` 0.7%
  - `mi_theap_malloc_aligned` 0.4%
  - `<alloc::raw_vec::RawVecInner>::finish_grow` 0.3%
  - `mi_page_xthread_id` 0.3%
- **hash** 2.2%
  - `<php_types::array::PhpArray>::insert` 0.5%
  - `<php_types::array::KeyIndex>::lookup` 0.4%
  - `<php_types::array::PhpArray>::to_hashed` 0.2%
  - `<php_types::array::KeyIndex>::lookup::{closure#0}` 0.2%
  - `php_types::array::canonical_int_key` 0.2%
  - `<php_types::array::KeyIndex>::insert_new` 0.1%
  - `<php_types::array::Key as core::cmp::PartialEq>::eq` 0.1%
  - `<php_types::array::PhpArray>::append` 0.1%
- **string** 5.0%
  - `memcmp` 1.7%
  - `core::ptr::copy_nonoverlapping::<u8>` 1.2%
  - `bcmp@plt` 0.5%
  - `memcpy@plt` 0.4%
  - `core::fmt::write` 0.2%
  - `<core::fmt::Formatter>::pad_integral` 0.1%
  - `<core::fmt::Arguments>::estimated_capacity` 0.1%
  - `<php_types::zstr::PhpStr>::new::<alloc::vec::Vec<u8>>` 0.1%
- **dispatch** 32.8%
  - `<php_runtime::vm::Vm>::run_loop` 9.1%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 2.7%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 2.4%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 1.6%
  - `<php_runtime::vm::Frame>::with_buffers` 1.5%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 1.3%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.2%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::capacity` 1.1%
- **compile** 1.6%
  - `core::ptr::drop_glue::<php_runtime::hir::HintKind>` 0.5%
  - `<php_runtime::hir::HintKind as core::clone::Clone>::clone` 0.4%
  - `<core::option::Option<php_runtime::hir::TypeHint> as core::clone::Clone>::clone` 0.2%
  - `under php_runtime::lower::lower_source` 0.1%
  - `under php_runtime::compile::compile_program` 0.1%
  - `<php_runtime::hir::Visibility as core::cmp::PartialEq>::eq` 0.1%
- **operators** 3.8%
  - `php_runtime::vm::run::binary_fast` 0.8%
  - `<php_runtime::vm::Vm>::binary_value_ab` 0.7%
  - `<php_runtime::vm::Vm>::incdec_slot_discard_slow` 0.6%
  - `php_types::convert::to_bool` 0.5%
  - `<php_runtime::vm::Vm>::compute_incdec` 0.3%
  - `php_types::ops::increment` 0.3%
  - `php_runtime::vm::overload_receiver` 0.2%
  - `<php_runtime::vm::Vm>::incdec_slot_discard` 0.2%
- **vm_body** 22.2%
  - `<php_runtime::vm::Vm>::coerce_or_check_hint` 5.2%
  - `<php_runtime::vm::Vm>::prop_set_entry::<true>` 2.1%
  - `php_runtime::vm::oop::write_property_at` 1.0%
  - `<php_types::object::PropsLayout>::slot_of` 1.0%
  - `php_runtime::vm::oop::resolve_prop_access` 0.9%
  - `php_runtime::vm::oop::resolve_method_runtime::{closure#3}` 0.9%
  - `<php_runtime::vm::Vm>::methodcall_fast` 0.9%
  - `php_runtime::vm::oop::deref_object` 0.8%
- **gc** 3.7%
  - `<php_runtime::vm::Vm>::gc_sweep_body` 1.1%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.6%
  - `<php_runtime::vm::Vm>::gc_sweep_impl` 0.4%
  - `<php_runtime::vm::Vm>::gc_note_frame` 0.3%
  - `<php_runtime::vm::Vm>::gc_sweep` 0.2%
  - `<php_runtime::vm::Vm>::sweep_idle` 0.2%
  - `<php_types::zval::Zval>::is_gc_container` 0.2%
  - `<php_runtime::vm::Vm>::gc_idle_compute` 0.2%
- **symtab** 5.3%
  - `<hashbrown::control::group::neon::Group>::match_tag` 0.8%
  - `<hashbrown::map::HashMap<alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo, rustc_hash::FxBuildHasher>>::get::...` 0.6%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_types::zval::Zval, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.6%
  - `rustc_hash::hash_bytes` 0.5%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_types::zval::Zval)>>::len` 0.3%
  - `<hashbrown::raw::RawTable<(alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo)>>::len` 0.2%
  - `<hashbrown::raw::RawTable<(alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo)>>::find::<hashbrown::map::equiva...` 0.2%
  - `<hashbrown::raw::RawTableInner>::find_inner` 0.2%
- **startup_io** 0.1%

Top leaf frames (self time, % of all samples):

- `<php_runtime::vm::Vm>::run_loop` 7.7%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 4.8%
- `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 2.7%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 2.5%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 2.4%
- `<php_runtime::vm::Vm>::coerce_or_check_hint` 1.9%
- `<php_runtime::vm::Vm>::prop_set_entry::<true>` 1.7%
- `memcmp` 1.7%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 1.6%
- `<php_runtime::vm::Frame>::with_buffers` 1.5%
- `php_runtime::coerce::coerce_to_hint` 1.5%
- `<alloc::raw_vec::RawVecInner>::capacity` 1.5%
- `[libc.so.6]` 1.4%
- `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 1.3%
- `core::mem::replace::<usize>` 1.3%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.2%
- `<core::cell::Cell<isize>>::get` 1.2%
- `mi_theap_malloc_zero_aligned_at` 1.1%
- `<php_runtime::vm::Vm>::gc_sweep_body` 1.0%
- `php_runtime::vm::oop::write_property_at` 1.0%
- `core::ptr::drop_glue::<alloc::vec::Vec<php_types::zval::Zval>>` 1.0%
- `php_runtime::vm::calls::decay_arg` 0.9%
- `mi_free_block_local` 0.9%
- `php_runtime::vm::oop::resolve_prop_access` 0.9%
- `php_runtime::vm::oop::deref_object` 0.8%
