"""Self time spent INSIDE a function's own machinery: samples whose stack
contains FUNC with no RUNLOOP frame between FUNC and the leaf (i.e. not the
PHP code it runs). Groups by the frame just below FUNC and by the leaf.
  perf script | rustfilt | python3 within.py FUNC [RUNLOOP]"""
import collections
import re
import sys

func = sys.argv[1]
stop = sys.argv[2] if len(sys.argv) > 2 else "Vm>::run_loop"
n = 0
hit = 0
below = collections.Counter()
leaf = collections.Counter()
stack = []


def flush():
    global n, hit
    if not stack:
        return
    n += 1
    # stack[0] is the leaf; find the innermost FUNC frame
    idx = next((i for i, f in enumerate(stack) if func in f), None)
    if idx is None:
        return
    if any(stop in f for f in stack[:idx]):
        return
    hit += 1
    below[stack[idx - 1] if idx > 0 else "(self)"] += 1
    leaf[stack[0]] += 1


for line in sys.stdin:
    if not line.strip():
        flush()
        stack = []
        continue
    if line[0] not in " \t":
        continue
    m = re.match(r"\s*[0-9a-f]+\s+(.*?)(\+0x[0-9a-f]+)?\s+\(", line)
    if m:
        stack.append(re.sub(r"::h[0-9a-f]{16}", "", m.group(1))[:110])
flush()
print(f"{hit} of {n} samples ({100 * hit / max(n, 1):.2f}%) in {func}'s own machinery")
print("-- by callee of", func, "--")
for k, v in below.most_common(18):
    print(f"{100 * v / n:6.2f}%  {k}")
print("-- by leaf --")
for k, v in leaf.most_common(18):
    print(f"{100 * v / n:6.2f}%  {k}")
