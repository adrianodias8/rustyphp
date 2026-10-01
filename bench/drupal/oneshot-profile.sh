#!/bin/bash
# perf profile of warm one-shot Drupal requests (30 warm-up, then N under
# perf, default 300): writes /scratch/ss2.data and prints the self-time top.
#   PRIV=1 docker/run.sh bash /work/php-rust/bench/drupal/oneshot-profile.sh
FERRO=${FERRO:-/target/release/ferro}
N=${N:-300}
cd /scratch/drupal-srv-fe/web; "$FERRO" -S 127.0.0.1:8244 .ht.router.php >/scratch/t1.log 2>&1 & P=$!
sleep 1
for i in $(seq 1 30); do curl -s -o /dev/null http://127.0.0.1:8244/; done
perf record -e cpu-clock -F ${FREQ:-1999} --call-graph ${CG:-dwarf,16384} -p $P -o /scratch/ss2.data -- sleep 600 >/dev/null 2>&1 &
R=$!
sleep 0.5
for i in $(seq 1 $N); do curl -s -o /dev/null http://127.0.0.1:8244/; done
kill -INT $R; wait $R
kill $P
perf report -i /scratch/ss2.data --no-children --percent-limit 0.4 --stdio -g none --sort sym 2>/dev/null \
  | grep -v '^#' | grep -v '^$' | rustfilt | sed -E 's/\[[^]]*\] //; s/::h[0-9a-f]{16}//' | cut -c1-140 | head -60
