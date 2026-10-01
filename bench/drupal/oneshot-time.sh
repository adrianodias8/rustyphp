#!/bin/bash
# Warm one-shot (classic) latency of the Drupal front page: ferro -S on the
# frontpage.sh copy (/scratch/drupal-srv-fe), 25 requests, last 8 shown.
# Run inside the image: docker/run.sh bash /work/php-rust/bench/drupal/oneshot-time.sh
FERRO=${FERRO:-/target/release/ferro}
S=/scratch/drupal-srv-fe
[ -d $S ] || { echo "no $S"; exit 1; }
cd $S/web && "$FERRO" -S 127.0.0.1:8244 .ht.router.php >/scratch/t1.log 2>&1 & P=$!
sleep 1
for i in $(seq 1 25); do curl -s -o /scratch/t1.out -w "%{time_total} %{http_code} %{size_download}\n" http://127.0.0.1:8244/; done | tail -8
md5sum /scratch/t1.out
kill $P
