#!/usr/bin/env python3
"""Attribute a folded-stack profile to the PROFILE.md buckets (PLAN.md §2.2).

usage: buckets.py <name.folded> [--top N] [--md] [--tsv]

Input: folded stacks — `perf script --inline | inferno-collapse-perf | rustfilt`
— one "comm;frame;...;leaf weight" per line, inlined frames included, Rust v0
symbols demangled.

Method
------
Each sample goes to exactly ONE bucket. First, if the stack passes through the
lowering/compiling/unit-linking code, the sample is "compile" (inclusive — see
COMPILE_PHASE). Otherwise walk the stack from the leaf towards the root and
take the first frame that matches a rule; rules are tried in the order below. Frames that match nothing (generic helpers such as
`core::ptr::read`, `Option::expect`, `Result::branch`, `RawVecInner::capacity`,
unresolved `[libc.so.6]`) are skipped, so their time is charged to the nearest
enclosing frame that does mean something.

Inlined frames count. With fat LTO almost every refcount operation, RefCell
check and Vec access is inlined into `run_loop`; the DWARF inline chain is the
only thing that tells "dispatch" from "an Rc increment inside a handler".

Limits
------
1. Line-table attribution in optimised code is approximate: an instruction can
   be charged to a neighbouring inlined callee. Treat a bucket as ±2 points.
2. A sample is charged where the PC is. The cost of a cache miss caused by one
   bucket (say, an allocation that scatters data) lands wherever the miss is
   taken.
3. Timer sampling (cpu-clock) inside a VM, no hardware counters.
4. The plan's bucket list has no place for handler bodies, which is most of an
   interpreter. They are reported as three NAMED "other" buckets rather than
   folded into "dispatch": operators/type juggling, VM handler bodies, GC
   bookkeeping, and the engine's own symbol-table hash maps. "Dispatch" is strictly the loop, the operand-stack and frame
   traffic, and call/return.
"""
import re, sys
from collections import Counter, defaultdict

PAGE_FAULT = re.compile(r"(do_page_fault|handle_mm_fault|do_anonymous_page|clear_page|do_mem_abort|el0_da|"
                        r"__handle_mm_fault|do_translation_fault|alloc_pages|folio_|__arm64_sys_(mmap|munmap|madvise|brk|mremap)|"
                        r"vm_mmap|do_munmap|unmap_|zap_|tlb_|free_pages|release_pages|madvise)")

# A GENERIC frame is a std/core helper whose name only mentions our types as
# type parameters (`Result<Zval, PhpError>::branch`, `ptr::read::<Frame>`,
# `RawVecInner::capacity`). For those only the rules marked generic_ok=True are
# tried; otherwise the frame is skipped and its parent decides.
GENERIC = re.compile(r"^(<core::(result::Result|option::Option)<|core::(ptr|mem|intrinsics|ops|cmp|convert|iter|slice|num|hint|array|clone)::|"
                     r"<core::|<usize as|<isize as|<[iu](8|16|32|64) as|<f64 as|<bool as|<&|<\(|<\[|"
                     r"<alloc::raw_vec::RawVec(Inner)?(<.*>)?>::(capacity|ptr|non_null|inner_ptr)|<alloc::vec::Vec<|<alloc::boxed::Box<)")

