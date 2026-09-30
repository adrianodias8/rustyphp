| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 11.6 |
| `RefCell` borrow / borrow_mut checks | 5.3 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 6.8 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 3.0 |
| string hashing / comparison / copying / formatting | 5.3 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 34.2 |
| argument passing into builtins (lookup by name, pre-call checks) | 2.5 |
| the builtin bodies themselves | 3.3 |
| parser / HIR / compile (script + prelude + include units), per run | 3.2 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 1.9 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 16.7 |
| other: GC bookkeeping and the per-statement destructor sweep | 2.8 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 2.9 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 0.1 |
| other: process startup, libc, std I/O | 0.3 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 11.6%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 3.9%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 2.5%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<php_types::array::PhpArray>>>::as_ref` 1.1%
  - `core::ptr::drop_glue::<alloc::vec::Vec<php_types::zval::Zval>>` 0.8%
  - `core::ptr::drop_glue::<[php_types::zval::Zval]>` 0.4%
  - `<php_types::zval::Zval>::deref_clone` 0.4%
  - `<php_types::array::PhpArray as core::ops::drop::Drop>::drop` 0.4%
  - `<php_types::zstr::ZStr as core::clone::Clone>::clone` 0.3%
- **refcell** 5.3%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::as_ref` 1.0%
  - `<core::cell::BorrowRef>::new` 0.8%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Mut, u32, alloc::rc::Rc<core::cell...` 0.7%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Immut, u32, alloc::rc::Rc<core::ce...` 0.3%
  - `<core::cell::BorrowRef as core::ops::drop::Drop>::drop` 0.3%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::inc_strong` 0.3%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::dec_strong` 0.2%
  - `core::mem::replace::<core::slice::iter::Iter<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::object...` 0.2%
- **alloc** 6.8%
  - `mi_theap_malloc_zero_aligned_at` 0.7%
  - `mi_page_malloc_zero` 0.5%
  - `<alloc::raw_vec::RawVecInner>::finish_grow` 0.5%
  - `<alloc::raw_vec::RawVecInner>::grow_amortized` 0.4%
  - `mi_free` 0.3%
  - `mi_free_block_local` 0.3%
  - `_mi_unchecked_ptr_page` 0.3%
  - `mi_theap_malloc_aligned` 0.3%
- **hash** 3.0%
  - `<php_types::array::PhpArray>::insert` 0.7%
  - `<php_types::array::KeyIndex>::lookup` 0.7%
  - `<php_types::array::PhpArray>::append` 0.3%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next` 0.2%
  - `php_types::array::canonical_int_key` 0.2%
  - `<php_types::array::PhpArray>::contains_key` 0.1%
  - `<php_types::array::PhpArray>::iter` 0.1%
  - `<php_types::array::KeyIndex>::insert_new` 0.1%
- **string** 5.3%
  - `memcmp` 1.9%
  - `<php_types::zstr::ZStr as core::ops::deref::Deref>::deref` 0.9%
  - `core::ptr::copy_nonoverlapping::<u8>` 0.6%
  - `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` 0.4%
  - `bcmp@plt` 0.3%
  - `memcpy@plt` 0.3%
  - `<php_types::zstr::PhpStr>::as_bytes` 0.2%
  - `<[u8] as core::cmp::PartialEq>::eq` 0.1%
- **dispatch** 34.2%
  - `<php_runtime::vm::Vm>::run_loop` 19.4%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 1.6%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 1.5%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.0%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 0.9%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 0.8%
  - `<php_runtime::vm::Frame>::with_buffers` 0.7%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::capacity` 0.7%
