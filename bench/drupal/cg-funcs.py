#!/usr/bin/env python3
"""Per-request cachegrind views from two runs of bench/drupal/cachegrind.sh.

  cg-funcs.py BASE.out RUN.out M [--by fn|file|fnfile|handler] [--vm H] [--ops R] [--top N]

Every count is (RUN - BASE) / M, i.e. per measured request.
  fn       per function (cachegrind's fn=, i.e. the symbol the code was inlined into)
  file     per source file of the line (inlined code is charged to its own file)
  fnfile   per (function, file) pair
  handler  like fnfile, but lines of zend_vm_execute.h (--vm, the php-src copy)
           are charged to the opcode handler whose body holds them: with the
           hybrid VM the handlers are labels inside execute_ex, not functions.
           For ferro, --ops (vm/run.rs) charges run.rs lines to their `Op::X` arm
           (lines before the first arm keep the function name).
"""
import argparse, bisect, collections, re


def load(path):
    ev, cur_fl, cur_fn = None, "?", "?"
    rows = collections.defaultdict(lambda: None)
    for line in open(path, errors="replace"):
        if line[0].isdigit():
            f = line.split()
            key = (cur_fl, cur_fn, int(f[0]))
            v = [int(x) for x in f[1:]] + [0] * (len(ev) - len(f) + 1)
            old = rows[key]
            rows[key] = v if old is None else [a + b for a, b in zip(old, v)]
        elif line.startswith("fl="):
            cur_fl = line[3:].strip()
        elif line.startswith("fn="):
            cur_fn = line[3:].strip()
        elif line.startswith("events:"):
            ev = line.split()[1:]
    return ev, rows


def vm_map(path):
    starts, names = [], []
    # every function definition (handlers and their *_helper_SPEC helpers) and
    # every HYBRID_CASE label of execute_ex and its inline
    # zend_leave_helper_SPEC_LABEL (the RETURN tail) starts a new region
    fdef = re.compile(r"^(?:static|ZEND_API)\b.*?\b([A-Za-z_]\w*)\((?!.*;\s*$)")
    case = re.compile(r"HYBRID_CASE\((\w+)\):|^(\w+)_LABEL:")
    for i, line in enumerate(open(path, errors="replace"), 1):
        m = fdef.search(line) or case.search(line)
        if m:
            starts.append(i)
            names.append(re.sub(r"_HANDLER$", "", next(x for x in m.groups() if x)))
    return starts, names


def ops_map(path):
    """ferro: the arms of run_loop's `match op` in vm/run.rs, one region per `Op::X`,
    and one per `fn` (helpers inlined into run_loop keep their own name)."""
    starts, names = [], []
    arm = re.compile(r"^\s+(?:\|\s*)?Op::(\w+)")
    fdef = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?fn (\w+)")
    for i, line in enumerate(open(path, errors="replace"), 1):
        m, f = arm.search(line), fdef.search(line)
        if m or f:
            starts.append(i)
            names.append("Op::" + m.group(1) if m else f"fn {f.group(1)} [run.rs]")
    return starts, names


def short(fn):
    fn = re.sub(r"::h[0-9a-f]{16}$", "", fn)
    fn = re.sub(r"<[^<>]*>", "", fn) if len(fn) > 110 else fn
    return fn[:110]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("base"); ap.add_argument("run"); ap.add_argument("m", type=int)
    ap.add_argument("--by", default="fn"); ap.add_argument("--vm"); ap.add_argument("--ops"); ap.add_argument("--top", type=int, default=20)
    ap.add_argument("--sort", default="Ir")
    ap.add_argument("--cols", default="Ir,Dr,Dw,D1mr,D1mw,DLmr,DLmw,I1mr,Bc,Bcm")
    a = ap.parse_args()
    ev, base = load(a.base)
    _, run = load(a.run)
    vm = vm_map(a.vm) if a.vm else None
    ops = ops_map(a.ops) if a.ops else None
    agg = collections.defaultdict(lambda: [0] * len(ev))
    for key in set(base) | set(run):
        fl, fn, ln = key
        r, b = run.get(key) or [0] * len(ev), base.get(key) or [0] * len(ev)
        d = [x - y for x, y in zip(r, b)]
        base_fl = fl.rsplit("/", 1)[-1]
        if a.by == "fn":
            k = short(fn)
        elif a.by == "file":
            k = base_fl
        elif a.by == "fnfile":
            k = f"{short(fn)}  [{base_fl}]"
        else:
            if vm and base_fl == "zend_vm_execute.h":
                i = bisect.bisect_right(vm[0], ln) - 1
                k = vm[1][i] if i >= 0 else "zend_vm_execute.h:?"
            elif ops and fl.endswith("vm/run.rs"):
                i = bisect.bisect_right(ops[0], ln) - 1
                k = ops[1][i] if i >= 0 else f"{short(fn)}  [run.rs]"
            else:
                k = f"{short(fn)}  [{base_fl}]"
        acc = agg[k]
        for i, x in enumerate(d):
            acc[i] += x
    tot = [sum(v[i] for v in agg.values()) for i in range(len(ev))]
    si = ev.index(a.sort)
    cols = [c for c in a.cols.split(",") if c in ev]
    print("| # | " + a.by + " | " + " | ".join(cols) + " | % Ir |")
    print("|---|---|" + "---:|" * (len(cols) + 1))
    print("| | **total** | " + " | ".join(f"{tot[ev.index(c)] / a.m:,.0f}" for c in cols) + " | 100 |")
    for n, (k, v) in enumerate(sorted(agg.items(), key=lambda kv: -kv[1][si])[: a.top], 1):
        print(f"| {n} | `{k}` | " + " | ".join(f"{v[ev.index(c)] / a.m:,.0f}" for c in cols)
              + f" | {100 * v[ev.index('Ir')] / tot[ev.index('Ir')]:.1f} |")


main()
