#!/bin/bash
# Leaf-sample histogram of one function mapped to source lines, inline-aware
# (perf annotate crashes on these binaries; GNU addr2line cannot read their
# line tables). Needs gimli's addr2line:
#   CARGO_TARGET_DIR=/target/tools cargo install addr2line --features bin --root /target/tools
# and a binary built with CARGO_PROFILE_RELEASE_DEBUG=line-tables-only.
# Inside the image:  hotlines.sh PERF_DATA BINARY SYMBOL_SUBSTRING [N] [OUTER_FILE]
# Each sample is charged to the OUTERMOST frame of the inline chain that lies
# in OUTER_FILE (default vm/run.rs: the op arm of run_loop) and, separately,
# to its innermost line.
D=$1; BIN=$2; SYM=$3; N=${4:-40}; OUTER=${5:-vm/run.rs}
A2L=/target/tools/bin/addr2line
perf script -i "$D" -F ip,sym,symoff -G 2>/dev/null | grep "$SYM+0x" | sed 's/.*+0x//' > /scratch/hl_offs.txt
read base size < <(nm -S "$BIN" | grep "$SYM\$" | head -1 | awk '{print $1, $2}')
sort -u /scratch/hl_offs.txt | while read off; do printf '0x%x\n' $((0x$base + 0x$off)); done > /scratch/hl_addrs.txt
"$A2L" -e "$BIN" -i -a < /scratch/hl_addrs.txt > /scratch/hl_sym.txt
python3 - "$base" "$N" "$OUTER" <<'PY'
import collections, sys
base = int(sys.argv[1], 16); n = int(sys.argv[2]); outer = sys.argv[3]
chains = {}; cur = None
for l in open("/scratch/hl_sym.txt"):
    l = l.strip()
    if l.startswith("0x"):
        cur = int(l, 16); chains[cur] = []
    elif cur is not None and ":" in l:
        f = l.split("/crates/")[-1] if "/crates/" in l else "std:" + l.split("/library/")[-1].split("/src/")[-1]
        chains[cur].append(f)
outer_c = collections.Counter(); inner_c = collections.Counter(); tot = 0
for l in open("/scratch/hl_offs.txt"):
    a = base + int(l.strip(), 16); tot += 1
    ch = chains.get(a, ["?"])
    inner_c[ch[0]] += 1
    o = [f for f in ch if outer in f]
    outer_c[o[-1] if o else "?"] += 1
print(f"{tot} samples in symbol")
print(f"-- by outermost {outer} line --")
for k, v in outer_c.most_common(n):
    print(f"{100*v/tot:5.1f}%  {k}")
print("-- by innermost line --")
for k, v in inner_c.most_common(15):
    print(f"{100*v/tot:5.1f}%  {k}")
PY
