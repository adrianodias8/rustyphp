| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 19.5 |
| `RefCell` borrow / borrow_mut checks | 0.4 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 11.3 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 14.0 |
| string hashing / comparison / copying / formatting | 2.2 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 30.5 |
| argument passing into builtins (lookup by name, pre-call checks) | 1.2 |
| the builtin bodies themselves | 1.1 |
| parser / HIR / compile (script + prelude + include units), per run | 0.6 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 6.2 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 10.6 |
| other: GC bookkeeping and the per-statement destructor sweep | 1.0 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 1.2 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 0.1 |
| other: process startup, libc, std I/O | 0.1 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 19.5%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 7.1%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 6.8%
  - `<php_types::array::PhpArray as core::ops::drop::Drop>::drop` 1.2%
  - `<php_types::zval::Zval>::deref_clone` 0.8%
  - `core::ptr::drop_glue::<(php_types::array::Key, php_types::zval::Zval)>` 0.7%
  - `<php_types::zstr::ZStr as core::clone::Clone>::clone` 0.5%
  - `core::ptr::drop_glue::<alloc::vec::Vec<php_types::zval::Zval>>` 0.4%
  - `<php_types::zstr::ZStr as core::ops::drop::Drop>::drop` 0.3%
- **refcell** 0.4%
  - `core::ptr::drop_glue::<core::option::Option<alloc::rc::Rc<core::cell::RefCell<php_types::zval::Zval>>>>` 0.1%
  - `<core::cell::BorrowRef>::new` 0.1%
- **alloc** 11.3%
  - `_mi_memcpy` 2.5%
  - `<alloc::vec::Vec<(php_types::array::Key, php_types::zval::Zval)>>::extend_desugared::<core::iter::adapters::map::Map<...` 2.2%
  - `(kernel) page fault / mm: el0_da` 1.0%
  - `mi_theap_malloc_zero_aligned_at` 0.6%
  - `mi_free_block_local` 0.6%
  - `mi_theap_malloc_aligned` 0.6%
  - `mi_free` 0.5%
  - `_mi_unchecked_ptr_page` 0.5%
- **hash** 14.0%
  - `<php_types::array::PhpArray>::insert` 3.9%
  - `<php_types::array::KeyIndex>::lookup` 2.2%
  - `<php_types::array::PhpArray>::append` 2.1%
  - `<alloc::vec::Vec<(php_types::array::Key, php_types::zval::Zval)>>::set_len` 2.0%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next` 0.8%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next::{closure#0}` 0.7%
  - `<php_types::array::PhpArray>::contains_key` 0.4%
  - `<php_types::array::KeyIndex>::insert_new` 0.3%
- **string** 2.2%
  - `memcmp` 0.6%
  - `<php_types::zstr::PhpStr>::from_i64` 0.4%
  - `core::ptr::copy_nonoverlapping::<u8>` 0.3%
  - `memcpy@plt` 0.2%
  - `<*const php_types::zstr::PhpStr>::add` 0.2%
  - `<php_types::zstr::PhpStr>::zhash` 0.1%
  - `<php_types::zstr::PhpStr>::as_bytes` 0.1%
  - `bcmp@plt` 0.1%
- **dispatch** 30.5%
  - `<php_runtime::vm::Vm>::run_loop` 11.0%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 2.2%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 2.0%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.3%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 1.2%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 1.0%
  - `<php_runtime::vm::Vm>::drive_to_return` 0.9%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::capacity` 0.9%
- **builtin_args** 1.2%
  - `<php_runtime::vm::Vm>::run_value_builtin` 0.6%
  - `<php_runtime::vm::Vm>::value_builtin_call` 0.2%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_runtime::builtin::Builtin, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.1%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_runtime::builtin::Builtin)>>::len` 0.1%
  - `<php_runtime::vm::Vm>::compute_stringify` 0.1%
- **builtin_body** 1.1%
  - `php_types::ops::stable_sort_by::<php_types::zval::Zval, php_builtins::array::sort::{closure#2}>` 0.5%
  - `<php_runtime::vm::Vm>::ho_array_filter` 0.2%
  - `php_builtins::math::mt_next` 0.1%
  - `php_builtins::math::mt_rand` 0.1%
  - `php_builtins::var::strlen` 0.1%
