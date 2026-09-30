| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 5.9 |
| `RefCell` borrow / borrow_mut checks | 2.1 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 20.5 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 4.7 |
| string hashing / comparison / copying / formatting | 16.8 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 8.0 |
| argument passing into builtins (lookup by name, pre-call checks) | 4.5 |
| the builtin bodies themselves | 24.5 |
| parser / HIR / compile (script + prelude + include units), per run | 0.4 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 3.8 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 6.2 |
| other: GC bookkeeping and the per-statement destructor sweep | 0.6 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 1.7 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 0.1 |
| other: process startup, libc, std I/O | 0.2 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 5.9%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 1.6%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 0.9%
  - `<php_types::zstr::ZStr as core::ops::drop::Drop>::drop` 0.9%
  - `<php_types::array::PhpArray as core::ops::drop::Drop>::drop` 0.5%
  - `<php_types::zstr::ZStr as core::clone::Clone>::clone` 0.4%
  - `php_types::zstr::zstr_drop_slow` 0.3%
  - `<php_types::zval::Zval>::deref_clone` 0.2%
  - `php_types::array::rd1_drop_val` 0.2%
- **refcell** 2.1%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Immut, u32, alloc::rc::Rc<core::ce...` 0.9%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Mut, u32, alloc::rc::Rc<core::cell...` 0.4%
  - `<alloc::boxed::Box<alloc::rc::RcInner<core::cell::RefCell<php_types::object::Object>>>>::new` 0.1%
- **alloc** 20.5%
  - `mi_free_block_local` 1.8%
  - `mi_theap_malloc_zero_aligned_at` 1.5%
  - `mi_page_malloc_zero` 1.3%
  - `_mi_unchecked_ptr_page` 1.2%
  - `mi_free` 1.2%
  - `(kernel) page fault / mm: el0_da` 1.1%
  - `<alloc::raw_vec::RawVecInner>::finish_grow` 1.0%
  - `_mi_theap_realloc_zero` 0.9%
- **hash** 4.7%
  - `<php_types::array::PhpArray>::insert` 1.4%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next` 0.7%
  - `<php_types::array::KeyIndex>::lookup` 0.4%
  - `<php_types::array::KeyIndex>::insert_new` 0.3%
  - `<php_types::array::PhpArray>::to_hashed` 0.3%
  - `<php_types::array::PhpArray>::append` 0.3%
  - `<php_types::array::Key as core::cmp::PartialEq>::eq` 0.3%
  - `php_types::array::canonical_int_key` 0.1%
- **string** 16.8%
  - `core::ptr::copy_nonoverlapping::<u8>` 3.9%
  - `memcmp` 3.5%
  - `core::str::validations::run_utf8_validation` 1.2%
  - `core::slice::memchr::memchr_naive` 1.0%
  - `core::fmt::float::float_to_exponential_common_shortest::<f64>` 1.0%
  - `memcpy@plt` 0.9%
  - `bcmp@plt` 0.7%
  - `core::fmt::float::float_to_decimal_common_exact::<f64>` 0.5%
- **dispatch** 8.0%
  - `<php_runtime::vm::Vm>::run_loop` 2.5%
  - `<php_runtime::vm::Vm>::flush_diags` 0.9%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 0.8%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 0.6%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 0.6%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 0.4%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::capacity` 0.4%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 0.4%
