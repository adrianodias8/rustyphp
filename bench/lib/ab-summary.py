#!/usr/bin/env python3
"""Summarise bench/ab.sh output: per-benchmark geometric mean of B/A and the
sections outside ±5 %, with the round-to-round spread beside each."""
import csv, statistics, sys
rows = list(csv.DictReader(open(sys.argv[1]), delimiter="\t"))
ok = [r for r in rows if r["B/A"] not in ("-", "")]
print(len(rows), "sections;", sum(1 for r in rows if r["note"]), "with notes")
for r in rows:
    if r["note"]:
        print("NOTE", r["bench"], r["section"], r["note"])
for b in dict.fromkeys(r["bench"] for r in ok):
    rs = [float(r["B/A"]) for r in ok if r["bench"] == b and r["section"] != "Total"]
    print(f"{b:18s} n={len(rs):3d} geomean B/A={statistics.geometric_mean(rs):.3f} min={min(rs):.3f} max={max(rs):.3f}")
def show(title, pred, key):
    print(title)
    for r in sorted(ok, key=key):
        if pred(float(r["B/A"])):
            print(f"  {r['bench']:18s} {r['section']:38s} A={r['A_ms']:>10s} B={r['B_ms']:>10s} B/A={r['B/A']} spreadA={r['spread_A%']}% spreadB={r['spread_B%']}%")
show("--- slower in B (B/A > 1.05)", lambda x: x > 1.05, lambda r: -float(r["B/A"]))
show("--- faster in B (B/A < 0.95)", lambda x: x < 0.95, lambda r: float(r["B/A"]))
