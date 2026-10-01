#!/bin/bash
# Interleaved warm one-shot Drupal timing of two binaries (cancels host drift):
#   ab-oneshot.sh [A] [B] [ROUNDS]   defaults: /target/head/release/ferro /target/release/ferro 6
A=${1:-/target/head/release/ferro}; B=${2:-/target/release/ferro}; R=${3:-6}
for r in $(seq 1 $R); do
  for b in "$A" "$B"; do
    echo "$b $(FERRO=$b bash /work/php-rust/bench/drupal/oneshot-time.sh 2>/dev/null | awk '{print $2}')"
  done
done | python3 -c '
import sys, statistics, collections
d = collections.defaultdict(list)
for l in sys.stdin:
    b, v = l.split()
    d[b].append(float(v))
for b, v in d.items():
    print(f"{b:32} mean {statistics.mean(v):.2f}  median {statistics.median(v):.2f}  {v}")
'