# (bucket, regex, generic_ok)
RULES = [
    ("refcell", r"core::cell::(RefCell|BorrowRef|BorrowRefMut|Ref\b|RefMut\b|panic_already_)", True),
    ("alloc", r"^_?_?mi_|__rust_(alloc|dealloc|realloc|alloc_zeroed)|__rustc::__rust_|__rdl_|^(malloc|free|calloc|realloc|cfree|posix_memalign)$|"
              r"<alloc::raw_vec::RawVec(Inner)?(<.*>)?>::(grow|finish_grow|reserve|try_reserve|do_reserve|allocate|try_allocate|with_capacity|deallocate|shrink|current_memory)|"
              r"alloc::raw_vec::(finish_grow|handle_error|capacity_overflow)|alloc::alloc::|<alloc::alloc::Global|<mimalloc::MiMalloc|"
              r"<alloc::vec::Vec<.*>>::(reserve|with_capacity|shrink_to_fit|extend_from_slice|extend_with|extend_desugared|from_iter|resize)\b|"
              r"<alloc::vec::Vec<.*> as (core::iter::traits::collect::FromIterator|alloc::vec::spec_|core::clone::Clone)|"
              r"<alloc::boxed::Box<.*>>::new|alloc::vec::from_elem|alloc_block|block_layout|memset", True),
    ("gc", r"gc_note|gc_buf|gc_root|gc_sweep|gc_collect|gc_refresh|gc_idle|sweep_idle|sweep_skip|<php_runtime::vm::Vm>::sweep|is_gc_container|"
           r"GcMark|WalkMark|break_request_cycles|run_shutdown_destructors|php_runtime::vm::gc|may_hold_containers", False),
    ("rc", r"core::ptr::drop_(glue|in_place)::<(php_types|alloc::rc|core::option::Option<(php_types|alloc::rc)|"
           r"alloc::vec::Vec<(php_types|\(php_types|core::option::Option<\(?php_types|\(u32, php_types)|\[php_types|\[core::option::Option<\(?php_types|"
           r"\(php_types|core::cell::RefCell<php_types|alloc::boxed::Box<php_types|\[\(php_types)|"
           r"<alloc::rc::|alloc::rc::|<php_types::zval::Zval as core::clone::Clone>|<core::option::Option<php_types::zval::Zval> as core::clone::Clone>|"
           r"<php_types::zstr::ZStr as core::(clone::Clone|ops::drop::Drop)>|zstr_drop_slow|rd1_drop_val|"
           r"<php_types::(array::PhpArray|object::Object|object::Props|zval::Closure) as core::ops::drop::Drop>|"
           r"drop_bounded|deref_clone|free_object_id|take_freed_object_id|is_trivial_drop", True),
    ("builtin_args", r"value_builtin_call|run_value_builtin|pop_keys|compute_stringify|compute_debug_info|is_user_stream_op|user_wrapper_|user_stream_|"
                     r"is_lazy_value|as_countable|php_runtime::builtin::|undefined_builtin", True),
    ("hash", r"php_types::array::|snapshot_entries|key_to_zval", False),
    ("hash", r"^<alloc::vec::Vec<(core::option::Option<)?\(php_types::array::Key", True),
    ("symtab", r"hashbrown::|rustc_hash::|core::hash::|as core::hash::Hash>|std::collections::hash|ci_hash|find_fn_ci|class_index", True),
    ("compile", r"mago_|bumpalo::|php_runtime::lower::|php_runtime::compile::|php_runtime::hir::|php_runtime::lsp_check|unit_cache|compile_unit|"
                r"drive_unit|link_fatal|<php_runtime::bytecode::(Module|Func|CompiledClass)>::|relocate_|php_runtime::run_|php_runtime::vm_new|"
                r"main_unit_acquire|accumulate_seed|uc_log|unit_fp|"
                r"<php_runtime::vm::Vm>::(new|link_|load_|seed_|include_|require_|park_module|module_id|publish|register_|declare_|install_)|"
                r"core::ptr::drop_(glue|in_place)::<(php_runtime::(hir|bytecode|lower|compile)|mago_|alloc::vec::Vec<php_runtime::(hir|bytecode))", True),
    ("builtin_body", r"php_builtins::|php_runtime::(preg|json|unserialize|serialize|scanf)::|regex(_automata|_syntax)?::|fancy_regex::|onig|aho_corasick::|"
                     r"^memchr::|<memchr::|php_runtime::vm::(host|host_reflect|dom|pdo|mysqli|session|tokenizer|xmlparser|gd|tidy|xslt|ini|websapi)::|"
                     r"<php_runtime::vm::Vm>::(ho_|host_)|md5::|sha1::|sha2::|digest::|pcre2", False),
    ("string", r"(^|[^A-Za-z0-9_])_*(mem(cpy|move|cmp|chr|rchr)|bcmp|strlen|strnlen)([^A-Za-z0-9]|$)|core::str::|core::slice::memchr|core::fmt::|"
               r"^core::ptr::copy(_nonoverlapping)?::<u8>|^core::intrinsics::copy(_nonoverlapping)?::<u8>|"
               r"alloc::fmt::|compiler_builtins::mem|core::slice::cmp|from_utf8|<\[u8\] as", True),
    ("string", r"php_types::zstr::|php_types::dtoa::|alloc::string::", False),
    ("operators", r"php_types::ops::|php_types::convert::|php_types::numstr::|binary_value_ab|binary_fast|apply_binop|try_number_binop|long_cmp|"
                  r"compare_values|loose_eq|strict_eq|incdec|unary_value|php_runtime::vm::run::(binary|cmp|arith|concat|is_)|overload_receiver", False),
    ("dispatch", r"^<alloc::vec::Vec<php_runtime::(bytecode::Op|vm::Frame|bytecode::Const|vm::IterState)>|"
                 r"^<alloc::vec::Vec<php_types::zval::Zval>>::|^<alloc::vec::Vec<php_types::zval::Zval> as core::ops::|"
                 r"^<alloc::raw_vec::RawVec<php_types::zval::Zval>>::(capacity|ptr|non_null|inner_ptr)|"
                 r"SliceIndex<\[(php_types::zval::Zval|php_runtime::bytecode::Op|php_runtime::vm::Frame)\]>|"
                 r"^core::(ptr::(read|write)|mem::(replace|take|swap))::<(php_types::zval::Zval|php_runtime::vm::Frame)>|"
                 r"^core::ptr::drop_(glue|in_place)::<(php_runtime::vm::Frame|core::option::Option<alloc::boxed::Box<php_runtime::vm::FrameExt)", True),
    ("dispatch", r"<php_runtime::vm::Vm>::run_loop|<php_runtime::vm::Vm>::(run|drive_to_return|enter_callee|pooled_frame|recycle_frame|reg_store_slot|"
                 r"reg_load|reg_operand|cur_line|flush_diags)\b|<php_runtime::vm::Frame>|<php_runtime::vm::FramePool>|bind_params|decay_arg|"
                 r"<php_runtime::bytecode::Const>::to_zval|php_runtime::vm::(store_slot|load_slot)\b", False),
    ("vm_body", r"php_runtime::vm::|<php_runtime::vm::|php_runtime::bytecode::|<php_runtime::bytecode::|php_types::object::|<php_types::object::|"
                r"php_types::zval::|<php_types::zval::|php_types::generator|php_types::stream|php_types::diag", False),
    ("startup_io", r"^_start$|__libc_start|std::rt::|^main$|php_cli::|<php_cli::|std::(fs|io|sys|env|process|thread|path|os|time|backtrace)::|<std::|"
                   r"log4rs|php_runtime::logging|_dl_|ld-linux|libgd|libxml|libz\.so|php_types::(tz|sapi|zlibio|netio|fsown)", False),
]
RULES = [(n, re.compile(p), g) for n, p, g in RULES]

