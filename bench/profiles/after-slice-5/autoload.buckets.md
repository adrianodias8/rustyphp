| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 2.3 |
| `RefCell` borrow / borrow_mut checks | 0.4 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 4.8 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 0.1 |
| string hashing / comparison / copying / formatting | 1.2 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 2.7 |
| argument passing into builtins (lookup by name, pre-call checks) | 0.4 |
| the builtin bodies themselves | 0.4 |
| parser / HIR / compile (script + prelude + include units), per run | 72.8 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 0.3 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 3.5 |
| other: GC bookkeeping and the per-statement destructor sweep | 0.3 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 1.0 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 7.8 |
| other: process startup, libc, std I/O | 1.9 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 2.3%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 0.5%
  - `core::ptr::drop_glue::<[alloc::rc::Rc<php_runtime::hir::ClassDecl>]>` 0.4%
  - `<alloc::rc::RcInner<php_runtime::hir::ClassDecl> as alloc::rc::RcInnerPtr>::dec_strong` 0.3%
  - `core::ptr::drop_glue::<[alloc::rc::Rc<php_runtime::bytecode::Func>]>` 0.2%
  - `<php_types::zval::Zval>::deref_clone` 0.1%
  - `<alloc::rc::RcInner<php_runtime::hir::FnDecl> as alloc::rc::RcInnerPtr>::dec_strong` 0.1%
  - `<core::option::Option<&alloc::rc::Rc<php_runtime::bytecode::Func>>>::is_some_and::<<php_runtime::vm::Vm>::run_linked:...` 0.1%
  - `<core::iter::adapters::enumerate::Enumerate<core::slice::iter::Iter<alloc::rc::Rc<php_runtime::bytecode::Func>>> as c...` 0.1%