- **compile** 0.6%
  - `under php_runtime::lower::lower_source` 0.2%
  - `under php_runtime::compile::compile_program` 0.2%
- **operators** 6.2%
  - `php_types::ops::compare` 3.1%
  - `php_runtime::vm::run::binary_fast` 0.8%
  - `php_types::convert::to_bool` 0.5%
  - `php_runtime::vm::run::concat_n_join` 0.5%
  - `php_types::convert::to_zstr` 0.2%
  - `<php_runtime::vm::Vm>::incdec_slot_discard` 0.2%
  - `<php_runtime::vm::Vm>::incdec_slot_discard_slow` 0.2%
  - `php_types::ops::identical` 0.2%
- **vm_body** 10.6%
  - `php_runtime::vm::path_apply` 2.9%
  - `php_runtime::vm::path_walk` 1.0%
  - `<php_runtime::vm::Vm>::vm_merge_sort_with` 0.8%
  - `<php_runtime::vm::Vm>::path_op` 0.7%
  - `php_runtime::vm::apply_last` 0.5%
  - `<php_runtime::vm::Vm>::gc_classify` 0.4%
  - `<php_runtime::vm::Vm>::concat_n` 0.3%
  - `php_runtime::vm::arrays::read_slot` 0.3%
- **gc** 1.0%
  - `<php_runtime::vm::Vm>::sweep_idle` 0.2%
  - `<php_runtime::vm::Vm>::gc_note_frame` 0.2%
  - `<php_types::zval::Zval>::is_gc_container` 0.2%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.2%
  - `<php_runtime::vm::Vm>::gc_sweep_body` 0.1%
  - `<php_runtime::vm::Vm>::sweep_skip_next` 0.1%
- **symtab** 1.2%
  - `<hashbrown::control::group::neon::Group>::match_tag` 0.3%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_types::zval::Zval, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.2%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_types::zval::Zval)>>::len` 0.1%
  - `rustc_hash::hash_bytes` 0.1%
  - `<hashbrown::raw::RawTableInner>::is_empty_singleton` 0.1%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_types::zval::Zval)>>::get::<hashbrown::map::equivalent_key<[u8], ...` 0.1%
  - `<hashbrown::control::group::neon::Group>::load` 0.1%
  - `core::mem::replace::<std::collections::hash::map::HashMap<u32, php_types::zstr::ZStr>>` 0.1%
- **kernel** 0.1%
- **startup_io** 0.1%
  - `[ld-linux-aarch64.so.1]` 0.1%

Top leaf frames (self time, % of all samples):

- `<php_runtime::vm::Vm>::run_loop` 10.0%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 7.1%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 6.8%
- `<php_types::array::PhpArray>::insert` 3.1%
- `php_types::ops::compare` 3.1%
- `[libc.so.6]` 3.0%
- `php_runtime::vm::path_apply` 2.6%
- `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 2.2%
- `<php_types::array::KeyIndex>::lookup` 2.2%
- `<php_types::array::PhpArray>::append` 2.1%
- `<alloc::vec::Vec<(php_types::array::Key, php_types::zval::Zval)>>::extend_desugared::<core::iter::adapters::map::Map<...` 2.1%
- `<alloc::vec::Vec<(php_types::array::Key, php_types::zval::Zval)>>::set_len` 2.0%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 2.0%
- `<alloc::raw_vec::RawVecInner>::capacity` 1.4%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.3%
- `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 1.2%
- `__pi_clear_page` 1.1%
- `core::mem::replace::<usize>` 1.1%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 1.0%
- `<php_runtime::vm::Vm>::drive_to_return` 0.9%
- `php_runtime::vm::path_walk` 0.8%
- `<core::option::Option<php_types::zval::Zval>>::as_ref` 0.8%
- `<php_runtime::vm::Vm>::vm_merge_sort_with` 0.8%
- `php_runtime::vm::run::binary_fast` 0.8%
- `<php_types::zval::Zval>::deref_clone` 0.8%
