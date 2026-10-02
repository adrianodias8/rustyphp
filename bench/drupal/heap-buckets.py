"""Attribute heaptrack's leaked-bytes folded stacks (bytes still allocated when
the worker stopped) to engine sites.
  heaptrack_print ... --flamegraph-cost-type leaked -F x.folded
  python3 heap-buckets.py [--sites T] < x.folded
heaptrack folds a frame as `name (file.rs)`; a stack is charged to its
innermost frame outside the standard library (decided by file name), and to
the first RULES bucket any frame matches, scanning from the leaf."""
import collections
import re
import sys

STD_FILES = {
    "boxed.rs", "raw_vec.rs", "alloc.rs", "rc.rs", "sync.rs", "function.rs", "option.rs", "result.rs",
    "map.rs", "set.rs", "spec_from_iter.rs", "spec_from_iter_nested.rs", "spec_extend.rs", "vec.rs",
    "mod.rs?", "slice.rs", "iterator.rs", "clone.rs", "cmp.rs", "into_iter.rs", "string.rs", "str.rs",
    "unwind_safe.rs", "panicking.rs", "panic.rs", "backtrace.rs", "lifecycle.rs", "unix.rs", "local.rs",
    "cell.rs", "once.rs", "once_cell.rs", "borrow.rs", "convert.rs", "fold.rs", "accum.rs", "range.rs",
    "raw.rs", "rustc_entry.rs", "hash.rs", "default.rs", "ptr.rs", "mem.rs", "cloned.rs", "copied.rs",
    "filter_map.rs", "flatten.rs", "chain.rs", "zip.rs", "enumerate.rs", "rev.rs", "take.rs",
    "partial_eq.rs", "extend.rs", "collect.rs", "btree", "node.rs", "navigate.rs", "append.rs",
    "fmt.rs", "builders.rs", "num.rs", "writer.rs", "vec_deque.rs", "linked_list.rs",
}
FRAME = re.compile(r"^(.*) \(([^()]*\.rs)\)$")

# Long-lived owners, most specific first; matched against `name (file)`.
RULES = [
    ("preg cache (compiled regexes)", r"preg_compile|pcre2|regex::|fancy_regex"),
    ("realpath / stat caches", r"realpath|stat_cache"),
    ("include index", r"include_index|resolve_include"),
    ("deferred declarations cache", r"defercache|DeferredDecl|run_deferred|defer_"),
    ("lower: HIR (parse + lower)", r"\(lower|lower_|lower\.rs|\(parse|parser|lexer|hir\.rs|\(hir"),
    ("compile: bytecode", r"compile_|\(compile|compile/|bytecode\.rs|assemble|emit_"),
    ("unit link / class tables", r"run_linked|link_|relocat|declare_class|class_table|apply_seed|seed_"),
    ("interned strings", r"intern"),
    ("unit cache (other)", r"unit_cache|UNIT_CACHE|unit_fp"),
]


def site_of(frames):
    for f in reversed(frames):
        m = FRAME.match(f)
        if not m:
            continue
        name, file = m.group(1), m.group(2)
        if file in STD_FILES or name.startswith("_RN"):
            continue
        return f"{re.sub(r'<[^<>]*>', '<>', name)[-110:]} ({file})"
    return "(none)"


# What kind of unit the memory was built for (lowering context).
CONTEXTS = [
    ("deferred declaration", r"run_deferred|DeclareDeferred|defer"),
    ("eval()", r"\beval|run_eval"),
    ("include", r"run_include"),
    ("main script", r"run_module_with_hir|run_source"),
]


def context_of(frames):
    for f in reversed(frames):
        for c, rx in CONTEXTS:
            if re.search(rx, f):
                return c
    return "other"


def bucket_of(frames):
    for f in reversed(frames):
        for b, rx in RULES:
            if re.search(rx, f):
                return b
    return "other"


args = sys.argv[1:]
top = int(args[args.index("--sites") + 1]) if "--sites" in args else 40
total = 0
sites = collections.Counter()
buckets = collections.Counter()
by_ctx = collections.Counter()
for line in sys.stdin:
    stack, _, n = line.rstrip().rpartition(" ")
    if not n.isdigit():
        continue
    n = int(n)
    total += n
    frames = stack.split(";")
    sites[site_of(frames)] += n
    b = bucket_of(frames)
    buckets[b] += n
    if b.startswith(("lower", "compile")):
        by_ctx[(b.split(":")[0], context_of(frames))] += n

mib = lambda v: v / 2**20
print(f"total live at stop: {mib(total):.1f} MiB")
print("\n## by owner")
for k, v in buckets.most_common():
    print(f"{mib(v):8.2f} MiB {100 * v / total:5.1f}%  {k}")
print("\n## HIR / bytecode by the unit they were built for")
for (b, c), v in sorted(by_ctx.items(), key=lambda kv: -kv[1]):
    print(f"{mib(v):8.2f} MiB {100 * v / total:5.1f}%  {b:8} {c}")
print(f"\n## top {top} allocation sites (innermost non-std frame)")
for k, v in sites.most_common(top):
    print(f"{mib(v):8.2f} MiB {100 * v / total:5.1f}%  {k}")
