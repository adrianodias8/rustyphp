# Drupal 11 front page under wrk — 2026-10-01T13:35:19Z

- ferro: `9abe909ad71cbb4d` (ae84774f), worker mode, RESET=recipe
- php-fpm: `PHP 8.5.7 (fpm-fcgi) (built: Jun 24 2026 01:24:52) (NTS)`, opcache on (validate_timestamps=0, jit off), static pool of 8, nginx
- FrankenPHP: `FrankenPHP v1.12.7 PHP 8.5.11 Caddy v2.11.4 h1:XKxkMTgNSizEvKG6QHue6cAsFOteU2qA61w2tKkCWi0=`, 8 workers, num_threads 16, GOMAXPROCS default
- wrk: 2 threads, 32 connections, 15s per run, 3 runs (median), 5s warm-up; 12 CPUs in the VM, shared by wrk and the servers
- check (token-stripped body of `/`): frankenphp: identical to php-fpm; ferro-worker: identical to php-fpm; 

| server | req/s | p50 | p99 | ferro ÷ this |
|---|---:|---:|---:|---:|
| fpm | 1415.78 | 22.4ms | 27.1ms | 0.08 |
| frankenphp | 369.15 | 85.9ms | 126.8ms | 0.31 |
| ferro-worker | 114.50 | 278.4ms | 338.1ms | 1.00 |
