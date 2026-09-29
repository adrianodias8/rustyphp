| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 9.4 |
| `RefCell` borrow / borrow_mut checks | 4.4 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 6.5 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 2.3 |
| string hashing / comparison / copying / formatting | 4.7 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 30.9 |
| argument passing into builtins (lookup by name, pre-call checks) | 2.2 |
| the builtin bodies themselves | 3.1 |
| parser / HIR / compile (script + prelude + include units), per run | 4.1 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 1.7 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 22.6 |
| other: GC bookkeeping and the per-statement destructor sweep | 2.3 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 5.3 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 0.1 |
| other: process startup, libc, std I/O | 0.3 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 9.4%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 2.8%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 2.4%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<php_types::array::PhpArray>>>::as_ref` 0.7%
  - `<php_types::zstr::ZStr as core::ops::drop::Drop>::drop` 0.5%
  - `<php_types::zstr::ZStr as core::clone::Clone>::clone` 0.4%
  - `core::ptr::drop_glue::<alloc::vec::Vec<php_types::zval::Zval>>` 0.4%
  - `core::ptr::drop_glue::<[php_types::zval::Zval]>` 0.4%
  - `<php_types::array::PhpArray as core::ops::drop::Drop>::drop` 0.2%
- **refcell** 4.4%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::as_ref` 0.8%
  - `<core::cell::BorrowRef>::new` 0.5%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::dec_strong` 0.5%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Mut, u32, alloc::rc::Rc<core::cell...` 0.4%
  - `<core::cell::BorrowRef as core::ops::drop::Drop>::drop` 0.4%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::inc_strong` 0.2%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::strong` 0.2%
  - `<alloc::raw_vec::RawVec<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::object::Object>>>>>::capacity` 0.1%
- **alloc** 6.5%
  - `mi_theap_malloc_zero_aligned_at` 0.7%
  - `mi_free` 0.6%
  - `mi_theap_malloc_aligned` 0.6%
  - `mi_page_malloc_zero` 0.5%
  - `(kernel) page fault / mm: el0_da` 0.3%
  - `_mi_unchecked_ptr_page` 0.3%
  - `<alloc::raw_vec::RawVecInner>::finish_grow` 0.3%
  - `<alloc::raw_vec::RawVecInner>::grow_amortized` 0.3%