LABELS = {
    "rc": "`Rc` increment/decrement and drop glue (incl. `ZStr` refcount)",
    "refcell": "`RefCell` borrow / borrow_mut checks",
    "alloc": "allocation / deallocation (mimalloc, `Vec` growth, `Box`, page faults, zeroing)",
    "hash": "hash table ops in `PhpArray` (insert / lookup / iterate / clone / foreach snapshot)",
    "string": "string hashing / comparison / copying / formatting",
    "dispatch": "VM dispatch loop itself (the `match`, operand stack, frame push/pop, call/return)",
    "builtin_args": "argument passing into builtins (lookup by name, pre-call checks)",
    "builtin_body": "the builtin bodies themselves",
    "compile": "parser / HIR / compile (script + prelude + include units), per run",
    "operators": "other: operator & type-juggling bodies (`php_types::ops`/`convert`, binary-op helpers)",
    "vm_body": "other: VM handler bodies (variables, array/property paths, OOP, calls, exceptions)",
    "gc": "other: GC bookkeeping and the per-statement destructor sweep",
    "symtab": "other: engine-internal hash maps (hashbrown/Fx: class, function, constant tables)",
    "kernel": "other: kernel (syscalls, I/O, interrupts) excluding page faults",
    "startup_io": "other: process startup, libc, std I/O",
    "unclassified": "other: unclassified (no symbol / broken unwind)",
}
ORDER = ["rc", "refcell", "alloc", "hash", "string", "dispatch", "builtin_args", "builtin_body", "compile",
         "operators", "vm_body", "gc", "symtab", "kernel", "startup_io", "unclassified"]


