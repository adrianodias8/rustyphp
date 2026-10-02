# Drupal 11 front page under wrk — 2026-10-02T00:04:51Z

- ferro: `eafa8fbe7a4ae595` (ec8d6bdf), worker mode, RESET=recipe; classic: `--workers 1`, fresh Vm per request, one request per connection (no keep-alive, like `php -S`)
- php-fpm: `PHP 8.5.7 (fpm-fcgi) (built: Jun 24 2026 01:24:52) (NTS)`, opcache on (validate_timestamps=0, jit off), static pool of 1, nginx
- FrankenPHP: `FrankenPHP v1.12.7 PHP 8.5.11 Caddy v2.11.4 h1:XKxkMTgNSizEvKG6QHue6cAsFOteU2qA61w2tKkCWi0=`, 1 workers, num_threads 2, GOMAXPROCS default
- wrk: 2 threads, 4 connections, 15s per run, 3 runs (median), 5s warm-up; 12 CPUs in the VM, shared by wrk and the servers
- check (token-stripped body of `/`): 

| server | req/s | p50 | p99 | ferro ÷ this |
|---|---:|---:|---:|---:|
| fpm | 206.94 | 19.0ms | 21.8ms | - |
