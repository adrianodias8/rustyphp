"""Geomean per section, and the rows above 1.05 or with differing results,
of a bench/ab.sh output (tab-separated rows: section, bench, A, B, B/A, ...)."""
import collections
import math
import sys

g = collections.defaultdict(list)
bad = []
for line in open(sys.argv[1]):
    f = line.rstrip("\n").split("\t")
    if len(f) < 5:
        continue
    try:
        r = float(f[4])
    except ValueError:
        continue
    g[f[0]].append(r)
    if r > 1.05 or r < 0.85 or "DIFFERS" in line:
        bad.append(line.rstrip())
for k, v in g.items():
    print(f"{k:16} {len(v):3} geomean {math.exp(sum(map(math.log, v)) / len(v)):.3f}")
print("rows outside [0.85, 1.05] or differing:")
print("\n".join(bad))
