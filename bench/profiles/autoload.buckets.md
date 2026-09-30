| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 1.6 |
| `RefCell` borrow / borrow_mut checks | 0.2 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 3.1 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 0.1 |
| string hashing / comparison / copying / formatting | 0.6 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 1.8 |
| argument passing into builtins (lookup by name, pre-call checks) | 0.2 |
| the builtin bodies themselves | 0.2 |
| parser / HIR / compile (script + prelude + include units), per run | 84.8 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 0.1 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 3.0 |
| other: GC bookkeeping and the per-statement destructor sweep | 0.2 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 1.1 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 2.1 |
| other: process startup, libc, std I/O | 0.9 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 1.6%
  - `core::ptr::drop_glue::<[alloc::rc::Rc<php_runtime::hir::ClassDecl>]>` 0.3%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 0.2%
  - `<alloc::rc::RcInner<php_runtime::hir::ClassDecl> as alloc::rc::RcInnerPtr>::dec_strong` 0.2%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 0.1%
  - `<alloc::rc::RcInner<php_runtime::hir::FnDecl> as alloc::rc::RcInnerPtr>::dec_strong` 0.1%
  - `core::ptr::drop_glue::<[alloc::rc::Rc<php_runtime::bytecode::Func>]>` 0.1%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<php_runtime::bytecode::Func>>>::as_ref` 0.1%
  - `<php_types::zstr::ZStr as core::ops::drop::Drop>::drop` 0.1%
- **refcell** 0.2%
  - `<core::cell::BorrowRef>::new` 0.1%
  - `<core::ptr::non_null::NonNull<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::as_ref` 0.1%
- **alloc** 3.1%
  - `(kernel) page fault / mm: el0_da` 0.8%
  - `mi_block_set_nextx` 0.5%
  - `mi_free_block_local` 0.3%
  - `mi_block_nextx` 0.3%
  - `mi_free` 0.2%
  - `(kernel) page fault / mm: do_mem_abort` 0.1%
  - `_mi_unchecked_ptr_page` 0.1%
  - `mi_page_malloc_zero` 0.1%
- **hash** 0.1%
- **string** 0.6%
  - `memcmp` 0.1%
  - `core::ptr::copy_nonoverlapping::<u8>` 0.1%
  - `<[u8] as core::cmp::PartialEq<[u8` 0.1%
  - `bcmp@plt` 0.1%
- **dispatch** 1.8%
  - `<php_runtime::vm::Vm>::run_loop` 0.7%
  - `core::ptr::write::<php_runtime::vm::Frame>` 0.2%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 0.1%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 0.1%
  - `<php_runtime::vm::Vm>::recycle_frame` 0.1%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::ptr` 0.1%
  - `php_runtime::vm::calls::bind_params` 0.1%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 0.1%
- **builtin_args** 0.2%
  - `<php_runtime::vm::Vm>::run_value_builtin` 0.1%
- **builtin_body** 0.2%
- **compile** 84.8%
  - `under <php_runtime::vm::Vm>::lower_unit` 32.4%
  - `under <php_runtime::vm::Vm>::compile_unit_module` 22.3%
  - `under <php_runtime::vm::Vm>::unit_remap_elided` 7.9%
  - `under <php_runtime::vm::Vm>::unit_fp` 7.6%
  - `under <php_runtime::vm::Vm>::seed_stub_mask` 7.5%
  - `under php_runtime::vm::unit_cache_put` 1.9%
  - `under php_runtime::compile::compile_program` 0.9%
  - `under php_runtime::lower::lower_source` 0.9%
- **operators** 0.1%
- **vm_body** 3.0%
  - `php_runtime::vm::host_builtin_canonical` 0.6%
  - `<php_runtime::vm::Vm>::resolve_dynamic_class` 0.3%
  - `<php_types::object::PropsLayout>::slot_of` 0.3%
  - `php_runtime::vm::oop::resolve_method_runtime` 0.3%
  - `<php_runtime::vm::Vm>::run_linked` 0.2%
  - `php_runtime::vm::host_builtin_canonical::{closure#0}` 0.1%
  - `<php_runtime::vm::Vm>::prop_set_entry::<true>` 0.1%
  - `<php_runtime::vm::Vm>::dispatch_instance_call` 0.1%
- **gc** 0.2%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.1%
- **symtab** 1.1%
  - `<hashbrown::control::group::neon::Group>::load` 0.4%
  - `<php_runtime::bytecode::Module>::find_fn_ci` 0.1%
  - `rustc_hash::hash_bytes` 0.1%
  - `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` 0.1%
  - `<hashbrown::map::HashMap<alloc::boxed::Box<[u8]>, php_runtime::bytecode::PropInfo, rustc_hash::FxBuildHasher>>::get::...` 0.1%
  - `rustc_hash::multiply_mix` 0.1%
  - `<hashbrown::map::HashMap<usize, (), std::hash::random::RandomState>>::contains_key::<usize>` 0.1%
  - `<hashbrown::control::group::neon::Group>::match_tag` 0.1%
- **kernel** 2.1%
  - `(kernel) path_lookupat` 0.3%
  - `(kernel) local_daif_restore` 0.3%
  - `(kernel) __arch_copy_to_user` 0.2%
  - `(kernel) __kernel_clock_gettime` 0.2%
  - `(kernel) __d_lookup_rcu` 0.2%
  - `(kernel) do_el0_svc` 0.1%
  - `(kernel) btrfs_getattr` 0.1%
  - `(kernel) generic_permission` 0.1%
- **startup_io** 0.9%
  - `[ld-linux-aarch64.so.1]` 0.5%
  - `std::sys::fs::unix::canonicalize` 0.1%
  - `std::sys::fs::unix::try_statx::statx` 0.1%
  - `<std::sys::fd::unix::FileDesc>::read_buf` 0.1%

Top leaf frames (self time, % of all samples):

- `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` 5.0%
- `<[u8]>::make_ascii_lowercase` 4.0%
- `core::ptr::read::<core::ptr::Packed<core::core_arch::arm_shared::neon::uint8x8_t>>` 3.9%
- `[libc.so.6]` 3.6%
- `memcmp` 3.5%
- `core::core_arch::arm_shared::neon::generated::vreinterpret_u64_u8` 3.5%
- `__pi_clear_page` 3.3%
- `<hashbrown::raw::ProbeSeq>::move_next` 2.9%
- `<hashbrown::control::bitmask::BitMask>::lowest_set_bit` 2.6%
- `<u128>::wrapping_mul` 1.9%
- `mi_theap_malloc_zero_aligned_at` 1.7%
- `mi_page_malloc_zero` 1.7%
- `mi_free_block_local` 1.6%
- `core::intrinsics::likely` 1.5%
- `rustc_hash::hash_bytes` 1.5%
- `mi_block_set_nextx` 1.3%
- `<u8>::make_ascii_lowercase` 1.3%
- `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, usize, rustc_hash::FxBuildHasher>>::rustc_entry` 1.2%
- `_mi_unchecked_ptr_page` 1.2%
- `core::hash::sip::u8to64_le` 1.1%
- `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&alloc::vec::Vec<u8>>` 1.1%
- `core::ptr::copy_nonoverlapping::<u8>` 1.0%
- `<&alloc::vec::Vec<u8> as core::hash::Hash>::hash::<rustc_hash::FxHasher>` 0.9%
- `mi_theap_malloc_aligned` 0.9%
- `memcpy@plt` 0.9%
