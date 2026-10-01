# Drupal 11 front page under wrk — 2026-10-01T13:32:27Z

- ferro: `9abe909ad71cbb4d` (ae84774f), worker mode, RESET=recipe
- php-fpm: `PHP 8.5.7 (fpm-fcgi) (built: Jun 24 2026 01:24:52) (NTS)`, opcache on (validate_timestamps=0, jit off), static pool of 4, nginx
- FrankenPHP: `FrankenPHP v1.12.7 PHP 8.5.11 Caddy v2.11.4 h1:XKxkMTgNSizEvKG6QHue6cAsFOteU2qA61w2tKkCWi0=`, 4 workers, num_threads 8, GOMAXPROCS default
- wrk: 2 threads, 32 connections, 15s per run, 3 runs (median), 5s warm-up; 12 CPUs in the VM, shared by wrk and the servers
- check (token-stripped body of `/`): frankenphp: identical to php-fpm; ferro-worker: identical to php-fpm; 

| server | req/s | p50 | p99 | ferro ÷ this |
|---|---:|---:|---:|---:|
| fpm | 732.77 | 43.3ms | 48.3ms | 0.08 |
| frankenphp | 196.42 | 162.0ms | 224.8ms | 0.29 |
| ferro-worker | 57.22 | 541.6ms | 748.8ms | 1.00 |
