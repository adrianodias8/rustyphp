# Drupal classic-mode scaling: threads vs processes — 2026-10-02T12:00:05Z

- ferro 09c9410f, classic mode (fresh Vm per request), front page, page_cache off
- wrk: 2 threads, 4 connections per worker, 15s per run, 3 runs (median), 5s warm-up; 12 CPUs in the VM, shared by wrk and the servers
- efficiency = req/s / (workers x the same binary's 1x1 req/s); RSS = total resident MB of the ferro processes after the runs

| allocator | 1x1 req/s | 1x8 (threads) | efficiency | RSS | 8x1 (processes, SO_REUSEPORT) | efficiency | RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| mimalloc | 45.65 | 277.27 | 76 % | 1598 MB | 278.24 | 76 % | 1728 MB |
| purge0 | 46.15 | 263.76 | 71 % | 1488 MB | 265.91 | 72 % | 1678 MB |
| system | 38.73 | 210.48 | 68 % | 1349 MB | 211.82 | 68 % | 1484 MB |
