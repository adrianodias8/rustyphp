"""Fold `perf script` output (one blank-line-separated sample per stack, leaf
first) into self / inclusive / caller tables.
  perf script -i X | rustfilt | python3 fold.py [SYMBOL-SUBSTRING ...]
Without arguments: top self and inclusive functions. With a substring: the
immediate callers (by sample share) of each matching leaf function."""
import collections
import re
import sys

want = sys.argv[1:]
selfc = collections.Counter()
incl = collections.Counter()
callers = collections.defaultdict(collections.Counter)
n = 0


def name(line):
    m = re.match(r"\s*[0-9a-f]+\s+(.*?)(\+0x[0-9a-f]+)?\s+\(", line)
    if not m:
        return None
    s = re.sub(r"::h[0-9a-f]{16}", "", m.group(1))
    return s[:110]


def flush(stack):
    global n
    if not stack:
        return
    n += 1
    selfc[stack[0]] += 1
    for f in set(stack):
        incl[f] += 1
    for w in want:
        if w in stack[0]:
            caller = next((f for f in stack[1:] if f != stack[0]), "?")
            callers[w][caller] += 1


stack = []
for line in sys.stdin:
    if not line.strip():
        flush(stack)
        stack = []
        continue
    if line[0] not in " \t":
        continue
    f = name(line)
    if f:
        stack.append(f)
flush(stack)
if not want:
    print(f"{n} samples\n-- self --")
    for f, c in selfc.most_common(45):
        print(f"{100 * c / n:6.2f}%  {f}")
    print("-- inclusive --")
    for f, c in incl.most_common(70):
        print(f"{100 * c / n:6.2f}%  {f}")
for w in want:
    tot = sum(callers[w].values())
    print(f"== {w}: {100 * tot / max(n, 1):.2f}% self")
    for f, c in callers[w].most_common(12):
        print(f"   {100 * c / max(n, 1):6.2f}%  {f}")
