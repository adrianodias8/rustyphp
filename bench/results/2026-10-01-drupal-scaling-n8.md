# Drupal classic-mode scaling: threads vs processes — 2026-10-02T00:03:33Z

- ferro ec8d6bdf, classic mode (fresh Vm per request), front page, page_cache off
- wrk: 2 threads, 4 connections per worker, 15s per run, 3 runs (median), 5s warm-up; 12 CPUs in the VM, shared by wrk and the servers
- efficiency = req/s / (workers x the same binary's 1x1 req/s)

| allocator | 1x1 req/s | 1x8 (threads) | efficiency | 8x1 (processes, SO_REUSEPORT) | efficiency |
|---|---:|---:|---:|---:|---:|
| mimalloc | 52.08 | 305.53 | 73 % | 305.79 | 73 % |
| system | 41.59 | 236.06 | 71 % | 236.08 | 71 % |