def clean(frame):
    f = frame
    for suf in ("_[k]", "_[i]", " (inlined)"):
        if f.endswith(suf):
            f = f[: -len(suf)]
    return re.sub(r"::h[0-9a-f]{16}$", "", f)


# The compile bucket is INCLUSIVE: a sample anywhere under the lowering /
# compiling / unit-linking functions is compile cost, whatever its leaf is (the
# hashing, allocation and drops done while compiling are exactly what a bytecode
# cache would remove). Every other bucket is decided by the innermost frame.
COMPILE_PHASE = re.compile(
    r"^(php_runtime::lower::|<php_runtime::lower::|php_runtime::compile::|<php_runtime::compile::|mago_(syntax|database|span)|<mago_|"
    r"<php_runtime::vm::Vm>::(unit_fp|lower_unit|compile_unit|unit_remap|seed_stub|relocate|link_unit|accumulate_seed|unit_cache|publish)|"
    r"php_runtime::vm::(unit_cache_|relocate_)|php_runtime::lsp_check)")


def classify(frames):
    """-> (bucket, frame that decided it)"""
    for fr in frames:
        if COMPILE_PHASE.search(fr):
            return "compile", "under " + clean(fr)
    if frames and frames[-1].endswith("_[k]"):
        for fr in frames:
            if fr.endswith("_[k]") and PAGE_FAULT.search(fr):
                return "alloc", "(kernel) page fault / mm: " + clean(fr)
        return "kernel", "(kernel) " + clean(frames[-1])
    for fr in reversed(frames):
        f = clean(fr)
        if not f or f.startswith("[unknown"):
            continue
        generic = bool(GENERIC.search(f))
        for name, rx, generic_ok in RULES:
            if generic and not generic_ok:
                continue
            if rx.search(f):
                return name, f
    return "unclassified", clean(frames[-1]) if frames else "?"


def load(path):
    buckets, by_frame, leaf_self, total = Counter(), defaultdict(Counter), Counter(), 0
    import gzip
    opener = (lambda: gzip.open(path, "rt", errors="replace")) if path.endswith(".gz") else (lambda: open(path, errors="replace"))
    for line in opener():
        stack, _, cnt = line.rstrip("\n").rpartition(" ")
        try:
            n = int(cnt)
        except ValueError:
            continue
        frames = stack.split(";")
        frames = frames[1:] if len(frames) > 1 else frames      # drop the comm
        b, f = classify(frames)
        buckets[b] += n
        by_frame[b][f] += n
        leaf_self[clean(frames[-1])] += n
        total += n
    return buckets, by_frame, leaf_self, total


def main():
    path = sys.argv[1]
    md, tsv = "--md" in sys.argv, "--tsv" in sys.argv
    top = int(sys.argv[sys.argv.index("--top") + 1]) if "--top" in sys.argv else 12
    buckets, by_frame, leaf_self, total = load(path)
    if not total:
        print("no samples")
        return
    pct = lambda n: 100.0 * n / total
    if tsv:
        for b in ORDER:
            print(f"{b}\t{pct(buckets.get(b, 0)):.2f}")
        return
    short = lambda f, n=120: f if len(f) <= n else f[: n - 3] + "..."
    if md:
        print("| bucket | % of samples |")
        print("|---|---:|")
        for b in ORDER:
            print(f"| {LABELS[b]} | {pct(buckets.get(b, 0)):.1f} |")
        print()
        print("Deciding frames per bucket (% of all samples):")
        print()
        for b in ORDER:
            if pct(buckets.get(b, 0)) < 0.05:
                continue
            print(f"- **{b}** {pct(buckets[b]):.1f}%")
            for f, n in by_frame[b].most_common(8):
                if pct(n) >= 0.05:
                    print(f"  - `{short(f)}` {pct(n):.1f}%")
        print()
        print("Top leaf frames (self time, % of all samples):")
        print()
        for f, n in leaf_self.most_common(25):
            print(f"- `{short(f)}` {pct(n):.1f}%")
    else:
        for b in ORDER:
            print(f"{b:<14}{pct(buckets.get(b, 0)):6.1f}%")
        print()
        for b in ORDER:
            if not buckets.get(b):
                continue
            print(f"[{b}] {pct(buckets[b]):.1f}%")
            for f, n in by_frame[b].most_common(top):
                print(f"   {pct(n):6.2f}%  {short(f, 140)}")


main()
