| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 19.6 |
| `RefCell` borrow / borrow_mut checks | 0.3 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 12.0 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 14.2 |
| string hashing / comparison / copying / formatting | 2.7 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 28.4 |
| argument passing into builtins (lookup by name, pre-call checks) | 1.0 |
| the builtin bodies themselves | 1.1 |
| parser / HIR / compile (script + prelude + include units), per run | 0.6 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 5.3 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 11.8 |
| other: GC bookkeeping and the per-statement destructor sweep | 1.5 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 1.4 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 0.1 |
| other: process startup, libc, std I/O | 0.1 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 19.6%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 6.9%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 6.9%
  - `<php_types::array::PhpArray as core::ops::drop::Drop>::drop` 1.1%
  - `<php_types::zval::Zval>::deref_clone` 0.9%
  - `<php_types::zstr::ZStr as core::clone::Clone>::clone` 0.6%
  - `core::ptr::drop_glue::<(php_types::array::Key, php_types::zval::Zval)>` 0.6%
  - `core::ptr::drop_glue::<alloc::vec::Vec<php_types::zval::Zval>>` 0.4%
  - `<php_types::zstr::ZStr as core::ops::drop::Drop>::drop` 0.3%
- **refcell** 0.3%
- **alloc** 12.0%
  - `_mi_memcpy` 2.7%
  - `<alloc::vec::Vec<(php_types::array::Key, php_types::zval::Zval)>>::extend_desugared::<core::iter::adapters::map::Map<...` 2.2%
  - `(kernel) page fault / mm: el0_da` 1.1%
  - `mi_theap_malloc_zero_aligned_at` 0.6%
  - `mi_free_block_local` 0.6%
  - `mi_free` 0.5%
  - `_mi_unchecked_ptr_page` 0.5%
  - `mi_page_malloc_zero` 0.4%
- **hash** 14.2%
  - `<php_types::array::PhpArray>::insert` 3.7%
  - `<php_types::array::KeyIndex>::lookup` 2.2%
  - `<php_types::array::PhpArray>::append` 2.0%
  - `<alloc::vec::Vec<(php_types::array::Key, php_types::zval::Zval)>>::set_len` 2.0%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next` 0.8%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next::{closure#0}` 0.6%
  - `<php_types::array::PhpArray>::contains_key` 0.3%
  - `<php_types::array::KeyIndex>::insert_new` 0.3%
- **string** 2.7%
  - `memcmp` 0.7%
  - `<php_types::zstr::PhpStr>::from_i64` 0.5%
  - `<*const php_types::zstr::PhpStr>::add` 0.4%
  - `core::ptr::copy_nonoverlapping::<u8>` 0.3%
  - `memcpy@plt` 0.2%
  - `<php_types::zstr::PhpStr>::zhash` 0.2%
  - `<php_types::zstr::PhpStr>::as_bytes` 0.2%
  - `bcmp@plt` 0.1%
- **dispatch** 28.4%
  - `<php_runtime::vm::Vm>::run_loop` 9.9%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 1.9%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 1.8%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 1.3%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::capacity` 1.1%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.0%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 0.9%
  - `core::ptr::write::<php_runtime::vm::Frame>` 0.8%
- **builtin_args** 1.0%
  - `<php_runtime::vm::Vm>::run_value_builtin` 0.5%
  - `<php_runtime::vm::Vm>::value_builtin_call` 0.2%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_runtime::builtin::Builtin)>>::len` 0.1%
  - `<core::slice::iter::Iter<php_types::zval::Zval> as core::iter::traits::iterator::Iterator>::any::<<php_runtime::vm::V...` 0.1%
- **builtin_body** 1.1%
  - `php_types::ops::stable_sort_by::<php_types::zval::Zval, php_builtins::array::sort::{closure#2}>` 0.5%
  - `<php_runtime::vm::Vm>::ho_array_filter` 0.2%
  - `php_builtins::math::mt_next` 0.1%
  - `php_builtins::math::mt_rand` 0.1%
  - `php_builtins::var::strlen` 0.1%
