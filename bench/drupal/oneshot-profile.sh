#!/bin/bash
# perf profile of warm one-shot Drupal requests (30 warm-up, then 90 under
# perf): self and inclusive tops. PRIV=1 docker/run.sh bash /work/php-rust/bench/drupal/oneshot-profile.sh
FERRO=${FERRO:-/target/release/ferro}
cd /scratch/drupal-srv-fe/web; "$FERRO" -S 127.0.0.1:8244 .ht.router.php >/scratch/t1.log 2>&1 & P=$!
sleep 1
for i in $(seq 1 30); do curl -s -o /dev/null http://127.0.0.1:8244/; done
perf record -e cpu-clock -F 499 --call-graph dwarf,16384 -p $P -o /scratch/ss2.data -- sleep 8 >/dev/null 2>&1 &
R=$!
sleep 0.3
for i in $(seq 1 90); do curl -s -o /dev/null http://127.0.0.1:8244/; done
wait $R
kill $P
perf report -i /scratch/ss2.data --no-children --percent-limit 0.8 --stdio -g none 2>/dev/null | grep -v '^#' | grep -v '^$' | head -45
echo ==== inclusive
perf report -i /scratch/ss2.data --children --percent-limit 4 --stdio -g none 2>/dev/null | grep -v '^#' | grep -v '^$' | head -45
