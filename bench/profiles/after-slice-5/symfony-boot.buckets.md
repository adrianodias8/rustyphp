| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 6.4 |
| `RefCell` borrow / borrow_mut checks | 3.2 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 7.3 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 1.6 |
| string hashing / comparison / copying / formatting | 3.2 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 19.0 |
| argument passing into builtins (lookup by name, pre-call checks) | 1.2 |
| the builtin bodies themselves | 2.7 |
| parser / HIR / compile (script + prelude + include units), per run | 36.0 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 1.3 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 9.6 |
| other: GC bookkeeping and the per-statement destructor sweep | 1.4 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 1.8 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 2.3 |
| other: process startup, libc, std I/O | 2.8 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 6.4%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 2.2%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 1.3%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<php_types::array::PhpArray>>>::as_ref` 0.3%
  - `<alloc::rc::RcInner<php_types::array::PhpArray> as alloc::rc::RcInnerPtr>::dec_strong` 0.3%
  - `<php_types::zstr::ZStr as core::clone::Clone>::clone` 0.3%
  - `<php_types::zstr::ZStr as core::ops::drop::Drop>::drop` 0.2%
  - `core::ptr::drop_glue::<core::option::Option<php_types::zval::Zval>>` 0.2%
  - `core::ptr::drop_glue::<[php_types::zval::Zval]>` 0.2%
- **refcell** 3.2%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::as_ref` 0.7%
  - `<core::cell::BorrowRef>::new` 0.5%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Mut, u32, alloc::rc::Rc<core::cell...` 0.3%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::inc_strong` 0.3%
  - `<core::cell::BorrowRef as core::ops::drop::Drop>::drop` 0.2%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Mut, u32, alloc::rc::Rc<core::cell...` 0.2%
  - `<alloc::vec::Vec<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::object::Object>>>>>::push_mut` 0.1%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::dec_strong` 0.1%
- **alloc** 7.3%
  - `(kernel) page fault / mm: el0_da` 1.8%
  - `mi_theap_malloc_zero_aligned_at` 0.5%
  - `(kernel) page fault / mm: do_mem_abort` 0.4%
  - `<alloc::raw_vec::RawVecInner>::finish_grow` 0.4%
  - `mi_free_block_local` 0.3%
  - `<alloc::raw_vec::RawVecInner>::grow_amortized` 0.3%
  - `mi_page_malloc_zero` 0.3%
  - `_mi_unchecked_ptr_page` 0.3%
- **hash** 1.6%
  - `<php_types::array::KeyIndex>::lookup` 0.4%
  - `<php_types::array::PhpArray>::insert` 0.3%
  - `<php_types::array::KeyIndex>::insert_new` 0.3%
  - `<php_types::array::KeyIndex>::lookup::{closure#0}` 0.2%
  - `<php_types::array::Key as core::cmp::PartialEq>::eq` 0.1%
  - `<php_types::array::PhpArray>::append` 0.1%
  - `<php_types::array::Key>::khash` 0.1%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::try_fold::<(), core::iter::traits::iterator::Iter...` 0.1%
- **string** 3.2%
  - `memcmp` 1.2%
  - `<php_types::zstr::ZStr as core::ops::deref::Deref>::deref` 0.5%
  - `core::ptr::copy_nonoverlapping::<u8>` 0.3%
  - `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` 0.2%
  - `bcmp@plt` 0.2%
  - `memcpy@plt` 0.2%
  - `core::str::validations::run_utf8_validation` 0.2%
  - `<u8 as core::slice::cmp::SlicePartialEq<u8>>::equal_same_length` 0.1%
- **dispatch** 19.0%
  - `<php_runtime::vm::Vm>::run_loop` 10.4%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 1.0%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 1.0%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 0.7%
  - `<php_runtime::vm::Frame>::with_buffers` 0.5%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 0.5%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 0.4%
  - `<php_runtime::vm::Vm>::recycle_frame` 0.4%
- **builtin_args** 1.2%
  - `<php_runtime::vm::Vm>::run_value_builtin` 0.5%
  - `<php_runtime::vm::Vm>::value_builtin_call` 0.2%
  - `<php_runtime::vm::Vm>::compute_stringify::any_object` 0.2%
  - `<core::slice::iter::Iter<php_types::zval::Zval> as core::iter::traits::iterator::Iterator>::any::<<php_runtime::vm::V...` 0.1%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_runtime::builtin::Builtin, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.1%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_runtime::builtin::Builtin)>>::find::<hashbrown::map::equivalent_k...` 0.1%
  - `<php_runtime::vm::Vm>::compute_stringify` 0.1%
  - `<core::slice::iter::Iter<php_types::zval::Zval> as core::iter::traits::iterator::Iterator>::any::<<php_runtime::vm::V...` 0.1%
- **builtin_body** 2.7%
  - `md5::compress::soft::op_h` 0.1%
  - `md5::compress::soft::op_i` 0.1%
  - `php_builtins::json::encode_string` 0.1%
  - `fancy_regex::vm::run` 0.1%
  - `regex_automata::util::determinize::next` 0.1%
  - `<regex_syntax::ast::parse::ParserI<&mut regex_syntax::ast::parse::Parser>>::bump_if` 0.1%
  - `regex_automata::dfa::search::find_rev::<regex_automata::dfa::dense::DFA<alloc::vec::Vec<u32>>>` 0.1%
  - `php_builtins::html::htmlspecialchars` 0.1%
