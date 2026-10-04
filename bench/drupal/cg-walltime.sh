#!/usr/bin/env bash
# bench/drupal/cg-walltime.sh — native wall time of the same two servers that
# cachegrind.sh measures (ferro -S classic; php -S + opcache, enable_cli=1),
# interleaved rounds on the sites cachegrind.sh left warm, so the instruction
# ratio can be compared to the time ratio (equal ratios = equal CPI).
#   docker/run.sh bash /work/php-rust/bench/drupal/cg-walltime.sh
set -uo pipefail
ROUNDS=${ROUNDS:-8}; N=${N:-25}
FERRO=${FERRO:-/target/release/ferro}; PHP=${PHP:-/usr/local/bin/php}
FC=/scratch/cg/fc-wall; rm -rf $FC; mkdir -p $FC
(cd /scratch/drupal-cg-ferro-rel/web && exec "$FERRO" -S 127.0.0.1:8281 .ht.router.php >/dev/null 2>&1) & A=$!
(cd /scratch/drupal-cg-php-oracle/web && exec "$PHP" -n -d extension=gd -d extension=sodium -d opcache.enable=1 \
  -d opcache.enable_cli=1 -d opcache.jit=off -d opcache.validate_timestamps=0 -d opcache.memory_consumption=128 \
  -d opcache.max_accelerated_files=10000 -d opcache.file_cache=$FC -S 127.0.0.1:8282 .ht.router.php >/dev/null 2>&1) & B=$!
for p in 8281 8282; do for i in $(seq 1 60); do curl -s -o /dev/null -f http://127.0.0.1:$p/ && break; sleep 0.5; done; done
for p in 8281 8282; do for i in $(seq 1 20); do curl -s -o /dev/null http://127.0.0.1:$p/; done; done
: > /scratch/cg/wall.tsv
for r in $(seq 1 "$ROUNDS"); do for p in 8281 8282; do for i in $(seq 1 "$N"); do
  echo -e "$p\t$(curl -s -o /dev/null -w '%{time_total}' http://127.0.0.1:$p/)" >> /scratch/cg/wall.tsv
done; done; done
kill $A $B 2>/dev/null
python3 - <<'PY'
import statistics, collections
v = collections.defaultdict(list)
for l in open("/scratch/cg/wall.tsv"):
    p, t = l.split(); v[p].append(float(t) * 1000)
f, p = statistics.median(v["8281"]), statistics.median(v["8282"])
print(f"ferro -S {f:.2f} ms  php -S+opcache {p:.2f} ms  ratio {f / p:.2f}x  (medians, {len(v['8281'])} requests each)")
PY
