# bench/drupal — Drupal 11 under ferro

Everything runs in the dev image; the codebases live on the scratch volume.

```bash
# 1. codebase (oracle composer), once
docker/run.sh bash -c 'cd /scratch && COMPOSER_HOME=/scratch/.composer composer create-project \
  --no-interaction drupal/recommended-project:^11 drupal && cd drupal && \
  COMPOSER_HOME=/scratch/.composer composer require --no-interaction drush/drush'
# 2. install parity (step 1): drush site:install standard on SQLite, fresh copy per run
docker/run.sh /work/php-rust/bench/drupal/install.sh                 # ferro
ENGINE=php docker/run.sh /work/php-rust/bench/drupal/install.sh      # oracle
# 3. the shared base for page parity: the oracle's install, page_cache off,
#    the private key generated once (it is created lazily and randomly,
#    which would make permissionsHash differ between two servers)
docker/run.sh bash -c 'cd /scratch && rm -rf drupal-base && cp -a drupal-php drupal-base && cd drupal-base && \
  php vendor/drush/drush/drush.php pm:uninstall page_cache -y && \
  php vendor/drush/drush/drush.php php:eval "\Drupal::service(\"private_key\")->get();"'
# 4. front-page parity (step 2): oracle php -S vs ferro -S on copies of the base
docker/run.sh /work/php-rust/bench/drupal/frontpage.sh
# 5. worker mode: one ferro worker vs the oracle's one-shot, per-request diff
#    (drupal-worker.php; RESET selects what is reset between requests, `recipe`
#    is the minimal set that makes 10/10 identical)
docker/run.sh bash -c 'RESET=recipe /work/php-rust/bench/drupal/worker-leaks.sh'
```

Throughput (run from the host; each arm on its own copy of the base):

```bash
# php-fpm+opcache vs FrankenPHP worker vs ferro worker vs ferro classic pool
ARMS="fpm frankenphp ferro-worker ferro-classic" WORKERS=8 bench/drupal/bench-wrk-drupal.sh
# classic-mode scaling: 1x1, 1xN threads, Nx1 processes (SO_REUSEPORT), per allocator
# (needs a `--features system-alloc` build in /target/sysalloc for the second arm)
N=8 bench/drupal/scaling.sh
```

Single-request timing and analysis (inside the image):

| script | what it gives |
|---|---|
| `oneshot-time.sh` | warm one-shot latency of `/` (`FERRO=`, `PORT=`), mean of the last 8 of 25 |
| `ab-oneshot.sh [A] [B] [ROUNDS]` | the same, two binaries interleaved (default `/target/head` vs `/target/release`) |
| `phases.php` | autoload / boot / `handle()` / send+terminate split of one request (copy into the web root, point a `-S` router at it) |
| `optime.sh [build]` | op-time census: ns per op kind and per called function (`/target/census`, `--features php-runtime/op-census`); `GCMODE=classic` for the old GC |
| `oneshot-profile.sh` | perf record of warm requests (`CG=fp FERRO=/target/fp/release/ferro` with the frame-pointer build) |
| `fold.py`, `buckets.py`, `within.py`, `hotlines.sh` | perf script analysis: self/inclusive/callers, by subsystem, a function's own machinery, hot source lines |

Results: `MISSING_FOR_DRUPAL.md` (what was missing, install and page parity),
`bench/results/*-drupal-*.md` (every wrk and scaling run) and `NOTES.md` (sessions 7–9).
