| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 1.3 |
| `RefCell` borrow / borrow_mut checks | 0.2 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 2.4 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 0.1 |
| string hashing / comparison / copying / formatting | 0.4 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 1.6 |
| argument passing into builtins (lookup by name, pre-call checks) | 0.1 |
| the builtin bodies themselves | 0.1 |
| parser / HIR / compile (script + prelude + include units), per run | 86.9 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 0.1 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 2.1 |
| other: GC bookkeeping and the per-statement destructor sweep | 0.2 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 0.6 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 3.4 |
| other: process startup, libc, std I/O | 0.7 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 1.3%
  - `<alloc::rc::RcInner<php_runtime::hir::ClassDecl> as alloc::rc::RcInnerPtr>::dec_strong` 0.2%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 0.2%
  - `core::ptr::drop_glue::<[alloc::rc::Rc<php_runtime::hir::ClassDecl>]>` 0.1%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 0.1%
  - `core::ptr::drop_glue::<[alloc::rc::Rc<php_runtime::bytecode::Func>]>` 0.1%
  - `<php_types::zstr::ZStr as core::ops::drop::Drop>::drop` 0.1%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<php_runtime::bytecode::Func>>>::as_ref` 0.1%
  - `<alloc::rc::RcInner<php_runtime::bytecode::Func> as alloc::rc::RcInnerPtr>::dec_strong` 0.1%
- **refcell** 0.2%
- **alloc** 2.4%
  - `mi_free_block_local` 0.3%
  - `mi_block_nextx` 0.3%
  - `(kernel) page fault / mm: el0_da` 0.3%
  - `mi_block_set_nextx` 0.3%
  - `(kernel) page fault / mm: folio_mark_accessed` 0.1%
  - `mi_free` 0.1%
  - `(kernel) page fault / mm: __arm64_sys_madvise` 0.1%
  - `mi_page_xthread_id` 0.1%
- **hash** 0.1%
- **string** 0.4%
  - `memcmp` 0.1%
  - `<[u8] as core::cmp::PartialEq<[u8` 0.1%
- **dispatch** 1.6%
  - `<php_runtime::vm::Vm>::run_loop` 0.6%
  - `<php_runtime::vm::Vm>::recycle_frame` 0.1%
  - `<php_runtime::vm::Vm>::enter_callee` 0.1%
  - `php_runtime::vm::calls::bind_params` 0.1%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 0.1%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 0.1%
  - `core::ptr::write::<php_runtime::vm::Frame>` 0.1%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::ptr` 0.1%
- **builtin_args** 0.1%
- **builtin_body** 0.1%
- **compile** 86.9%
  - `under <php_runtime::vm::Vm>::lower_unit` 41.0%
  - `under <php_runtime::vm::Vm>::compile_unit_module` 18.6%
  - `under <php_runtime::vm::Vm>::unit_fp` 6.9%
  - `under <php_runtime::vm::Vm>::seed_stub_mask` 6.9%
  - `under <php_runtime::vm::Vm>::unit_remap_elided` 6.8%
  - `under php_runtime::vm::unit_cache_put` 1.7%
  - `under php_runtime::lower::lower_source` 1.2%
  - `under php_runtime::compile::compile_program` 0.9%
- **operators** 0.1%
- **vm_body** 2.1%
  - `php_runtime::vm::host_builtin_canonical` 0.2%
  - `php_runtime::vm::oop::resolve_method_runtime` 0.1%
  - `<php_runtime::vm::Vm>::resolve_dynamic_class` 0.1%
  - `php_runtime::vm::host_builtin_canonical::{closure#0}` 0.1%
  - `<php_types::object::PropsLayout>::slot_of` 0.1%
  - `<php_runtime::vm::Vm>::run_linked` 0.1%
  - `<php_runtime::vm::Vm>::prop_set_entry::<true>` 0.1%
  - `<php_runtime::vm::Vm>::coerce_or_check_hint` 0.1%
- **gc** 0.2%
- **symtab** 0.6%
  - `<hashbrown::control::group::neon::Group>::load` 0.1%
  - `<[(u64, u32)]>::binary_search_by::<<[(u64, u32)]>::partition_point<<php_runtime::bytecode::Module>::find_fn_ci::{clos...` 0.1%
- **kernel** 3.4%
  - `(kernel) blk_mq_end_request` 0.6%
  - `(kernel) local_daif_restore` 0.4%
  - `(kernel) path_lookupat` 0.3%
  - `(kernel) security_inode_getattr` 0.2%
  - `(kernel) _raw_read_unlock` 0.1%
  - `(kernel) [[vdso]]` 0.1%
  - `(kernel) do_el0_svc` 0.1%
  - `(kernel) security_inode_permission` 0.1%
- **startup_io** 0.7%
  - `[ld-linux-aarch64.so.1]` 0.4%
  - `std::sys::fs::unix::canonicalize` 0.2%

Top leaf frames (self time, % of all samples):

- `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` 10.1%
- `<[u8]>::make_ascii_lowercase` 4.2%
- `core::core_arch::arm_shared::neon::generated::vreinterpret_u64_u8` 3.7%
- `<hashbrown::raw::ProbeSeq>::move_next` 3.5%
- `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&alloc::vec::Vec<u8>>` 3.0%
- `core::hash::sip::u8to64_le` 3.0%
- `[libc.so.6]` 2.9%
- `memcmp` 2.9%
- `core::ptr::read::<core::ptr::Packed<core::core_arch::arm_shared::neon::uint8x8_t>>` 2.8%
- `<hashbrown::control::bitmask::BitMask>::lowest_set_bit` 2.7%
- `__pi_clear_page` 2.6%
- `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, usize, std::hash::random::RandomState>>::rustc_entry` 2.0%
- `<core::hash::sip::Sip13Rounds as core::hash::sip::Sip>::c_rounds` 1.7%
- `mi_theap_malloc_zero_aligned_at` 1.6%
- `core::ptr::copy_nonoverlapping::<u8>` 1.5%
- `rustc_hash::hash_bytes` 1.4%
- `<u64>::wrapping_add` 1.3%
- `mi_page_malloc_zero` 1.3%
- `mi_free_block_local` 1.3%
- `core::intrinsics::likely` 1.3%
- `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::finish` 0.9%
- `<hashbrown::raw::RawTableInner>::is_bucket_full` 0.9%
- `<u128>::wrapping_mul` 0.9%
- `mi_block_set_nextx` 0.8%
- `_mi_unchecked_ptr_page` 0.8%
