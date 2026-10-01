"""Bucket the leaf (self) samples of a folded perf script by subsystem.
  perf script -i X | rustfilt | python3 buckets.py"""
import collections
import re
import sys

RULES = [
    ("dispatch (run_loop self)", r"Vm>::run_loop$"),
    ("alloc/free", r"^(mi_|_mi_|__libc_malloc|malloc|free|realloc|<alloc::raw_vec|alloc::alloc|__rust_alloc|__rust_dealloc|<alloc::alloc)"),
    ("refcount: Zval clone/drop", r"(drop_glue::<php_types::zval::Zval>|Zval as core::clone::Clone|Rc<core::cell::RefCell<php_types::zval::Zval>>>::drop_slow|drop_glue::<alloc::rc::Rc)"),
    ("arrays", r"(php_types::array|PhpArray|KeyIndex|Repr>|arrays::)"),
    ("objects/props", r"(resolve_prop|slot_of|PropInfo|field_|Props|prop_|magic_applies|deref_object|object::)"),
    ("methods/classes", r"(resolve_method|dispatch_instance|dispatch_static|method_call|methodcall|instance_of|class_id|resolve_dynamic_class|resolve_class)"),
    ("call frames", r"(Frame|recycle_frame|enter_callee|bind_params|decay_arg|pooled_frame|coerce_param|CheckArity|check_arity|call_ns_fallback|value_builtin_call|run_value_builtin|call_user|call_callable)"),
    ("gc emulation", r"(gc_|collect_cycles|BTreeMap<u32)"),
    ("unit link/include", r"(run_linked|unit_|lower|compile|apply_seed|seed_|fp_mix|run_include|resolve_include|run_deferred|defer)"),
    ("hashing/maps", r"(hashbrown|HashMap|HashSet|hash::|Hasher|FxHash|ci_hash)"),
    ("strings/bytes", r"(memcmp|memcpy|memmove|memset|bcmp|strlen|PhpStr|ZStr|zstr|from_utf8|str::|string::|concat|to_zstr|convert::)"),
    ("unserialize", r"(unserialize|unser|ud_|vm_ser)"),
    ("regex", r"(regex|preg|fancy_regex|aho_corasick|memchr)"),
    ("sqlite/pdo", r"(sqlite|pdo|yy_)"),
    ("fibers", r"(corosensei|fiber)"),
    ("kernel", r"^\[k\]|^(el0|do_|__arm64|__pi_|path_|filemap|btrfs|vfs_|_raw_spin|copy_|clear_page|__d_lookup|security_|generic_|kmem_|unmap|tlb_|handle_mm|xas_|blk_|virtio)"),
]


def bucket(f):
    for name, rx in RULES:
        if re.search(rx, f):
            return name
    return "other"


c = collections.Counter()
other = collections.Counter()
n = 0
first = None
for line in sys.stdin:
    if not line.strip():
        if first:
            b = bucket(first)
            c[b] += 1
            if b == "other":
                other[first] += 1
            n += 1
        first = None
        continue
    if line[0] not in " \t" or first:
        continue
    m = re.match(r"\s*[0-9a-f]+\s+(.*?)(\+0x[0-9a-f]+)?\s+\(", line)
    if m:
        first = re.sub(r"::h[0-9a-f]{16}", "", m.group(1))
print(f"{n} samples")
for k, v in c.most_common():
    print(f"{100 * v / n:6.2f}%  {k}")
print("-- top of 'other' --")
for k, v in other.most_common(15):
    print(f"{100 * v / n:6.2f}%  {k[:110]}")
