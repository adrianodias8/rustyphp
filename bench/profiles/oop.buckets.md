| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 9.4 |
| `RefCell` borrow / borrow_mut checks | 4.7 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 8.1 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 2.5 |
| string hashing / comparison / copying / formatting | 5.0 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 34.2 |
| argument passing into builtins (lookup by name, pre-call checks) | 0.0 |
| the builtin bodies themselves | 0.0 |
| parser / HIR / compile (script + prelude + include units), per run | 1.8 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 3.5 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 21.4 |
| other: GC bookkeeping and the per-statement destructor sweep | 3.6 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 5.7 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 0.0 |
| other: process startup, libc, std I/O | 0.1 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 9.4%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 4.4%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 2.4%
  - `core::ptr::drop_glue::<alloc::vec::Vec<php_types::zval::Zval>>` 0.7%
  - `core::ptr::drop_glue::<core::option::Option<php_types::zval::Zval>>` 0.4%
  - `<php_types::zval::Zval>::deref_clone` 0.2%
  - `<php_types::array::PhpArray as core::ops::drop::Drop>::drop` 0.2%
  - `core::ptr::drop_glue::<php_types::array::Repr>` 0.1%
  - `core::ptr::drop_glue::<[php_types::zval::Zval]>` 0.1%
- **refcell** 4.7%
  - `<core::cell::BorrowRef>::new` 1.1%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::dec_strong` 0.5%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::inc_strong` 0.4%
  - `<alloc::raw_vec::RawVec<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::object::Object>>>>>::capacity` 0.2%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::as_ref` 0.2%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Mut, u32, alloc::rc::Rc<core::cell...` 0.2%
  - `core::mem::replace::<core::slice::iter::Iter<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::object...` 0.2%
  - `<usize as core::slice::index::SliceIndex<[core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::object::O...` 0.2%
- **alloc** 8.1%
  - `mi_theap_malloc_zero_aligned_at` 1.3%
  - `mi_free` 0.9%
  - `_mi_unchecked_ptr_page` 0.8%
  - `mi_free_block_local` 0.8%
  - `mi_page_malloc_zero` 0.7%
  - `mi_theap_malloc_aligned` 0.4%
  - `<alloc::raw_vec::RawVecInner>::finish_grow` 0.4%
  - `<alloc::raw_vec::RawVecInner>::grow_amortized` 0.2%
