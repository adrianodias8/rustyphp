# Drupal 11 front page under wrk — 2026-10-01T18:07:52Z

- ferro: `f4b19701931e00a0` (2a8f4606), worker mode, RESET=recipe; classic: `--workers 8`, fresh Vm per request, one request per connection (no keep-alive, like `php -S`)
- php-fpm: `PHP 8.5.7 (fpm-fcgi) (built: Jun 24 2026 01:24:52) (NTS)`, opcache on (validate_timestamps=0, jit off), static pool of 8, nginx
- FrankenPHP: `FrankenPHP v1.12.7 PHP 8.5.11 Caddy v2.11.4 h1:XKxkMTgNSizEvKG6QHue6cAsFOteU2qA61w2tKkCWi0=`, 8 workers, num_threads 16, GOMAXPROCS default
- wrk: 2 threads, 32 connections, 15s per run, 3 runs (median), 5s warm-up; 12 CPUs in the VM, shared by wrk and the servers
- check (token-stripped body of `/`): ferro-classic: identical to php-fpm; 

| server | req/s | p50 | p99 | ferro ÷ this |
|---|---:|---:|---:|---:|
| fpm | 1354.45 | 23.4ms | 28.7ms | 0.18 |
| ferro-classic | 247.56 | 128.2ms | 152.0ms | 1.00 |
