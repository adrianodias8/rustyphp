| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 4.6 |
| `RefCell` borrow / borrow_mut checks | 2.2 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 14.6 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 1.6 |
| string hashing / comparison / copying / formatting | 2.7 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 15.1 |
| argument passing into builtins (lookup by name, pre-call checks) | 0.8 |
| the builtin bodies themselves | 2.0 |
| parser / HIR / compile (script + prelude + include units), per run | 36.1 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 1.1 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 10.3 |
| other: GC bookkeeping and the per-statement destructor sweep | 1.2 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 2.7 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 2.6 |
| other: process startup, libc, std I/O | 2.4 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 4.6%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 1.6%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 1.2%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<php_types::array::PhpArray>>>::as_ref` 0.3%
  - `core::ptr::drop_glue::<[php_types::zval::Zval]>` 0.2%
  - `<php_types::zval::Zval>::deref_clone` 0.2%
  - `<php_types::zstr::ZStr as core::ops::drop::Drop>::drop` 0.1%
  - `<php_types::array::PhpArray as core::ops::drop::Drop>::drop` 0.1%
  - `<alloc::rc::RcInner<php_types::array::PhpArray> as alloc::rc::RcInnerPtr>::strong` 0.1%
- **refcell** 2.2%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::as_ref` 0.4%
  - `<core::cell::BorrowRef>::new` 0.4%
  - `<core::cell::BorrowRef as core::ops::drop::Drop>::drop` 0.1%
  - `<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>> as alloc::rc::RcInnerPtr>::dec_strong` 0.1%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Mut, u32, alloc::rc::Rc<core::cell...` 0.1%
  - `core::ptr::drop_glue::<alloc::vec::drain::Drain<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::obj...` 0.1%
  - `<core::cell::BorrowRefMut as core::ops::drop::Drop>::drop` 0.1%
  - `core::ptr::drop_glue::<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::zval::Zval>>>>` 0.1%
- **alloc** 14.6%
  - `(kernel) page fault / mm: el0_da` 7.4%
  - `(kernel) page fault / mm: do_mem_abort` 2.1%
  - `(kernel) page fault / mm: __arm64_sys_madvise` 0.6%
  - `mi_theap_malloc_zero_aligned_at` 0.5%
  - `(kernel) page fault / mm: read_cache_folio` 0.4%
  - `(kernel) page fault / mm: __arm64_sys_mmap` 0.3%
  - `mi_free` 0.3%
  - `mi_page_malloc_zero` 0.3%
- **hash** 1.6%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next` 0.3%
  - `<php_types::array::PhpArray>::insert` 0.3%
  - `<php_types::array::KeyIndex>::lookup` 0.2%
  - `<php_types::array::PhpArray>::append` 0.1%
  - `<php_types::array::PhpArray>::to_hashed` 0.1%
  - `<php_types::array::PhpArray>::contains_key` 0.1%
  - `php_types::array::canonical_int_key` 0.1%
  - `<php_types::array::PhpArray>::get` 0.1%
- **string** 2.7%
  - `memcmp` 0.9%
  - `core::ptr::copy_nonoverlapping::<u8>` 0.5%
  - `<php_types::zstr::ZStr as core::ops::deref::Deref>::deref` 0.4%
  - `memcpy@plt` 0.3%
  - `<core::str::lossy::Utf8Chunks as core::iter::traits::iterator::Iterator>::next` 0.1%
  - `bcmp@plt` 0.1%
  - `<[u8] as core::cmp::PartialEq<[u8` 0.1%
  - `core::str::validations::run_utf8_validation` 0.1%
- **dispatch** 15.1%
  - `<php_runtime::vm::Vm>::run_loop` 8.2%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 0.5%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 0.5%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 0.5%
  - `<php_runtime::vm::Vm>::enter_callee` 0.5%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::capacity` 0.4%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::pop` 0.4%
  - `php_runtime::vm::calls::decay_arg` 0.3%
- **builtin_args** 0.8%
  - `<php_runtime::vm::Vm>::run_value_builtin` 0.4%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_runtime::builtin::Builtin, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.1%
  - `<php_runtime::vm::Vm>::value_builtin_call` 0.1%
  - `<php_runtime::builtin::Ctx>::to_zstr` 0.1%
- **builtin_body** 2.0%
  - `fancy_regex::vm::run` 0.1%
  - `md5::compress::soft::op_g` 0.1%
  - `md5::compress::soft::op_i` 0.1%
  - `php_builtins::json::encode_array::{closure#2}` 0.1%
  - `<regex_automata::meta::regex::CapturesMatches as core::iter::traits::iterator::Iterator>::next::{closure#0}` 0.1%
  - `regex_automata::util::determinize::next` 0.1%
  - `regex_automata::util::determinize::state::write_varu32` 0.1%
