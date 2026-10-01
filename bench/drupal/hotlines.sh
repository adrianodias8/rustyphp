#!/bin/bash
# Leaf-sample histogram of one function mapped to source lines via
# `objdump -l` (perf annotate crashes on these binaries; build the binary
# with CARGO_PROFILE_RELEASE_DEBUG=line-tables-only). Inside the image:
#   hotlines.sh PERF_DATA BINARY SYMBOL_SUBSTRING [N]
D=$1; BIN=$2; SYM=$3; N=${4:-40}
perf script -i "$D" -F ip,sym,symoff -G 2>/dev/null | grep "$SYM+0x" | sed 's/.*+0x//' > /scratch/hl_offs.txt
read base size < <(nm -S "$BIN" | grep "$SYM\$" | head -1 | awk '{print $1, $2}')
objdump -d -l --start-address=0x$base --stop-address=$((0x$base + 0x$size)) "$BIN" > /scratch/hl_dis.txt
python3 - "$base" "$N" <<'PY'
import collections, re, sys
base = int(sys.argv[1], 16); n = int(sys.argv[2])
line_of = {}; cur = "?"
for l in open("/scratch/hl_dis.txt"):
    m = re.match(r"^(/\S+):(\d+)", l)
    if m:
        f = m.group(1)
        f = f.split("/crates/")[-1] if "/crates/" in f else "std:" + f.split("/library/")[-1]
        cur = f"{f}:{m.group(2)}"
        continue
    m = re.match(r"^\s+([0-9a-f]+):", l)
    if m:
        line_of[int(m.group(1), 16)] = cur
c = collections.Counter()
tot = 0
for l in open("/scratch/hl_offs.txt"):
    a = base + int(l.strip(), 16); tot += 1
    c[line_of.get(a, "?")] += 1
print(f"{tot} samples in symbol")
for k, v in c.most_common(n):
    print(f"{100*v/tot:5.1f}%  {k}")
# the hottest instructions, with their line
dis = {}
for l in open("/scratch/hl_dis.txt"):
    m = re.match(r"^\s+([0-9a-f]+):\s+[0-9a-f]+\s+(.*)", l)
    if m:
        dis[int(m.group(1), 16)] = m.group(2).strip()
ic = collections.Counter(base + int(l.strip(), 16) for l in open("/scratch/hl_offs.txt"))
print("-- instructions --")
for a, v in ic.most_common(n):
    print(f"{100*v/tot:5.1f}%  {a:x}  {line_of.get(a, '?'):40} {dis.get(a, '?')[:60]}")
PY
