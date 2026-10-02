#!/usr/bin/env bash
# bench/drupal/heap-profile.sh — where one warm classic Drupal worker's memory
# goes (MEMORY_DRUPAL.md). Inside the image:  docker/run.sh bash /work/php-rust/bench/drupal/heap-profile.sh
#  1. RSS of `ferro -S --workers 1` after N front-page requests, shipped build (mimalloc)
#     and a `--features system-alloc` build (glibc malloc, what heaptrack can see).
#  2. heaptrack on the system-alloc build: the bytes still allocated when the worker
#     is stopped after N requests (between requests: what the worker keeps), as
#     folded stacks (heap-leaked.folded) and heaptrack_print's report.
#  3. smaps of the shipped build: heap vs code vs stacks vs anonymous maps.
#  4. opcache's memory for the same front page (bench/drupal/opcache-size.php).
set -uo pipefail
N="${N:-30}"; OUT="${OUT:-/scratch/heap}"; PORT=8085
HEAP_BIN=/target/heap/release/ferro
mkdir -p "$OUT"; rm -f "$OUT"/heaptrack.*
(cd /work/php-rust/php-rust && CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_TARGET_DIR=/target/heap \
  cargo build --release -p php-cli --features system-alloc 2>&1 | grep -E "^error|Finished") || exit 1
rm -rf /scratch/drupal-h && cp -a /scratch/drupal-base /scratch/drupal-h && cd /scratch/drupal-h/web || exit 1

warm() {  # pid: wait for the server, then N requests
  for i in $(seq 1 60); do curl -s -o /dev/null -f "http://127.0.0.1:$PORT/" && break; sleep 1; done
  for i in $(seq 1 "$N"); do curl -s -o /dev/null "http://127.0.0.1:$PORT/"; done
}
rss_of() { awk '/^VmRSS/{print $2}' "/proc/$1/status"; }
pid_of() { pgrep -n -f "ferro -S 127.0.0.1:$PORT"; }

# Warm the copy first: its first request rebuilds Drupal's caches (container,
# discovery), whose transient peak would otherwise inflate whichever run is first.
/target/release/ferro -S 127.0.0.1:$PORT -t . .ht.router.php --workers 1 >/dev/null 2>&1 &
warm; kill "$(pid_of)"; wait 2>/dev/null
for run in "/target/release/ferro" "/target/release/ferro MIMALLOC_PURGE_DELAY=0" "$HEAP_BIN"; do
  bin="${run%% *}"; env=""; [[ "$run" == *" "* ]] && env="${run#* }"
  env $env "$bin" -S 127.0.0.1:$PORT -t . .ht.router.php --workers 1 >/dev/null 2>&1 &
  warm; sleep 2; pid="$(pid_of)"
  echo "rss $(basename "$(dirname "$(dirname "$bin")")") ${env:-default} $(rss_of "$pid") kB after $N requests"
  if [[ "$run" == /target/release/ferro ]]; then
    awk '/^[0-9a-f]+-/{name=$6; if (name=="") name="[anon]"} /^Rss:/{rss[name]+=$2} END{for (n in rss) if (rss[n]>1024) printf "smaps %8d kB %s\n", rss[n], n}' "/proc/$pid/smaps" | sort -k2 -nr
  fi
  kill "$pid"; wait 2>/dev/null
done

heaptrack -o "$OUT/heaptrack" "$HEAP_BIN" -S 127.0.0.1:$PORT -t . .ht.router.php --workers 1 >"$OUT/heaptrack.log" 2>&1 &
warm; sleep 1; pid="$(pid_of)"
echo "rss under heaptrack $(rss_of "$pid") kB"
kill "$pid"; wait 2>/dev/null; sleep 2
f="$(ls "$OUT"/heaptrack*.zst "$OUT"/heaptrack*.gz 2>/dev/null | head -1)"
heaptrack_print -f "$f" --print-peaks 1 -n 25 --print-leaks 1 --print-allocators 0 --print-temporary 0 \
  --flamegraph-cost-type leaked -F "$OUT/heap-leaked.folded" >"$OUT/heaptrack.txt" 2>&1
grep -E "^(total runtime|calls to allocation|temporary memory|peak heap memory|peak RSS|total memory leaked)" "$OUT/heaptrack.txt"
python3 /work/php-rust/bench/drupal/heap-buckets.py <"$OUT/heap-leaked.folded"
# what opcache holds for the same page (one CLI process, warm copy)
php -d opcache.enable_cli=1 -d opcache.memory_consumption=1024 -d opcache.interned_strings_buffer=64 \
  /work/php-rust/bench/drupal/opcache-size.php "$PWD" >/dev/null
