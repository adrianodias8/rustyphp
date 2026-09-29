| bucket | % of samples |
|---|---:|
| `Rc` increment/decrement and drop glue (incl. `ZStr` refcount) | 3.9 |
| `RefCell` borrow / borrow_mut checks | 1.6 |
| allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing) | 14.3 |
| hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot) | 3.4 |
| string hashing / comparison / copying / formatting | 44.3 |
| VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return) | 5.6 |
| argument passing into builtins (lookup by name, pre-call checks) | 2.6 |
| the builtin bodies themselves | 15.4 |
| parser / HIR / compile (script + prelude + include units), per run | 0.4 |
| other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers) | 2.3 |
| other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions) | 4.3 |
| other: GC bookkeeping and the per-statement destructor sweep | 0.5 |
| other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables) | 1.1 |
| other: kernel (syscalls, I/O, interrupts) excluding page faults | 0.1 |
| other: process startup, libc, std I/O | 0.2 |
| other: unclassified (no symbol / broken unwind) | 0.0 |

Deciding frames per bucket (% of all samples):

- **rc** 3.9%
  - `core::ptr::drop_glue::<php_types::zval::Zval>` 1.1%
  - `<php_types::zval::Zval as core::clone::Clone>::clone` 0.5%
  - `<php_types::zstr::ZStr as core::ops::drop::Drop>::drop` 0.4%
  - `<php_types::zstr::ZStr as core::clone::Clone>::clone` 0.4%
  - `<php_types::array::PhpArray as core::ops::drop::Drop>::drop` 0.3%
  - `php_types::zstr::zstr_drop_slow` 0.2%
  - `core::ptr::drop_glue::<php_types::array::Repr>` 0.1%
  - `core::ptr::drop_glue::<[php_types::zval::Zval]>` 0.1%
- **refcell** 1.6%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Immut, u32, alloc::rc::Rc<core::ce...` 0.7%
  - `<alloc::collections::btree::node::NodeRef<alloc::collections::btree::node::marker::Mut, u32, alloc::rc::Rc<core::cell...` 0.4%
- **alloc** 14.3%
  - `mi_theap_malloc_zero_aligned_at` 1.2%
  - `mi_free_block_local` 1.2%
  - `mi_page_malloc_zero` 1.0%
  - `_mi_unchecked_ptr_page` 1.0%
  - `(kernel) page fault / mm: el0_da` 0.8%
  - `mi_free` 0.8%
  - `<alloc::raw_vec::RawVecInner>::finish_grow` 0.7%
  - `mi_theap_malloc_aligned` 0.6%
- **hash** 3.4%
  - `<php_types::array::PhpArray>::insert` 1.0%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next` 0.5%
  - `<php_types::array::KeyIndex>::lookup` 0.2%
  - `<php_types::array::KeyIndex>::insert_new` 0.2%
  - `<php_types::array::PhpArray>::to_hashed` 0.2%
  - `<php_types::array::Iter as core::iter::traits::iterator::Iterator>::next::{closure#1}` 0.2%
  - `<php_types::array::PhpArray>::append` 0.2%
  - `<php_types::array::Key as core::cmp::PartialEq>::eq` 0.1%
- **string** 44.3%
  - `core::ptr::copy_nonoverlapping::<u8>` 35.8%
  - `memcmp` 2.4%
  - `core::str::validations::run_utf8_validation` 0.7%
  - `core::slice::memchr::memchr_naive` 0.7%
  - `core::fmt::float::float_to_exponential_common_shortest::<f64>` 0.6%
  - `memcpy@plt` 0.6%
  - `core::fmt::float::float_to_decimal_common_exact::<f64>` 0.4%
  - `bcmp@plt` 0.3%
- **dispatch** 5.6%
  - `<php_runtime::vm::Vm>::run_loop` 1.7%
  - `<php_runtime::vm::Vm>::flush_diags` 0.5%
  - `<alloc::vec::Vec<php_runtime::bytecode::Op>>::as_slice` 0.5%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::len` 0.4%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_slice` 0.3%
  - `<alloc::vec::Vec<php_runtime::vm::Frame>>::as_mut_slice` 0.3%
  - `<alloc::raw_vec::RawVec<php_types::zval::Zval>>::capacity` 0.3%
  - `<alloc::vec::Vec<php_types::zval::Zval>>::as_slice` 0.2%
- **builtin_args** 2.6%
  - `<php_runtime::vm::Vm>::run_value_builtin` 0.9%
  - `<php_runtime::vm::Vm>::value_builtin_call` 0.4%
  - `<php_runtime::builtin::Ctx>::to_zstr` 0.3%
  - `<php_runtime::vm::Vm>::compute_stringify` 0.2%
  - `<php_runtime::vm::Vm>::compute_stringify::any_object` 0.2%
  - `<hashbrown::map::HashMap<alloc::vec::Vec<u8>, php_runtime::builtin::Builtin, rustc_hash::FxBuildHasher>>::get::<[u8]>` 0.1%
  - `<hashbrown::raw::RawTable<(alloc::vec::Vec<u8>, php_runtime::builtin::Builtin)>>::len` 0.1%
  - `<core::slice::iter::Iter<php_types::zval::Zval> as core::iter::traits::iterator::Iterator>::any::<<php_runtime::vm::V...` 0.1%
