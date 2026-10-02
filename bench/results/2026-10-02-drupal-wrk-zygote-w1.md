# Drupal 11 front page under wrk — 2026-10-02T09:22:54Z

- ferro: `0a4052fffd223b5e` (ef6dd8b7), worker mode, RESET=recipe; classic: `--workers 1`, fresh Vm per request, one request per connection (no keep-alive, like `php -S`)
- php-fpm: `PHP 8.5.7 (fpm-fcgi) (built: Jun 24 2026 01:24:52) (NTS)`, opcache on (validate_timestamps=0, jit off), static pool of 1, nginx
- FrankenPHP: `FrankenPHP v1.12.7 PHP 8.5.11 Caddy v2.11.4 h1:XKxkMTgNSizEvKG6QHue6cAsFOteU2qA61w2tKkCWi0=`, 1 workers, num_threads 2, GOMAXPROCS default
- wrk: 2 threads, 8 connections, 15s per run, 3 runs (median), 5s warm-up; 12 CPUs in the VM, shared by wrk and the servers
- check (token-stripped body of `/`): ferro-classic: identical to php-fpm; ferro-zygote: identical to php-fpm; 

| server | req/s | p50 | p99 | ferro ÷ this |
|---|---:|---:|---:|---:|
| ferro-classic | 45.77 | 173.2ms | 202.0ms | 1.00 |
| ferro-zygote | 49.65 | 157.6ms | 406.8ms | 0.92 |