- **hash** 2.5%
  - `<php_types::array::PhpArray>::insert` 0.6%
  - `<php_types::array::KeyIndex>::lookup` 0.5%
  - `<php_types::array::KeyIndex>::lookup::{closure#0}` 0.2%
  - `<php_types::array::PhpArray>::to_hashed` 0.2%
  - `<php_types::array::KeyIndex>::insert_new` 0.2%
  - `php_types::array::canonical_int_key` 0.2%
  - `<php_types::array::PhpArray>::append` 0.1%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next` 0.1%
- **string** 5.0%
  - `memcmp` 1.9%
  - `core::ptr::copy_nonoverlapping::<u8>` 1.2%
  - `bcmp@plt` 0.4%
  - `memcpy@plt` 0.3%
  - `core::fmt::write` 0.2%
  - `<core::fmt::Arguments>::estimated_capacity` 0.2%
  - `<core::fmt::Formatter>::pad_integral` 0.1%
  - `<php_types::zstr::PhpStr>::new::<alloc::vec::Vec<u8>>` 0.1%
- **dispatch** 34.2%
  - `<php_runtime::vm::Vm>::run_loop` 8.9%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 2.4%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 2.0%
  - `<php_runtime::vm::Vm>::enter_callee` 1.9%
  - `<php_runtime::vm::Frame>::with_buffers` 1.8%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 1.6%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.4%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 1.3%
- **compile** 1.8%
  - `<php_runtime::hir::HintKind as core::clone::Clone>::clone` 0.7%
  - `core::ptr::drop_glue::<php_runtime::hir::HintKind>` 0.3%
  - `<core::option::Option<php_runtime::hir::TypeHint> as core::clone::Clone>::clone` 0.3%
  - `under php_runtime::lower::lower_source` 0.2%
  - `under php_runtime::compile::compile_program` 0.1%
  - `<php_runtime::hir::Visibility as core::cmp::PartialEq>::eq` 0.1%
- **operators** 3.5%
  - `php_runtime::vm::run::binary_fast` 0.7%
  - `<php_runtime::vm::Vm>::binary_value_ab` 0.6%
  - `php_types::convert::to_bool` 0.6%
  - `<php_runtime::vm::Vm>::compute_incdec` 0.5%
  - `<php_runtime::vm::Vm>::incdec_slot_discard_slow` 0.4%
  - `php_types::ops::increment` 0.2%
  - `<php_runtime::vm::Vm>::incdec_slot_discard` 0.1%
  - `php_runtime::vm::overload_receiver` 0.1%
- **vm_body** 21.4%
  - `<php_runtime::vm::Vm>::coerce_or_check_hint` 5.4%
  - `<php_runtime::vm::Vm>::prop_set_entry::<true>` 2.2%
  - `php_runtime::vm::oop::deref_object` 1.3%
  - `<php_types::object::PropsLayout>::slot_of` 1.0%
  - `php_runtime::vm::oop::resolve_method_runtime::{closure#3}` 0.9%
  - `php_runtime::vm::oop::resolve_prop_access` 0.7%
  - `<php_runtime::vm::Vm>::methodcall_fast` 0.6%
  - `php_runtime::vm::oop::resolve_method_runtime` 0.6%
- **gc** 3.6%
  - `<php_runtime::vm::Vm>::gc_sweep_body` 1.0%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.6%
  - `<php_runtime::vm::Vm>::gc_sweep_impl` 0.4%
  - `<php_runtime::vm::Vm>::gc_note_frame` 0.3%
  - `<php_runtime::vm::Vm>::sweep_idle` 0.3%
  - `<php_types::zval::Zval>::is_gc_container` 0.2%
  - `<php_runtime::vm::Vm>::gc_sweep` 0.2%
  - `<php_runtime::vm::Vm>::gc_idle_compute` 0.2%
- **symtab** 5.7%
  - `<hashbrown::control::group::neon::Group>::match_tag` 1.1%
  - `<hashbrown::map::HashMap<alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo, rustc_hash::FxBuildHasher>>::get::...` 0.7%
  - `rustc_hash::hash_bytes` 0.6%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_types::zval::Zval, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.4%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_types::zval::Zval)>>::len` 0.3%
  - `<hashbrown::raw::RawTable<(alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo)>>::find::<hashbrown::map::equiva...` 0.2%
  - `<hashbrown::raw::RawTableInner>::find_inner` 0.2%
  - `<hashbrown::raw::RawTable<(alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo)>>::len` 0.2%
- **startup_io** 0.1%

Top leaf frames (self time, % of all samples):

- `<php_runtime::vm::Vm>::run_loop` 7.5%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 4.4%
- `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 2.4%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 2.4%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 2.0%
- `<php_runtime::vm::Vm>::prop_set_entry::<true>` 2.0%
- `memcmp` 1.9%
- `<php_runtime::vm::Frame>::with_buffers` 1.8%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 1.6%
- `[libc.so.6]` 1.6%
- `<php_runtime::vm::Vm>::coerce_or_check_hint` 1.6%
- `php_runtime::coerce::coerce_to_hint` 1.5%
- `<php_runtime::vm::Vm>::enter_callee` 1.5%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.4%
- `php_runtime::coerce::php_type_name` 1.3%
- `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 1.3%
- `mi_theap_malloc_zero_aligned_at` 1.3%
- `php_runtime::vm::oop::deref_object` 1.3%
- `core::mem::replace::<usize>` 1.3%
- `<alloc::raw_vec::RawVecInner>::capacity` 1.3%
- `php_runtime::vm::calls::bind_params` 1.1%
- `core::core_arch::arm_shared::neon::generated::vreinterpret_u64_u8` 1.1%
- `<php_runtime::vm::Vm>::gc_sweep_body` 0.9%
- `mi_free` 0.9%
- `<core::cell::Cell<isize>>::get` 0.8%