- **builtin_args** 4.5%
  - `<php_runtime::vm::Vm>::run_value_builtin` 1.6%
  - `<php_runtime::builtin::Ctx>::to_zstr` 0.7%
  - `<php_runtime::vm::Vm>::value_builtin_call` 0.6%
  - `<php_runtime::vm::Vm>::compute_stringify::any_object` 0.4%
  - `<php_runtime::vm::Vm>::compute_stringify` 0.3%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_runtime::builtin::Builtin, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.3%
  - `<core::slice::iter::Iter<php_types::zval::Zval> as core::iter::traits::iterator::Iterator>::any::<<php_runtime::vm::V...` 0.2%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_runtime::builtin::Builtin)>>::len` 0.1%
- **builtin_body** 24.5%
  - `<php_runtime::json::Parser>::string` 1.9%
  - `php_builtins::json::encode_string` 1.4%
  - `<php_runtime::json::Parser>::peek` 0.7%
  - `<php_runtime::json::Parser>::object` 0.7%
  - `php_builtins::string::strtr_array` 0.7%
  - `<php_runtime::unserialize::Parser>::value` 0.7%
  - `md5::compress::soft::op_h` 0.6%
  - `md5::compress::soft::op_g` 0.6%
- **compile** 0.4%
  - `under php_runtime::lower::lower_source` 0.1%
  - `under php_runtime::compile::compile_program` 0.1%
- **operators** 3.8%
  - `php_types::convert::to_zstr` 1.2%
  - `<php_runtime::vm::Vm>::binary_value_ab` 0.9%
  - `php_types::convert::to_long_cast` 0.4%
  - `php_runtime::vm::run::concat_n_join` 0.3%
  - `php_runtime::vm::apply_binop` 0.2%
  - `<php_runtime::vm::Vm>::apply_binop_ovl` 0.1%
  - `php_runtime::vm::run::binary_fast` 0.1%
  - `php_types::ops::try_to_long` 0.1%
- **vm_body** 6.2%
  - `<php_runtime::vm::Vm>::vm_json_to_zval` 0.6%
  - `<php_runtime::vm::Vm>::write_output` 0.6%
  - `<php_runtime::vm::Vm>::vm_ser_build` 0.4%
  - `<php_runtime::vm::Vm>::vm_ser_to_zval_slot` 0.3%
  - `php_runtime::vm::calls::value_builtin_string_coerces` 0.3%
  - `php_runtime::vm::oop::deref_object` 0.3%
  - `<php_runtime::vm::Vm>::json_normalize` 0.2%
  - `<php_runtime::vm::Vm>::concat_n` 0.2%
- **gc** 0.6%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.3%
  - `<php_types::zval::Zval>::is_gc_container` 0.1%
  - `<php_runtime::vm::Vm>::sweep_idle` 0.1%
- **symtab** 1.7%
  - `<hashbrown::control::group::neon::Group>::match_tag` 0.3%
  - `core::mem::replace::<std::collections::hash::map::HashMap<u32, php_types::zstr::ZStr>>` 0.2%
  - `<core::result::Result<std::collections::hash::map::HashMap<u32, php_types::zstr::ZStr>, php_types::diag::PhpError> as...` 0.2%
  - `<hashbrown::raw::RawTableInner>::is_empty_singleton` 0.2%
  - `rustc_hash::hash_bytes` 0.1%
  - `<hashbrown::raw::RawTableInner>::record_item_insert_at` 0.1%
  - `<hashbrown::raw::RawTableInner>::find_inner` 0.1%
  - `core::ptr::drop_glue::<std::collections::hash::map::HashMap<u32, php_types::zstr::ZStr>>` 0.1%
- **kernel** 0.1%
  - `(kernel) __kernel_clock_gettime` 0.1%
- **startup_io** 0.2%
  - `<std::sys::thread_local::native::lazy::Storage<core::cell::Cell<(u64, u64)>, !>>::get_or_init::<<std::hash::random::R...` 0.1%

Top leaf frames (self time, % of all samples):

- `[libc.so.6]` 4.8%
- `memcmp` 3.5%
- `<php_runtime::vm::Vm>::run_loop` 2.1%
- `<alloc::raw_vec::RawVecInner>::capacity` 2.0%
- `mi_free_block_local` 1.8%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 1.6%
- `mi_theap_malloc_zero_aligned_at` 1.5%
- `<u32>::wrapping_add` 1.5%
- `mi_page_malloc_zero` 1.3%
- `<php_types::array::PhpArray>::insert` 1.2%
- `_mi_unchecked_ptr_page` 1.2%
- `php_types::convert::to_zstr` 1.2%
- `<php_runtime::json::Parser>::string` 1.2%
- `mi_free` 1.2%
- `php_builtins::json::encode_string` 1.2%
- `core::str::validations::run_utf8_validation` 1.1%
- `core::mem::replace::<usize>` 1.1%
- `__pi_clear_page` 1.0%
- `core::slice::memchr::memchr_naive` 1.0%
- `<php_runtime::vm::Vm>::run_value_builtin` 0.9%
- `_mi_theap_realloc_zero` 0.9%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 0.9%
- `<php_runtime::vm::Vm>::binary_value_ab` 0.9%
- `mi_theap_malloc_aligned` 0.9%
- `memcpy@plt` 0.9%
