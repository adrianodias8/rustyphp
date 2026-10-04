#!/usr/bin/env python3
"""Per-function Ir difference between two engines' cachegrind.sh run pairs
(A = before, B = after), per request: what a change added and removed.

  cg-diff.py A0.out A20.out B0.out B20.out M [--top N]
"""
import argparse, collections, os

here = os.path.dirname(os.path.abspath(__file__))
src = open(os.path.join(here, "cg-funcs.py")).read().replace("\nmain()\n", "\n")
cgf = {"__file__": os.path.join(here, "cg-funcs.py")}
exec(compile(src, "cg-funcs.py", "exec"), cgf)


def per_fn(p0, p1, m):
    _, r0 = cgf["load"](p0)
    _, r1 = cgf["load"](p1)
    out = collections.Counter()
    for (fl, fn, ln), v in r1.items():
        out[fn] += v[0]
    for (fl, fn, ln), v in r0.items():
        out[fn] -= v[0]
    return {k: v / m for k, v in out.items()}


def main():
    ap = argparse.ArgumentParser()
    for x in ("a0", "a1", "b0", "b1"):
        ap.add_argument(x)
    ap.add_argument("m", type=int); ap.add_argument("--top", type=int, default=25)
    a = ap.parse_args()
    A, B = per_fn(a.a0, a.a1, a.m), per_fn(a.b0, a.b1, a.m)
    d = sorted(((k, B.get(k, 0) - A.get(k, 0), A.get(k, 0), B.get(k, 0)) for k in set(A) | set(B)), key=lambda r: r[1])
    print(f"total A {sum(A.values()):,.0f}  B {sum(B.values()):,.0f}  delta {sum(B.values()) - sum(A.values()):+,.0f}")
    print("| delta | A | B | function |\n|---:|---:|---:|---|")
    for k, dv, x, y in d[: a.top] + [None] + d[-a.top:][::-1] if False else d[: a.top] + d[-a.top:][::-1]:
        print(f"| {dv:+,.0f} | {x:,.0f} | {y:,.0f} | `{k[:110]}` |")


main()
