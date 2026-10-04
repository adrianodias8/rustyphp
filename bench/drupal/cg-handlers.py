#!/usr/bin/env python3
"""Instructions per interpreter handler, from two callgrind runs of
bench/drupal/cachegrind.sh (TOOL=callgrind, --dump-instr=yes).

  cg-handlers.py BASE.out RUN.out M BINARY --engine ferro|php --src FILE [--top N] [--tsv OUT]

--src is vm/run.rs (ferro) or Zend/zend_vm_execute.h (php). Every count is
(RUN - BASE) / M, per measured request.

Every instruction address inside the interpreter function (run_loop /
execute_ex, at any recursion level) is resolved through its DWARF inline chain
(gimli addr2line -i) to the outermost frame in --src, and that line to the
handler region holding it (cg-funcs.py's maps: `Op::X` arms of run_loop; the
handler functions, helpers and HYBRID_CASE labels of zend_vm_execute.h).
  Ir     instructions executed in the interpreter function itself, inlined code
         included (php: plus the ZEND_*_HANDLER functions compiled out of line)
  execs  the most-executed instruction of the region (~ how often it ran)
Exclusive costs only: callgrind's inclusive costs are not usable here (the
hybrid VM's jumps between handlers and run_loop's recursion produce call edges
larger than the whole run), so out-of-line callees stay under their own name
(the F rows of --tsv, grouped by cg-buckets.py).
"""
import argparse, bisect, collections, os, re, subprocess

here = os.path.dirname(os.path.abspath(__file__))
src = open(os.path.join(here, "cg-funcs.py")).read().replace("\nmain()\n", "\n")
cgf = {}
exec(compile(src, "cg-funcs.py", "exec"), cgf)

A2L = "/target/tools/bin/addr2line"


def parse(path):
    """-> self_cost{(ob, fn, addr): Ir}, edges{(ob, fn, addr, cob, cfn): incl}, targets[(cfn, addr)]
    (edges are kept for inspection only; targets calibrate the PIE load base)"""
    names = {"ob": {}, "fl": {}, "fn": {}}
    kind = {"ob": "ob", "cob": "ob", "fl": "fl", "fi": "fl", "fe": "fl", "cfi": "fl", "cfl": "fl",
            "fn": "fn", "cfn": "fn"}
    selfc = collections.Counter()
    edges = collections.Counter()
    targets = []
    ob = fn = cob = cfn = None
    addr = line = 0
    pending_call = False
    pat = re.compile(r"^\((\d+)\)(?: (.*))?$")

    def name(k, v):
        m = pat.match(v)
        if not m:
            return v
        t = names[kind[k]]
        if m.group(2) is not None:
            t[m.group(1)] = m.group(2)
        return t.get(m.group(1), "?")

    def sub(tok, cur, hexa):
        if tok == "*":
            return cur
        if tok[0] in "+-":
            return cur + int(tok)
        return int(tok, 16) if tok.startswith("0x") else int(tok)

    for raw in open(path, errors="replace"):
        c = raw[0]
        if c.isdigit() or c in "+-*":
            f = raw.split()
            addr = sub(f[0], addr, True)
            line = sub(f[1], line, False)
            cost = int(f[2]) if len(f) > 2 else 0
            if pending_call:
                edges[(ob, fn, addr, cob or ob, cfn)] += cost
                pending_call = False
                cob = None  # cob= applies to the one call that follows it
            else:
                selfc[(ob, fn, addr)] += cost
            continue
        k, _, v = raw.rstrip("\n").partition("=")
        if k in ("ob",):
            ob = name(k, v); cob = None
        elif k == "cob":
            cob = name(k, v)
        elif k == "fn":
            fn = name(k, v); cob = None
        elif k == "cfn":
            cfn = name(k, v)
        elif k in ("fl", "fi", "fe", "cfi", "cfl"):
            name(k, v)
        elif k == "calls":
            pending_call = True
            t = v.split()
            if len(t) > 1 and t[1].startswith("0x"):
                targets.append((cfn, int(t[1], 16)))
    return selfc, edges, targets


def base_fn(fn):
    return re.sub(r"'\d+$", "", fn)


def load_base(binary, targets):
    syms = {}
    out = subprocess.run(["nm", "--defined-only", binary], capture_output=True, text=True).stdout
    for l in out.splitlines():
        p = l.split()
        if len(p) == 3 and p[1] in "tT":
            syms.setdefault(p[2], int(p[0], 16))
    c = collections.Counter(a - syms[base_fn(f)] for f, a in targets if base_fn(f) in syms)
    return c.most_common(1)[0][0]


