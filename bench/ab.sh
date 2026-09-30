#!/usr/bin/env bash
# bench/ab.sh — A/B two phpr binaries on the benchmark set (PLAN.md §4: every
# slice lands only with "no slowdown on any benchmark").
#
#   docker/run.sh /work/php-rust/bench/ab.sh
#
#   A = $PHPR_A (default /target/base/release/phpr — the commit the change
#       started from, built from a git worktree)
#   B = $PHPR_B (default /target/release/phpr — the working tree)
#
# R rounds (default 7), A and B interleaved within each round and the order
# alternated between rounds (ABBA), medians per section from the in-script
# timers. Inputs staged on the container-local volume like bench/run.sh.
# Output: TSV  bench  section  A_ms  B_ms  B/A  spread_A%  spread_B%
# where spread = (max-min)/median over the rounds: a B/A inside the spreads
# is noise, not a result.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
PHP_SRC="${PHP_SRC:-$REPO/../php-src}"
A="${PHPR_A:-/target/base/release/phpr}"
B="${PHPR_B:-/target/release/phpr}"
R="${R:-7}"
ONLY="${ONLY:-zend_bench zend_micro_bench arrays strings oop autoload symfony-boot}"
SCRATCH="${SCRATCH:-/scratch}"
W="$SCRATCH/bench-work-ab"; RAW="$SCRATCH/bench-raw-ab"
rm -rf "$W" "$RAW"; mkdir -p "$W/lib" "$RAW"
cp "$HERE"/*.php "$W/"; cp "$HERE"/lib/*.php "$W/lib/"
cp "$PHP_SRC/Zend/bench.php" "$W/zend_bench.php"
# The copy both binaries can parse (A may predate the isset(Class::$static) fix).
sed 's|^\( *\)\$x = isset(Foo::\$a);|\1$x = 0; // patched out|' "$PHP_SRC/Zend/micro_bench.php" >"$W/zend_micro_bench.php"
export AUTOLOAD_DIR="$SCRATCH/autoload" SYMFONY_DIR="$SCRATCH/symfony-app"
[[ -x "$A" && -x "$B" ]] || { echo "need both binaries: $A $B"; exit 1; }
echo "# A=$A ($(sha256sum "$A" | cut -c1-16))  B=$B ($(sha256sum "$B" | cut -c1-16))  rounds=$R  loadavg=$(cut -d' ' -f1-3 /proc/loadavg)" >&2
for b in $ONLY; do
  for i in $(seq 1 "$R"); do
    if (( i % 2 )); then order="A B"; else order="B A"; fi
    for x in $order; do
      bin="$A"; [[ "$x" == B ]] && bin="$B"
      "$bin" -d memory_limit=-1 "$W/$b.php" >"$RAW/$b.$x.$i.out" 2>"$RAW/$b.$x.$i.err" || echo "rc=$? $b $x $i" >&2
    done
  done
done
python3 - "$RAW" "$R" $ONLY <<'PY'
import re, statistics, sys, os
raw, R, names = sys.argv[1], int(sys.argv[2]), sys.argv[3:]
Z = re.compile(r"^(\S.*?)\s+(\d+\.\d{3})(?:\s+-?\d+\.\d{3})?\s*$")
def parse(bench, p):
    t, res = {}, {}
    for line in open(p, errors="replace"):
        line = line.rstrip("\n")
        if line.startswith("TIME "):
            _, n, ms = line.split(" ", 2); t[n] = float(ms)
        elif line.startswith("RESULT "):
            _, n, v = line.split(" ", 2); res[n] = v
        elif bench.startswith("zend_"):
            m = Z.match(line)
            if m: t[m.group(1).strip()] = float(m.group(2)) * 1000.0
    return t, res
print("bench\tsection\tA_ms\tB_ms\tB/A\tspread_A%\tspread_B%\tnote")
worst = []
for b in names:
    data = {"A": {}, "B": {}}; res = {"A": {}, "B": {}}
    for x in "AB":
        for i in range(1, R + 1):
            t, r = parse(b, os.path.join(raw, f"{b}.{x}.{i}.out"))
            for k, v in t.items(): data[x].setdefault(k, []).append(v)
            res[x].update(r)
    for k in data["A"]:
        a, bb = data["A"][k], data["B"].get(k, [])
        if not bb:
            print(f"{b}\t{k}\t{statistics.median(a):.3f}\t-\t-\t-\t-\tMISSING in B"); continue
        ma, mb = statistics.median(a), statistics.median(bb)
        sa = 100 * (max(a) - min(a)) / ma if ma else 0
        sb = 100 * (max(bb) - min(bb)) / mb if mb else 0
        note = ""
        if res["A"].get(k) != res["B"].get(k): note = "RESULT DIFFERS"
        ratio = mb / ma if ma else float("nan")
        print(f"{b}\t{k}\t{ma:.3f}\t{mb:.3f}\t{ratio:.3f}\t{sa:.1f}\t{sb:.1f}\t{note}")
        worst.append((ratio, b, k, ma, mb, sa, sb))
PY
