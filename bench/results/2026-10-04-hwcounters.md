# 2026-10-04 — cachegrind counters, Drupal warm front page, ferro vs php

Full analysis: `HWCOUNTERS_DRUPAL.md`. Raw per-request numbers below (20 measured requests,
subtraction of a run without them).

| event | ferro (`/target/cg`, = `/target/release` ±0.1 %) | php (`-g` twin, = official ±0.1 %) | ratio |
|---|---:|---:|---:|
| Ir | 181,223,817 | 38,799,286 | 4.67× |
| I1mr | 496,525 | 87,552 | 5.67× |
| D1mr | 993,921 | 180,626 | 5.50× |
| D1mw | 193,381 | 47,321 | 4.09× |
| DLmr | 118,395 | 149 | — |
| DLmw | 26,841 | 29 | — |
| ILmr | 4,537 | 15 | — |
| Bc | 25,518,607 | 5,423,848 | 4.70× |
| Bcm | 2,088,735 | 530,466 | 3.94× |
| Bi | 1,305,309 | 633,153 | 2.06× |
| Bim | 608,887 | 205,589 | 2.96× |

Shipped binaries:

| binary | Ir per request |
|---|---:|
| `/target/release/ferro` | 181,373,209 |
| `/usr/local/bin/php` | 38,818,775 |

Callgrind, 10 measured requests:

| engine | Ir per request |
|---|---:|
| ferro | 182,224,252 |
| php | 38,746,778 |

Wall time, the same servers natively, interleaved, medians of 200 (`cg-walltime.sh`):

| run | ferro | php -S + opcache | ratio |
|---|---:|---:|---:|
| 1 | 20.33 ms | 5.06 ms | 4.01× |
| 2 | 20.01 ms | 5.04 ms | 3.97× |

`handle()` (phases.sh, N=180, ROUNDS=6):

| engine | bootstrap | handle | total |
|---|---:|---:|---:|
| ferro | 2.19 ms | 17.67 ms | 20.17 ms |
| php-fpm | 1.12 ms | 3.79 ms | 5.11 ms |
| ratio | | 4.66× | |
