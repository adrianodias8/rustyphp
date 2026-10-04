#!/usr/bin/env python3
"""Callers of a function, by call count, from two callgrind runs of cachegrind.sh
(TOOL=callgrind, --dump-instr=yes). Call counts are exact even where callgrind's
inclusive costs are not (HWCOUNTERS_DRUPAL.md §1), so this answers "who calls X
how often".

  cg-callers.py BASE.out RUN.out M REGEX [--top N] [--sites BINARY [--caller RE] [--frame RE]]

Counts are (RUN - BASE) / M per request, summed over every callee whose name
matches REGEX, grouped by caller function (recursion suffixes 'N dropped).
--sites groups by call-site source line instead (the full inline chain of the
call instruction, outermost frame last), for callers matching --caller.
"""
import argparse, collections, os, re, subprocess

here = os.path.dirname(os.path.abspath(__file__))
src = open(os.path.join(here, "cg-handlers.py")).read().replace("\nmain()\n", "\n")
cgh = {"__file__": os.path.join(here, "cg-handlers.py")}
exec(compile(src, "cg-handlers.py", "exec"), cgh)


def calls(path, pat):
    """-> {caller: n}, {(caller, call addr): n}, [(cfn, target addr)]"""
    names = {"ob": {}, "fl": {}, "fn": {}}
    kind = {"ob": "ob", "cob": "ob", "fl": "fl", "fi": "fl", "fe": "fl", "cfi": "fl", "cfl": "fl",
            "fn": "fn", "cfn": "fn"}
    byfn, bysite, targets = collections.Counter(), collections.Counter(), []
    fn = cfn = None
    addr = 0
    pending = 0
    np = re.compile(r"^\((\d+)\)(?: (.*))?$")
    def name(k, v):
        m = np.match(v)
        if not m:
            return v
        t = names[kind[k]]
        if m.group(2) is not None:
            t[m.group(1)] = m.group(2)
        return t.get(m.group(1), "?")
    for raw in open(path, errors="replace"):
        c = raw[0]
        if c.isdigit() or c in "+-*":
            tok = raw.split()[0]
            if tok == "*":
                pass
            elif tok[0] in "+-":
                addr += int(tok)
            else:
                addr = int(tok, 16) if tok.startswith("0x") else int(tok)
            if pending:
                f = re.sub(r"'\d+$", "", fn)
                byfn[f] += pending
                bysite[(f, addr)] += pending
                pending = 0
            continue
        k, _, v = raw.rstrip("\n").partition("=")
        if k in kind:
            n = name(k, v)
            if k == "fn":
                fn = n
            elif k == "cfn":
                cfn = n
        elif k == "calls":
            t = v.split()
            if len(t) > 1 and t[1].startswith("0x"):
                targets.append((cfn, int(t[1], 16)))
            if cfn and pat.search(cfn):
                pending = int(t[0])
    return byfn, bysite, targets


def chains(binary, addrs, base):
    inp = "".join(f"0x{a - base:x}\n" for a in addrs)
    out = subprocess.run([cgh["A2L"], "-e", binary, "-i", "-a"], input=inp, capture_output=True, text=True).stdout
    res, cur = {}, None
    for l in out.splitlines():
        l = l.strip()
        if l.startswith("0x"):
            cur = int(l, 16) + base
            res[cur] = []
        elif ":" in l and cur is not None:
            p, _, ln = l.split(" (discriminator")[0].rpartition(":")
            res[cur].append(f"{'/'.join(p.split('/')[-2:])}:{ln}")
    return {a: " < ".join(c) for a, c in res.items()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("base"); ap.add_argument("run"); ap.add_argument("m", type=int); ap.add_argument("regex")
    ap.add_argument("--top", type=int, default=30)
    ap.add_argument("--sites"); ap.add_argument("--caller", default="")
    ap.add_argument("--frame", help="with --sites: group by the innermost inline frame matching this regex "
                    "(e.g. the first engine frame of an allocation's chain) plus the caller function")
    a = ap.parse_args()
    pat = re.compile(a.regex)
    f0, s0, _ = calls(a.base, pat)
    f1, s1, targets = calls(a.run, pat)
    if not a.sites:
        rows = sorted(((k, (f1[k] - f0.get(k, 0)) / a.m) for k in f1), key=lambda r: -r[1])
        print(f"calls per request to /{a.regex}/: {sum(r[1] for r in rows):,.0f}")
        for k, v in rows[: a.top]:
            print(f"{v:12,.0f}  {k[:150]}")
        return
    cp = re.compile(a.caller)
    base = cgh["load_base"](a.sites, targets)
    d = collections.Counter()
    for (f, ad), n in s1.items():
        if cp.search(f):
            d[ad] += n - s0.get((f, ad), 0)
    ch = chains(a.sites, sorted(d), base)
    fnof = {}
    for (f, ad) in s1:
        fnof.setdefault(ad, f)
    agg = collections.Counter()
    fp = re.compile(a.frame) if a.frame else None
    for ad, n in d.items():
        c = ch.get(ad, hex(ad))
        if fp:
            hit = next((fr for fr in c.split(" < ") if fp.search(fr)), None)
            c = f"{hit or '?'}  [{cgh['cgf']['short'](fnof.get(ad, '?'))[:70]}]"
        agg[c] += n
    print(f"calls per request to /{a.regex}/ from /{a.caller}/: {sum(agg.values()) / a.m:,.0f}")
    for k, v in agg.most_common(a.top):
        print(f"{v / a.m:12,.0f}  {k[:260]}")


main()
