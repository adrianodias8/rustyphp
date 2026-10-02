# Drupal 11 front page under wrk — 2026-10-01T23:50:15Z

- ferro: `596b4574a4cf3fba` (ec8d6bdf), worker mode, RESET=recipe; classic: `--workers 4`, fresh Vm per request, one request per connection (no keep-alive, like `php -S`)
- php-fpm: `PHP 8.5.7 (fpm-fcgi) (built: Jun 24 2026 01:24:52) (NTS)`, opcache on (validate_timestamps=0, jit off), static pool of 4, nginx
- FrankenPHP: `FrankenPHP v1.12.7 PHP 8.5.11 Caddy v2.11.4 h1:XKxkMTgNSizEvKG6QHue6cAsFOteU2qA61w2tKkCWi0=`, 4 workers, num_threads 8, GOMAXPROCS default
- wrk: 2 threads, 32 connections, 15s per run, 3 runs (median), 5s warm-up; 12 CPUs in the VM, shared by wrk and the servers
- check (token-stripped body of `/`): frankenphp: identical to php-fpm; ferro-worker: identical to php-fpm; 

| server | req/s | p50 | p99 | ferro ÷ this |
|---|---:|---:|---:|---:|
| fpm | 752.87 | 42.2ms | 46.1ms | 0.10 |
| frankenphp | 184.11 | 172.6ms | 203.9ms | 0.39 |
| ferro-worker | 72.47 | 439.7ms | 553.5ms | 1.00 |
