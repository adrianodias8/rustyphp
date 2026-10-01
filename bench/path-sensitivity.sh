#!/usr/bin/env bash
# bench/path-sensitivity.sh — how much a section moves with NOTHING but the
# script's path changed (same binary, same file). Found in session 4: oop.php's
# prop_rmw_1m ranges 171–201 ms on one binary across four path names — a
# heap-layout effect (the path's length shifts every later allocation), larger
# than the A/B deltas one is tempted to read as regressions. Run it before
# declaring a >5 % delta on one of the sections listed in SECTIONS.
#
#   docker/run.sh /work/php-rust/bench/path-sensitivity.sh [phpr] [bench] [R]
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${1:-/target/release/ferro}"; B="${2:-oop}"; R="${3:-3}"
SECTIONS="${SECTIONS:-prop_rmw_1m|prop_write_1m|static_method_call_1m}"
S="${SCRATCH:-/scratch}/path-sens"; rm -rf "$S"; mkdir -p "$S"
for d in a bench-work-ab a_much_longer_directory_name_for_the_same_script; do
  mkdir -p "$S/$d/lib"; cp "$HERE"/lib/*.php "$S/$d/lib/"; cp "$HERE/$B.php" "$S/$d/$B.php"
done
cp "$HERE/$B.php" "$S/a/x.php"
echo "# $BIN ($(sha256sum "$BIN" | cut -c1-16)), $B.php, medians of $R, sections: $SECTIONS"
for f in a/x.php a/$B.php bench-work-ab/$B.php a_much_longer_directory_name_for_the_same_script/$B.php; do
  for i in $(seq 1 "$R"); do "$BIN" -d memory_limit=-1 "$S/$f" >"$S/out.$i" 2>/dev/null; done
  python3 - "$S" "$R" "$f" "$SECTIONS" <<'PY'
import sys, re, statistics
s, r, f, secs = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
t = {}
for i in range(1, r + 1):
    for line in open(f"{s}/out.{i}"):
        m = re.match(r"TIME (\S+) ([\d.]+)", line)
        if m and re.fullmatch(secs, m.group(1)): t.setdefault(m.group(1), []).append(float(m.group(2)))
print(f"{f:60s} " + "  ".join(f"{k} {statistics.median(v):7.1f}" for k, v in t.items()))
PY
done