- **builtin_body** 15.4%
  - `<php_runtime::json::Parser>::string` 1.2%
  - `php_builtins::json::encode_string` 1.0%
  - `<php_runtime::json::Parser>::object` 0.5%
  - `md5::compress::soft::op_g` 0.5%
  - `<php_runtime::unserialize::Parser>::value` 0.5%
  - `<php_runtime::json::Parser>::peek` 0.4%
  - `md5::compress::soft::op_i` 0.4%
  - `regex_automata::dfa::search::find_fwd_imp::<&regex_automata::dfa::dense::DFA<alloc::vec::Vec<u32>>>` 0.4%
- **compile** 0.4%
  - `under php_runtime::lower::lower_source` 0.1%
  - `under php_runtime::compile::compile_program` 0.1%
- **operators** 2.3%
  - `php_types::convert::to_zstr` 0.7%
  - `<php_runtime::vm::Vm>::binary_value_ab` 0.6%
  - `php_runtime::vm::apply_binop` 0.2%
  - `php_types::convert::to_long_cast` 0.1%
  - `php_runtime::vm::run::concat_n_join` 0.1%
  - `php_runtime::vm::run::binary_fast` 0.1%
  - `<php_runtime::vm::Vm>::apply_binop_ovl` 0.1%
  - `php_types::ops::try_to_long` 0.1%
- **vm_body** 4.3%
  - `<php_runtime::vm::Vm>::vm_json_to_zval` 0.5%
  - `<php_runtime::vm::Vm>::vm_ser_build` 0.4%
  - `<php_runtime::vm::Vm>::write_output` 0.3%
  - `php_runtime::vm::calls::value_builtin_string_coerces` 0.3%
  - `<php_runtime::vm::Vm>::vm_ser_to_zval_slot` 0.2%
  - `<php_runtime::vm::Vm>::json_normalize` 0.2%
  - `<php_runtime::vm::Vm>::concat_n` 0.2%
  - `php_runtime::vm::oop::deref_object` 0.2%
- **gc** 0.5%
  - `<php_runtime::vm::Vm>::gc_note_slow` 0.2%
  - `<php_types::zval::Zval>::is_gc_container` 0.1%
  - `<php_runtime::vm::Vm>::sweep_idle` 0.1%
  - `<php_types::array::PhpArray>::may_hold_containers` 0.1%
- **symtab** 1.1%
  - `<core::result::Result<std::collections::hash::map::HashMap<u32, php_types::zstr::ZStr>, php_types::diag::PhpError> as...` 0.2%
  - `<hashbrown::control::group::neon::Group>::match_tag` 0.1%
  - `core::mem::replace::<std::collections::hash::map::HashMap<u32, php_types::zval::Zval>>` 0.1%
  - `<hashbrown::raw::RawTableInner>::is_empty_singleton` 0.1%
  - `rustc_hash::hash_bytes` 0.1%
  - `<hashbrown::raw::RawTableInner>::record_item_insert_at` 0.1%
- **kernel** 0.1%
  - `(kernel) [[vdso]]` 0.1%
- **startup_io** 0.2%
  - `<std::sys::thread_local::native::lazy::Storage<core::cell::Cell<(u64, u64)>, !>>::get_or_init::<<std::hash::random::R...` 0.1%

Top leaf frames (self time, % of all samples):

- `[libc.so.6]` 36.5%
- `memcmp` 2.4%
- `<php_runtime::vm::Vm>::run_loop` 1.4%
- `<alloc::raw_vec::RawVecInner>::capacity` 1.3%
- `mi_theap_malloc_zero_aligned_at` 1.2%
- `mi_free_block_local` 1.2%
- `core::ptr::drop_glue::<php_types::zval::Zval>` 1.1%
- `mi_page_malloc_zero` 1.0%
- `_mi_unchecked_ptr_page` 1.0%
- `<u32>::wrapping_add` 0.9%
- `<php_types::array::PhpArray>::insert` 0.9%
- `php_builtins::json::encode_string` 0.8%
- `<php_runtime::json::Parser>::string` 0.8%
- `mi_free` 0.8%
- `__pi_clear_page` 0.7%
- `php_types::convert::to_zstr` 0.7%
- `core::slice::memchr::memchr_naive` 0.7%
- `memcpy@plt` 0.6%
- `<php_runtime::vm::Vm>::run_value_builtin` 0.6%
- `core::str::validations::run_utf8_validation` 0.6%
- `mi_theap_malloc_aligned` 0.6%
- `<php_runtime::vm::Vm>::binary_value_ab` 0.6%
- `core::mem::replace::<usize>` 0.6%
- `<php_types::zval::Zval as core::clone::Clone>::clone` 0.5%
- `<alloc::raw_vec::RawVecInner>::finish_grow` 0.5%