- **compile** 36.1%
  - `under php_runtime::lower::lower_source` 7.5%
  - `under php_runtime::compile::compile_program` 4.5%
  - `under <php_runtime::vm::Vm>::compile_unit_module` 4.3%
  - `under <php_runtime::vm::Vm>::lower_unit` 4.1%
  - `under php_runtime::lower::lower_source_seeded` 1.8%
  - `under php_runtime::lower::lower_source_impl` 1.6%
  - `under mago_syntax::parser::parse_file_content` 1.6%
  - `under <php_runtime::lower::Lowerer>::lower_stmt` 1.4%
- **operators** 1.1%
  - `php_types::convert::to_bool` 0.4%
  - `<php_runtime::vm::Vm>::binary_value_ab` 0.2%
  - `php_types::convert::to_zstr` 0.1%
  - `php_runtime::vm::run::binary_fast` 0.1%
- **vm_body** 10.3%
  - `php_runtime::vm::host_builtin_canonical` 2.5%
  - `<php_runtime::vm::Vm>::prop_set_entry::<true>` 0.6%
  - `php_runtime::vm::oop::resolve_method_runtime` 0.5%
  - `<php_runtime::vm::LcKey>::new` 0.5%
  - `<php_runtime::vm::Vm>::coerce_or_check_hint` 0.4%
  - `php_runtime::vm::oop::deref_object` 0.3%
  - `<php_types::object::PropsLayout>::slot_of` 0.3%
  - `php_runtime::vm::oop::resolve_method_runtime::{closure#2}` 0.3%
- **gc** 1.2%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.3%
  - `<php_runtime::vm::Vm>::gc_note_frame` 0.3%
  - `<php_runtime::vm::Vm>::gc_sweep_body` 0.1%
  - `<php_runtime::vm::Vm>::gc_sweep_impl` 0.1%
  - `<php_runtime::vm::Vm>::sweep_idle` 0.1%
- **symtab** 2.7%
  - `<[(u64, u32)]>::binary_search_by::<<[(u64, u32)]>::partition_point<<php_runtime::bytecode::Module>::find_fn_ci::{clos...` 0.3%
  - `rustc_hash::multiply_mix` 0.3%
  - `php_runtime::bytecode::ci_hash` 0.3%
  - `<hashbrown::control::group::neon::Group>::match_tag` 0.3%
  - `<php_runtime::bytecode::Module>::find_fn_ci` 0.2%
  - `rustc_hash::hash_bytes` 0.2%
  - `<hashbrown::raw::RawTableInner>::find_inner` 0.1%
  - `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` 0.1%
- **kernel** 2.6%
  - `(kernel) blk_mq_end_request` 0.6%
  - `(kernel) __arch_copy_to_user` 0.3%
  - `(kernel) path_lookupat` 0.3%
  - `(kernel) __pi_memset_generic` 0.1%
  - `(kernel) local_daif_restore` 0.1%
  - `(kernel) __d_lookup_rcu` 0.1%
  - `(kernel) change_protection` 0.1%
  - `(kernel) do_el0_svc` 0.1%
- **startup_io** 2.4%
  - `[ld-linux-aarch64.so.1]` 2.1%
  - `<std::sys::thread_local::native::lazy::Storage<core::cell::Cell<(u64, u64)>, !>>::get_or_init::<<std::hash::random::R...` 0.1%
  - `main` 0.1%

Top leaf frames (self time, % of all samples):

- `<php_runtime::vm::Vm>::run_loop` 7.7%
- `__pi_clear_page` 7.4%
- `blk_mq_end_request` 3.9%
- `[libc.so.6]` 3.3%
- `__pi_caches_clean_inval_pou` 2.3%
- `<core::slice::iter::Iter<&[u8]> as core::iter::traits::iterator::Iterator>::next` 2.2%
- `[ld-linux-aarch64.so.1]` 1.9%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 1.6%
- `blk_update_request` 1.3%
- `memcmp` 1.2%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 1.2%
- `<mago_syntax::ast::ast::statement::Statement as mago_span::HasSpan>::span` 1.1%
- `<core::iter::adapters::zip::Zip<core::slice::iter::IterMut<u8>, core::slice::iter::Iter<u8>> as core::iter::adapters:...` 0.8%
- `mi_theap_malloc_zero_aligned_at` 0.7%
- `mi_page_malloc_zero` 0.7%
- `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` 0.7%
- `<mago_syntax::lexer::Lexer>::advance` 0.7%
- `<u8>::is_ascii_uppercase` 0.7%
- `<mago_syntax_core::input::Input>::scan_identifier` 0.6%
- `mi_block_set_nextx` 0.6%
- `<[u8]>::make_ascii_lowercase` 0.6%
- `<alloc::raw_vec::RawVecInner>::capacity` 0.6%
- `core::mem::replace::<usize>` 0.6%
- `memcpy@plt` 0.5%
- `<core::slice::iter::Iter<&[u8]> as core::iter::traits::iterator::Iterator>::try_fold::<(), core::iter::adapters::copi...` 0.5%