- **compile** 36.0%
  - `under php_runtime::lower::lower_source` 5.8%
  - `under php_runtime::compile::compile_program` 5.3%
  - `under <php_runtime::vm::Vm>::compile_unit_module` 4.7%
  - `under <php_runtime::vm::Vm>::lower_unit` 2.6%
  - `under php_runtime::lower::lower_source_impl` 2.5%
  - `under mago_syntax::parser::parse_file_content` 1.9%
  - `under <mago_syntax::parser::Parser>::parse_statement` 1.7%
  - `under <php_runtime::lower::lower_prelude_uncached as core::ops::function::FnOnce<()>>::call_once` 1.6%
- **operators** 1.3%
  - `<php_runtime::vm::Vm>::binary_value_ab` 0.5%
  - `php_types::convert::to_bool` 0.4%
  - `php_runtime::vm::run::binary_fast` 0.2%
  - `php_types::numstr::parse_numeric_ex` 0.1%
  - `php_runtime::vm::run::concat_n_join` 0.1%
  - `php_types::convert::to_zstr` 0.1%
  - `php_types::ops::increment` 0.1%
  - `php_runtime::vm::run::long_cmp_i64` 0.1%
- **vm_body** 9.6%
  - `<php_runtime::vm::Vm>::prop_set_entry::<true>` 1.1%
  - `<php_runtime::vm::Vm>::coerce_or_check_hint` 1.0%
  - `php_runtime::vm::oop::resolve_method_runtime` 0.5%
  - `<php_runtime::vm::Vm>::call_ns_fallback_site` 0.4%
  - `<php_types::object::PropsLayout>::slot_of` 0.3%
  - `<php_types::object::PropsLayout>::slot_of::{closure#1}` 0.3%
  - `<php_runtime::vm::Vm>::methodcall_fast` 0.3%
  - `php_runtime::vm::oop::resolve_method_runtime::{closure#2}` 0.3%
- **gc** 1.4%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.5%
  - `<php_runtime::vm::Vm>::gc_sweep_body` 0.3%
  - `<php_types::zval::Zval>::is_gc_container` 0.2%
  - `<php_runtime::vm::Vm>::gc_sweep` 0.1%
  - `<php_runtime::vm::Vm>::gc_sweep_impl` 0.1%
  - `<php_runtime::vm::Vm>::gc_note_frame` 0.1%
  - `<php_runtime::vm::Vm>::gc_note` 0.1%
  - `<php_types::object::GcMark>::set_flag` 0.1%
- **symtab** 1.8%
  - `<hashbrown::control::group::neon::Group>::match_tag` 0.3%
  - `rustc_hash::hash_bytes` 0.3%
  - `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` 0.2%
  - `rustc_hash::multiply_mix` 0.2%
  - `<hashbrown::raw::RawTable<((usize, usize), bool)>>::find::<hashbrown::map::equivalent_key<(usize, usize), (usize, usi...` 0.1%
  - `<hashbrown::control::tag::Tag>::full` 0.1%
  - `<hashbrown::raw::RawTable<(alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo)>>::len` 0.1%
  - `<hashbrown::map::HashMap<alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo, rustc_hash::FxBuildHasher>>::get::...` 0.1%
- **kernel** 2.3%
  - `(kernel) blk_mq_end_request` 0.6%
  - `(kernel) path_lookupat` 0.2%
  - `(kernel) link_path_walk` 0.1%
  - `(kernel) __arch_copy_to_user` 0.1%
  - `(kernel) __d_lookup` 0.1%
  - `(kernel) local_daif_restore` 0.1%
  - `(kernel) __d_lookup_rcu` 0.1%
  - `(kernel) virtio_queue_rq` 0.1%
- **startup_io** 2.8%
  - `[ld-linux-aarch64.so.1]` 2.5%
  - `std::sys::fs::unix::canonicalize` 0.1%
  - `<std::sys::fs::unix::File>::open_c::{closure#0}` 0.1%
  - `<std::sys::fd::unix::FileDesc>::read_buf` 0.1%
  - `std::sys::pal::unix::stack_overflow::imp::sigstack_size` 0.1%

Top leaf frames (self time, % of all samples):

- `__pi_clear_page` 10.1%
- `<php_runtime::vm::Vm>::run_loop` 9.4%
- `[libc.so.6]` 2.5%
- `[ld-linux-aarch64.so.1]` 2.4%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 2.2%
- `memcmp` 1.5%
- `core::mem::replace::<usize>` 1.3%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 1.3%
- `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 1.1%
- `<php_runtime::vm::Vm>::prop_set_entry::<true>` 1.0%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 1.0%
- `<mago_syntax::lexer::Lexer>::advance` 0.9%
- `mi_theap_malloc_zero_aligned_at` 0.8%
- `<u8>::is_ascii_uppercase` 0.7%
- `core::ptr::drop_glue::<php_runtime::bytecode::Op>` 0.7%
- `<core::ptr::non_null::NonNull<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::as_ref` 0.7%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 0.7%
- `blk_mq_end_request` 0.6%
- `<php_runtime::hir::HintKind as core::clone::Clone>::clone` 0.6%
- `<core::ptr::non_null::NonNull<php_types::zstr::PhpStr>>::as_ref` 0.5%
- `<php_runtime::vm::Frame>::with_buffers` 0.5%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 0.5%
- `<core::result::Result<(), php_types::diag::PhpError> as core::ops::try_trait::Try>::branch` 0.5%
- `<[u32]>::binary_search_by::<<[u32]>::binary_search::{closure#0}>` 0.5%
- `<u8>::to_ascii_lowercase` 0.5%