- **builtin_args** 2.5%
  - `<php_runtime::vm::Vm>::run_value_builtin` 1.1%
  - `<php_runtime::vm::Vm>::value_builtin_call` 0.3%
  - `<php_runtime::vm::Vm>::compute_stringify::any_object` 0.2%
  - `<php_runtime::builtin::Ctx>::to_zstr` 0.2%
  - `php_runtime::vm::run::is_user_stream_op` 0.1%
  - `<php_runtime::vm::Vm>::compute_stringify` 0.1%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_runtime::builtin::Builtin, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.1%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_runtime::builtin::Builtin)>>::find::<hashbrown::map::equivalent_k...` 0.1%
- **builtin_body** 3.3%
  - `md5::compress::soft::op_h` 0.2%
  - `fancy_regex::vm::run` 0.2%
  - `php_builtins::html::encode_special` 0.2%
  - `md5::compress::soft::op_f` 0.1%
  - `php_builtins::json::encode_string` 0.1%
  - `regex_automata::dfa::search::find_fwd_imp::<&regex_automata::dfa::dense::DFA<alloc::vec::Vec<u32>>>` 0.1%
  - `md5::compress::soft::op_i` 0.1%
  - `md5::compress::soft::op_g` 0.1%
- **compile** 3.2%
  - `<php_runtime::hir::HintKind as core::clone::Clone>::clone` 0.5%
  - `core::ptr::drop_glue::<php_runtime::hir::HintKind>` 0.5%
  - `under <php_runtime::vm::Vm>::compile_unit_module` 0.4%
  - `under php_runtime::lower::lower_source` 0.4%
  - `under php_runtime::compile::compile_program` 0.3%
  - `under <php_runtime::vm::Vm>::lower_unit` 0.2%
  - `<core::option::Option<php_runtime::hir::TypeHint> as core::clone::Clone>::clone` 0.2%
  - `under php_runtime::lower::lower_source_seeded` 0.1%
- **operators** 1.9%
  - `php_types::convert::to_bool` 0.5%
  - `<php_runtime::vm::Vm>::binary_value_ab` 0.4%
  - `php_types::convert::to_zstr` 0.3%
  - `php_runtime::vm::run::binary_fast` 0.1%
  - `php_types::convert::is_true_silent` 0.1%
  - `php_runtime::vm::apply_binop` 0.1%
  - `php_types::ops::identical` 0.1%
  - `php_runtime::vm::run::concat_n_join` 0.1%
- **vm_body** 16.7%
  - `<php_runtime::vm::Vm>::coerce_or_check_hint` 2.3%
  - `<php_runtime::vm::Vm>::prop_set_entry::<true>` 1.0%
  - `<php_runtime::vm::Vm>::call_ns_fallback_site` 0.9%
  - `<php_runtime::vm::Vm>::methodcall_fast` 0.8%
  - `php_runtime::vm::oop::resolve_method_runtime` 0.7%
  - `<php_runtime::vm::Vm>::coerce_param_hints` 0.6%
  - `php_runtime::vm::oop::deref_object` 0.5%
  - `<php_types::object::PropsLayout>::slot_of` 0.5%
- **gc** 2.8%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.9%
  - `<php_runtime::vm::Vm>::gc_sweep_body` 0.5%
  - `<php_runtime::vm::Vm>::sweep_idle` 0.2%
  - `<php_types::zval::Zval>::is_gc_container` 0.2%
  - `<php_runtime::vm::Vm>::gc_idle_compute` 0.2%
  - `<php_runtime::vm::Vm>::gc_sweep` 0.1%
  - `<php_runtime::vm::Vm>::gc_note_frame` 0.1%
  - `<php_runtime::vm::Vm>::gc_note` 0.1%
- **symtab** 2.9%
  - `rustc_hash::multiply_mix` 0.3%
  - `<hashbrown::control::group::neon::Group>::match_tag` 0.3%
  - `rustc_hash::hash_bytes` 0.3%
  - `php_runtime::bytecode::ci_hash` 0.2%
  - `<hashbrown::map::HashMap<alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo, rustc_hash::FxBuildHasher>>::get::...` 0.2%
  - `<hashbrown::raw::RawTableInner>::find_inner` 0.1%
  - `<hashbrown::control::tag::Tag>::full` 0.1%
  - `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` 0.1%
- **kernel** 0.1%
  - `(kernel) __kernel_clock_gettime` 0.1%
- **startup_io** 0.3%
  - `[ld-linux-aarch64.so.1]` 0.2%
  - `<std::sys::thread_local::native::lazy::Storage<core::cell::Cell<(u64, u64)>, !>>::get_or_init::<<std::hash::random::R...` 0.1%

Top leaf frames (self time, % of all samples):

- `<php_runtime::vm::Vm>::run_loop` 17.8%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 3.9%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 2.5%
- `memcmp` 1.9%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 1.6%
- `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 1.5%
- `<core::result::Result<(), php_types::diag::PhpError> as core::ops::try_trait::Try>::branch` 1.5%
- `core::mem::replace::<usize>` 1.2%
- `<core::ptr::non_null::NonNull<alloc::rc::RcInner<php_types::array::PhpArray>>>::as_ref` 1.1%
- `<core::ptr::non_null::NonNull<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::as_ref` 1.0%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.0%
- `<u8>::is_ascii_uppercase` 1.0%
- `[libc.so.6]` 1.0%
- `<php_runtime::vm::Vm>::coerce_or_check_hint` 1.0%
- `<alloc::raw_vec::RawVecInner>::capacity` 0.9%
- `<core::ptr::non_null::NonNull<php_types::zstr::PhpStr>>::as_ref` 0.9%
- `<php_runtime::vm::Vm>::prop_set_entry::<true>` 0.9%
- `<php_runtime::vm::Vm>::gc_note_slow` 0.9%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 0.9%
- `core::ptr::drop_glue::<alloc::vec::Vec<php_types::zval::Zval>>` 0.8%
- `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 0.8%
- `mi_theap_malloc_zero_aligned_at` 0.7%
- `<php_runtime::vm::Frame>::with_buffers` 0.7%
- `<php_runtime::vm::Vm>::methodcall_fast` 0.7%
- `<php_runtime::vm::Vm>::run_value_builtin` 0.6%