- **compile** 0.6%
  - `under php_runtime::lower::lower_source` 0.2%
  - `under php_runtime::compile::compile_program` 0.1%
- **operators** 5.3%
  - `php_types::ops::compare` 2.8%
  - `php_runtime::vm::run::binary_fast` 0.5%
  - `php_runtime::vm::run::concat_n_join` 0.5%
  - `php_types::convert::to_bool` 0.4%
  - `php_types::convert::to_long_cast` 0.2%
  - `<php_runtime::vm::Vm>::incdec_slot_discard_slow` 0.2%
  - `php_types::ops::identical` 0.2%
  - `php_types::convert::to_zstr` 0.1%
- **vm_body** 11.8%
  - `php_runtime::vm::path_apply` 3.1%
  - `<php_runtime::vm::Vm>::vm_merge_sort_with` 1.0%
  - `php_runtime::vm::path_walk` 0.9%
  - `<php_runtime::vm::Vm>::gc_classify` 0.8%
  - `<php_runtime::vm::Vm>::path_op` 0.6%
  - `php_runtime::vm::apply_last` 0.4%
  - `<php_runtime::vm::Vm>::concat_n` 0.4%
  - `php_runtime::vm::arrays::read_slot` 0.4%
- **gc** 1.5%
  - `<php_runtime::vm::Vm>::gc_note_frame` 0.3%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.3%
  - `<php_types::zval::Zval>::is_gc_container` 0.2%
  - `<php_runtime::vm::Vm>::sweep_idle` 0.2%
  - `<php_runtime::vm::Vm>::gc_note_frame::{closure#0}` 0.1%
  - `<php_runtime::vm::Vm>::sweep_skip_next` 0.1%
  - `<php_runtime::vm::Vm>::gc_sweep_body` 0.1%
  - `<php_runtime::vm::Vm>::gc_root_arr` 0.1%
- **symtab** 1.4%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_types::zval::Zval, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.3%
  - `<hashbrown::control::group::neon::Group>::match_tag` 0.2%
  - `rustc_hash::hash_bytes` 0.2%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_types::zval::Zval)>>::len` 0.2%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_types::zval::Zval)>>::get::<hashbrown::map::equivalent_key<[u8], ...` 0.1%
  - `<hashbrown::raw::RawTableInner>::is_empty_singleton` 0.1%
  - `<hashbrown::control::tag::Tag>::full` 0.1%
- **kernel** 0.1%
- **startup_io** 0.1%
  - `[ld-linux-aarch64.so.1]` 0.1%

Top leaf frames (self time, % of all samples):

- `<php_runtime::vm::Vm>::run_loop` 9.1%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 6.9%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 6.9%
- `[libc.so.6]` 3.3%
- `<php_types::array::PhpArray>::insert` 3.1%
- `php_types::ops::compare` 2.8%
- `php_runtime::vm::path_apply` 2.8%
- `<php_types::array::KeyIndex>::lookup` 2.2%
- `<php_types::array::PhpArray>::append` 2.0%
- `<alloc::vec::Vec<(php_types::array::Key, php_types::zval::Zval)>>::set_len` 2.0%
- `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 1.9%
- `<alloc::vec::Vec<(php_types::array::Key, php_types::zval::Zval)>>::extend_desugared::<core::iter::adapters::map::Map<...` 1.9%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 1.8%
- `<alloc::raw_vec::RawVecInner>::capacity` 1.5%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 1.3%
- `__pi_clear_page` 1.2%
- `core::mem::replace::<usize>` 1.1%
- `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 1.0%
- `<php_runtime::vm::Vm>::vm_merge_sort_with` 1.0%
- `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 0.9%
- `<php_types::zval::Zval>::deref_clone` 0.9%
- `core::ptr::write::<php_runtime::vm::Frame>` 0.8%
- `<core::option::Option<php_types::zval::Zval>>::as_ref` 0.8%
- `<alloc::vec::Vec<php_types::zval::Zval>>::pop` 0.7%
- `<php_runtime::vm::Vm>::drive_to_return` 0.7%
