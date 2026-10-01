#!/bin/bash
# Warm one-shot (classic) latency of the Drupal front page: ferro -S on the
# frontpage.sh copy (/scratch/drupal-srv-fe), 25 requests, last 8 shown, then
# their mean. Run inside the image:
#   docker/run.sh bash /work/php-rust/bench/drupal/oneshot-time.sh
FERRO=${FERRO:-/target/release/ferro}
PORT=${PORT:-8244}
S=/scratch/drupal-srv-fe
[ -d $S ] || { echo "no $S"; exit 1; }
cd $S/web
"$FERRO" -S 127.0.0.1:$PORT .ht.router.php >/scratch/t1.log 2>&1 & P=$!
for i in $(seq 1 50); do curl -s -o /dev/null http://127.0.0.1:$PORT/ && break; sleep 0.2; done
kill -0 $P 2>/dev/null || { echo "server did not start:"; cat /scratch/t1.log; exit 1; }
for i in $(seq 1 25); do curl -s -o /scratch/t1.out -w "%{time_total} %{http_code} %{size_download}\n" http://127.0.0.1:$PORT/; done | tail -8 \
  | tee /dev/stderr | awk '{s+=$1} END{printf "mean %.2f ms\n", s/NR*1000}'
kill $P; wait $P 2>/dev/null