- **hash** 2.3%
  - `<php_types::array::PhpArray>::insert` 0.5%
  - `<php_types::array::KeyIndex>::lookup` 0.4%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next` 0.3%
  - `<php_types::array::PhpArray>::append` 0.2%
  - `php_types::array::canonical_int_key` 0.1%
  - `<php_types::array::KeyIndex>::lookup::{closure#0}` 0.1%
  - `<php_types::array::Key>::khash` 0.1%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next::{closure#1}` 0.1%
- **string** 4.7%
  - `memcmp` 1.3%
  - `<php_types::zstr::ZStr as core::ops::deref::Deref>::deref` 1.0%
  - `core::ptr::copy_nonoverlapping::<u8>` 0.7%
  - `bcmp@plt` 0.4%
  - `memcpy@plt` 0.3%
  - `<[u8] as core::cmp::PartialEq<[u8` 0.2%
  - `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` 0.1%
  - `<php_types::zstr::PhpStr>::as_bytes` 0.1%
- **dispatch** 30.9%
  - `<php_runtime::vm::Vm>::run_loop` 15.5%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 1.4%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 1.3%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.1%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::pop` 0.9%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 0.8%
  - `<php_runtime::vm::Frame>::with_buffers` 0.7%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::capacity` 0.6%
- **builtin_args** 2.2%
  - `<php_runtime::vm::Vm>::run_value_builtin` 1.1%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_runtime::builtin::Builtin, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.3%
  - `<php_runtime::vm::Vm>::value_builtin_call` 0.2%
  - `<php_runtime::vm::Vm>::compute_stringify::any_object` 0.2%
  - `<php_runtime::vm::Vm>::compute_stringify` 0.2%
  - `<core::slice::iter::Iter<php_types::zval::Zval> as core::iter::traits::iterator::Iterator>::any::<<php_runtime::vm::V...` 0.1%
  - `<php_runtime::builtin::Ctx>::to_zstr` 0.1%
  - `php_runtime::vm::run::is_user_stream_op` 0.1%
- **builtin_body** 3.1%
  - `md5::compress::soft::op_f` 0.2%
  - `regex_automata::dfa::search::find_fwd_imp::<&regex_automata::dfa::dense::DFA<alloc::vec::Vec<u32>>>` 0.2%
  - `php_builtins::html::encode_special` 0.2%
  - `fancy_regex::vm::run` 0.2%
  - `php_builtins::json::encode_string` 0.2%
  - `md5::compress::soft::op_i` 0.1%
  - `php_builtins::json::encode` 0.1%
  - `php_builtins::null_arg_deprecation` 0.1%
- **compile** 4.1%
  - `<php_runtime::hir::HintKind as core::clone::Clone>::clone` 0.7%
  - `under php_runtime::lower::lower_source` 0.5%
  - `under <php_runtime::vm::Vm>::compile_unit_module` 0.4%
  - `under <php_runtime::vm::Vm>::lower_unit` 0.4%
  - `under php_runtime::compile::compile_program` 0.3%
  - `core::ptr::drop_glue::<php_runtime::hir::HintKind>` 0.3%
  - `<core::option::Option<php_runtime::hir::TypeHint> as core::clone::Clone>::clone` 0.2%
  - `under php_runtime::lower::lower_source_impl` 0.2%
- **operators** 1.7%
  - `php_types::convert::to_zstr` 0.4%
  - `php_types::convert::to_bool` 0.4%
  - `<php_runtime::vm::Vm>::binary_value_ab` 0.2%
  - `php_runtime::vm::run::binary_fast` 0.1%
  - `php_runtime::vm::apply_binop` 0.1%
  - `<php_runtime::vm::Vm>::apply_binop_ovl` 0.1%
  - `php_types::ops::binop_longs` 0.1%
  - `php_types::ops::identical` 0.1%
- **vm_body** 22.6%
  - `php_runtime::vm::host_builtin_canonical` 6.1%
  - `<php_runtime::vm::Vm>::coerce_or_check_hint` 1.8%
  - `<php_runtime::vm::Vm>::prop_set_entry::<true>` 1.2%
  - `<php_types::object::PropsLayout>::slot_of` 0.8%
  - `php_runtime::vm::oop::resolve_method_runtime` 0.8%
  - `<php_runtime::vm::LcKey>::new` 0.7%
  - `php_runtime::vm::oop::deref_object` 0.6%
  - `php_runtime::vm::path_apply` 0.6%
- **gc** 2.3%
  - `<php_runtime::vm::Vm>::gc_sweep_body` 0.7%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.6%
  - `<php_types::zval::Zval>::is_gc_container` 0.2%
  - `<php_runtime::vm::Vm>::gc_sweep_impl` 0.2%
  - `<php_runtime::vm::Vm>::gc_note_frame` 0.1%
  - `<php_runtime::vm::Vm>::gc_note_iter` 0.1%
  - `<php_runtime::vm::Vm>::gc_root_arr` 0.1%
  - `<php_runtime::vm::Vm>::gc_sweep` 0.1%
- **symtab** 5.3%
  - `php_runtime::bytecode::ci_hash` 1.2%
  - `<[(u64, u32)]>::binary_search_by::<<[(u64, u32)]>::partition_point<<php_runtime::bytecode::Module>::find_fn_ci::{clos...` 0.9%
  - `rustc_hash::hash_bytes` 0.5%
  - `<hashbrown::control::group::neon::Group>::match_tag` 0.5%
  - `rustc_hash::multiply_mix` 0.4%
  - `<php_runtime::bytecode::Module>::find_fn_ci` 0.4%
  - `<hashbrown::map::HashMap<alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo, rustc_hash::FxBuildHasher>>::get::...` 0.2%
  - `<hashbrown::control::group::neon::Group>::load` 0.1%
- **kernel** 0.1%
  - `(kernel) [[vdso]]` 0.1%
- **startup_io** 0.3%
  - `[ld-linux-aarch64.so.1]` 0.2%
  - `<std::sys::thread_local::native::lazy::Storage<core::cell::Cell<(u64, u64)>, !>>::get_or_init::<<std::hash::random::R...` 0.1%

Top leaf frames (self time, % of all samples):

- `<php_runtime::vm::Vm>::run_loop` 14.4%
- `<core::slice::iter::Iter<&[u8]> as core::iter::traits::iterator::Iterator>::next` 5.2%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 2.8%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 2.4%
- `core::mem::replace::<usize>` 1.6%
- `memcmp` 1.4%
- `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 1.4%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 1.3%
- `<u8>::is_ascii_uppercase` 1.2%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.1%
- `<core::result::Result<(), php_types::diag::PhpError> as core::ops::try_trait::Try>::branch` 1.1%
- `[libc.so.6]` 1.1%
- `<core::iter::adapters::zip::Zip<core::slice::iter::IterMut<u8>, core::slice::iter::Iter<u8>> as core::iter::adapters:...` 1.1%
- `<core::ptr::non_null::NonNull<php_types::zstr::PhpStr>>::as_ref` 1.0%
- `<php_runtime::vm::Vm>::prop_set_entry::<true>` 1.0%
- `<core::slice::iter::Iter<&[u8]> as core::iter::traits::iterator::Iterator>::try_fold::<(), core::iter::adapters::copi...` 0.9%
- `<alloc::raw_vec::RawVecInner>::capacity` 0.9%
- `<alloc::vec::Vec<php_types::zval::Zval>>::pop` 0.9%
- `<[(u64, u32)]>::binary_search_by::<<[(u64, u32)]>::partition_point<<php_runtime::bytecode::Module>::find_fn_ci::{clos...` 0.9%
- `__pi_clear_page` 0.8%
- `<core::ptr::non_null::NonNull<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::as_ref` 0.8%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 0.8%
- `mi_theap_malloc_zero_aligned_at` 0.7%
- `<core::ptr::non_null::NonNull<alloc::rc::RcInner<php_types::array::PhpArray>>>::as_ref` 0.7%
- `<php_runtime::vm::Vm>::coerce_or_check_hint` 0.7%