- **refcell** 0.4%
  - `<alloc::boxed::Box<alloc::rc::RcInner<core::cell::RefCell<php_types::zval::Zval>>>>::new` 0.1%
  - `<core::cell::BorrowRef>::new` 0.1%
  - `core::ptr::drop_glue::<alloc::vec::drain::Drain<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::obj...` 0.1%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Mut, u32, alloc::rc::Rc<core::cell...` 0.1%
- **alloc** 4.8%
  - `(kernel) page fault / mm: el0_da` 0.7%
  - `mi_free_block_local` 0.6%
  - `mi_block_nextx` 0.5%
  - `mi_free` 0.4%
  - `(kernel) page fault / mm: folio_mark_accessed` 0.2%
  - `(kernel) page fault / mm: filemap_add_folio` 0.2%
  - `mi_page_malloc_zero` 0.2%
  - `mi_block_set_nextx` 0.2%
- **hash** 0.1%
- **string** 1.2%
  - `memcmp` 0.4%
  - `<[u8] as core::cmp::PartialEq<[u8` 0.2%
  - `memcpy@plt` 0.2%
  - `core::ptr::copy_nonoverlapping::<u8>` 0.1%
  - `bcmp@plt` 0.1%
- **dispatch** 2.7%
  - `<php_runtime::vm::Vm>::run_loop` 1.0%
  - `core::ptr::write::<php_runtime::vm::Frame>` 0.2%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 0.2%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 0.2%
  - `<php_runtime::bytecode::Const>::to_zval` 0.1%
  - `<usize as core::slice::index::SliceIndex<[php_runtime::vm::Frame]>>::index` 0.1%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::pop` 0.1%
  - `core::ptr::drop_glue::<core::option::Option<alloc::boxed::Box<php_runtime::vm::FrameExt>>>` 0.1%
- **builtin_args** 0.4%
  - `<php_runtime::vm::Vm>::run_value_builtin` 0.1%
  - `<php_runtime::vm::Vm>::value_builtin_call` 0.1%
  - `<php_runtime::vm::Vm>::compute_stringify::any_object` 0.1%
- **builtin_body** 0.4%
  - `php_builtins::null_arg_deprecation` 0.1%
  - `php_builtins::math::intdiv` 0.1%
  - `php_builtins::format::format_impl` 0.1%
  - `php_builtins::format::max_arg_index` 0.1%
- **compile** 72.8%
  - `under <php_runtime::vm::Vm>::compile_unit_module` 40.3%
  - `under <php_runtime::vm::Vm>::lower_unit` 16.6%
  - `under php_runtime::vm::unit_cache_put` 2.7%
  - `under <php_runtime::vm::Vm>::unit_remap_elided` 2.2%
  - `under php_runtime::lower::lower_source_impl` 1.7%
  - `under php_runtime::lower::lower_source` 1.5%
  - `under php_runtime::compile::compile_program` 1.2%
  - `under <php_runtime::vm::Vm>::seed_stub_mask` 1.2%
- **operators** 0.3%
  - `php_types::convert::to_zstr` 0.1%
  - `<php_runtime::vm::Vm>::binary_value_ab` 0.1%
  - `php_types::convert::to_long_cast` 0.1%
- **vm_body** 3.5%
  - `<php_types::object::PropsLayout>::slot_of` 0.4%
  - `<php_runtime::vm::Vm>::resolve_dynamic_class` 0.3%
  - `<php_runtime::vm::Vm>::run_linked` 0.2%
  - `<php_runtime::vm::Vm>::prop_set_entry::<true>` 0.2%
  - `php_runtime::vm::oop::deref_object` 0.2%
  - `<php_runtime::vm::Vm>::run_include` 0.2%
  - `php_runtime::vm::oop::resolve_prop_access` 0.2%
  - `<php_runtime::vm::Vm>::call_ns_fallback_site` 0.1%
- **gc** 0.3%
  - `<php_runtime::vm::Vm>::gc_note_frame` 0.1%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.1%
- **symtab** 1.0%
  - `rustc_hash::hash_bytes` 0.1%
  - `<hashbrown::map::HashMap<usize, (), std::hash::random::RandomState>>::contains_key::<usize>` 0.1%
  - `<hashbrown::raw::RawTableInner>::is_bucket_full` 0.1%
  - `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` 0.1%
  - `rustc_hash::multiply_mix` 0.1%
  - `<hashbrown::raw::RawTable<(alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo)>>::find::<hashbrown::map::equiva...` 0.1%
  - `<hashbrown::control::group::neon::Group>::load` 0.1%
  - `<hashbrown::control::bitmask::BitMask>::nonzero_trailing_zeros` 0.1%
- **kernel** 7.8%
  - `(kernel) blk_mq_end_request` 1.0%
  - `(kernel) path_lookupat` 0.8%
  - `(kernel) local_daif_restore` 0.7%
  - `(kernel) __pi_memset_generic` 0.7%
  - `(kernel) blk_update_request` 0.6%
  - `(kernel) virtio_queue_rq` 0.3%
  - `(kernel) generic_permission` 0.3%
  - `(kernel) security_inode_getattr` 0.2%
- **startup_io** 1.9%
  - `std::sys::fs::unix::canonicalize` 0.7%
  - `[ld-linux-aarch64.so.1]` 0.5%
  - `std::sys::fs::unix::try_statx::statx` 0.2%
  - `<std::sys::fd::unix::FileDesc>::read` 0.1%
  - `<std::sys::fd::unix::FileDesc>::read_buf` 0.1%

Top leaf frames (self time, % of all samples):

- `__pi_clear_page` 5.2%
- `[libc.so.6]` 3.7%
- `<hashbrown::raw::ProbeSeq>::move_next` 3.1%
- `core::ptr::read::<core::ptr::Packed<core::core_arch::arm_shared::neon::uint8x8_t>>` 2.6%
- `core::core_arch::arm_shared::neon::generated::vreinterpret_u64_u8` 2.4%
- `<hashbrown::control::bitmask::BitMask>::lowest_set_bit` 2.3%
- `memcmp` 2.3%
- `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, usize, rustc_hash::FxBuildHasher>>::rustc_entry` 1.9%
- `mi_theap_malloc_zero_aligned_at` 1.6%
- `mi_free_block_local` 1.6%
- `<php_runtime::vm::Vm>::unit_remap_elided` 1.6%
- `mi_page_malloc_zero` 1.4%
- `<u8>::make_ascii_lowercase` 1.3%
- `<u128>::wrapping_mul` 1.2%
- `core::mem::replace::<usize>` 1.2%
- `rustc_hash::hash_bytes` 1.0%
- `<mago_syntax::lexer::Lexer>::advance` 1.0%
- `blk_mq_end_request` 1.0%
- `_mi_unchecked_ptr_page` 0.9%
- `<&alloc::vec::Vec<u8> as core::hash::Hash>::hash::<rustc_hash::FxHasher>` 0.9%
- `php_runtime::compile::compile_program_impl_mode` 0.9%
- `mi_free` 0.9%
- `<core::iter::adapters::zip::Zip<core::slice::iter::IterMut<u8>, core::slice::iter::Iter<u8>> as core::iter::adapters:...` 0.9%
- `<php_runtime::vm::Vm>::run_loop` 0.8%
- `core::ptr::copy_nonoverlapping::<(u64, u32)>` 0.8%