def resolve(binary, addrs, base, srcfile):
    """addr -> source line of the outermost inline frame lying in srcfile (or None)"""
    if not addrs:
        return {}
    inp = "".join(f"0x{a - base:x}\n" for a in addrs)
    out = subprocess.run([A2L, "-e", binary, "-i", "-a"], input=inp, capture_output=True, text=True).stdout
    res, cur, chain = {}, None, []
    srcbase = srcfile.split("/")[-2:]
    def flush():
        if cur is not None:
            hit = None
            for fr in chain:  # innermost first; keep the last (outermost) match
                fpath, _, ln = fr.rpartition(":")
                if fpath.split("/")[-2:] == srcbase and ln.isdigit():
                    hit = int(ln)
            res[cur + base] = hit
    for l in out.splitlines():
        l = l.strip()
        if l.startswith("0x"):
            flush()
            cur, chain = int(l, 16), []
        elif ":" in l:
            chain.append(l.split(" (discriminator")[0])
    flush()
    return res


def per_handler(path, binary, engine, srcfile, regions, base=None, cache=None):
    """-> {("H", region) | ("F", function): [Ir, execs]}, total, base, line cache"""
    selfc, edges, targets = parse(path)
    obj = os.path.realpath(binary)
    interp = "run_loop" if engine == "ferro" else "execute_ex"
    is_vm = lambda ob, fn: ob is not None and os.path.realpath(ob) == obj and base_fn(fn).endswith(interp)
    if base is None:
        base = load_base(binary, targets)
    addrs = sorted({a for (ob, fn, a) in selfc if is_vm(ob, fn)} - set(cache or {}))
    lines = dict(cache or {})
    lines.update(resolve(binary, addrs, base, srcfile))
    def region(a):
        ln = lines.get(a)
        if ln is None:
            return f"{interp} (outside {srcfile.split('/')[-1]})"
        i = bisect.bisect_right(regions[0], ln) - 1
        return regions[1][i] if i >= 0 else f"{interp} (prologue)"
    h = collections.defaultdict(lambda: [0, 0])
    for (ob, fn, a), c in selfc.items():
        if is_vm(ob, fn):
            r = h[("H", region(a))]
            r[0] += c
            r[1] = max(r[1], c)
        else:
            k = cgf["short"](base_fn(fn))
            if engine == "php" and k.endswith("_HANDLER"):
                r = h[("H", k[: -len("_HANDLER")])]  # a handler compiled as its own function
                r[0] += c
                r[1] = max(r[1], c)
            else:
                h[("F", k)][0] += c
    return h, sum(selfc.values()), base, lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("base"); ap.add_argument("run"); ap.add_argument("m", type=int); ap.add_argument("binary")
    ap.add_argument("--engine", required=True); ap.add_argument("--src", required=True)
    ap.add_argument("--top", type=int, default=40); ap.add_argument("--tsv")
    a = ap.parse_args()
    regions = cgf["ops_map"](a.src) if a.engine == "ferro" else cgf["vm_map"](a.src)
    h1, t1, base, cache = per_handler(a.run, a.binary, a.engine, a.src, regions)
    h0, t0, _, _ = per_handler(a.base, a.binary, a.engine, a.src, regions, base, cache)
    m, tot = a.m, (t1 - t0) / a.m
    rows = []
    for k in set(h1) | set(h0):
        r1, r0 = h1.get(k, [0, 0]), h0.get(k, [0, 0])
        # execs: the hottest instruction's count is cumulative, so it differences too
        rows.append((k, (r1[0] - r0[0]) / m, (r1[1] - r0[1]) / m))
    rows.sort(key=lambda r: -r[1])
    hs = [r for r in rows if r[0][0] == "H"]
    print(f"per request: {tot:,.0f} Ir; interpreter handlers (self, inlined code included) "
          f"{sum(r[1] for r in hs):,.0f} = {100 * sum(r[1] for r in hs) / tot:.1f} %")
    print("| # | handler | Ir (self) | % of request | ≈ execs | Ir / exec |")
    print("|---|---|---:|---:|---:|---:|")
    for n, ((_, k), s, ex) in enumerate(hs[: a.top], 1):
        print(f"| {n} | `{k}` | {s:,.0f} | {100 * s / tot:.1f} | {ex:,.0f} | {s / ex if ex > 0 else 0:,.1f} |")
    if a.tsv:
        with open(a.tsv, "w") as f:
            f.write(f"T\ttotal\t{tot:.0f}\t0\n")
            for (t, k), s, ex in rows:
                f.write(f"{t}\t{k}\t{s:.0f}\t{ex:.0f}\n")


main()
