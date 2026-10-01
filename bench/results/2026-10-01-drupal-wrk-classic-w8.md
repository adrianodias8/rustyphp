# Drupal 11 front page under wrk — 2026-10-01T15:08:26Z

- ferro: `9ebd6553028240d4` (ae84774f), worker mode, RESET=recipe; classic: `--workers 8`, fresh Vm per request, one request per connection (no keep-alive, like `php -S`)
- php-fpm: `PHP 8.5.7 (fpm-fcgi) (built: Jun 24 2026 01:24:52) (NTS)`, opcache on (validate_timestamps=0, jit off), static pool of 8, nginx
- FrankenPHP: `FrankenPHP v1.12.7 PHP 8.5.11 Caddy v2.11.4 h1:XKxkMTgNSizEvKG6QHue6cAsFOteU2qA61w2tKkCWi0=`, 8 workers, num_threads 16, GOMAXPROCS default
- wrk: 2 threads, 32 connections, 15s per run, 3 runs (median), 5s warm-up; 12 CPUs in the VM, shared by wrk and the servers
- check (token-stripped body of `/`): ferro-worker: identical to php-fpm; ferro-classic: identical to php-fpm; 

| server | req/s | p50 | p99 | ferro ÷ this |
|---|---:|---:|---:|---:|
| fpm | 1355.31 | 23.0ms | 30.0ms | 0.11 |
| ferro-worker | 149.75 | 211.9ms | 263.5ms | 1.00 |
| ferro-classic | 219.18 | 144.7ms | 174.0ms | 0.68 |
