# file_exists and unserialize on the Drupal request — session 12, step 6(c)

All interleaved; base = `121e0de7`, B = the working tree of this commit.

## file_exists

| measure | php 8.5.7 + opcache | ferro |
|---|---:|---:|
| `file_exists` calls per warm front page (stat-family syscalls, `perf stat`) | 674 `faccessat` | 678 `statx` |
| time in `file_exists` per request (perf, 100 requests, 20 kHz) | 0.42 ms (8.3 % of samples) | 0.61 ms (2.9 %) |
| per call | ~625 ns | ~900 ns |

Every call is Composer's PSR-4 probe (`ClassLoader::findFileWithExtension`, 90 classmap entries
on this site); the cost is the kernel's path walk on both engines. The census figure (1.4 µs per
call) included the census's own timing. PHP's `php_stat` answers FS_EXISTS with `access(F_OK)`,
uncached (its stat cache is one entry, for `stat`/`is_file`…, and the realpath cache is not
consulted for a plain path), so a cache would not be PHP-faithful: `file_exists` now calls
`access(F_OK)` like PHP (no attribute fetch).

## unserialize, the site's 227 cache rows (2.6 MiB), best of 15

| engine | ms | ns/KiB | / php |
|---|---:|---:|---:|
| php 8.5.7 | 2.63 | 983 | 1.00 |
| ferro base | 5.43 | 2027 | 2.06 |
| ferro, fused digit parsing only | 5.22 | 1951 | 1.98 |
| ferro, single pass (this commit) | 4.75–4.81 | 1770–1796 | 1.81 |
| (tried, reverted) + per-call key cache | 5.05 | 1890 | — |

Top cost before: the validation pass (`skip_value` + half of `quoted_slice`: 28 % of
`unserialize()`), every byte read twice. Now a port of `var_unserializer.re` that builds in one
pass; payloads with `R:`/`C:` (found by a `memmem` for `;R:`/`;C:`) keep the two-phase path.

Differential fuzz (`bench/calls/unser-fuzz.php`, 8,037 cases: seeds + random mutations, oracle vs
ferro, output = value, warnings, magic-method trace): differing cases 1,658 of 3,037 (base) →
57 of 3,037 and 82 of 5,000; every remaining one is D-26 (destructor order), D-27 (typed /
dynamic / mangled property checks) — both pre-existing.

## handle() (bench/drupal/phases.sh, N=180, 6 rounds, medians of 151)

| engine | bootstrap | handle | total |
|---|---:|---:|---:|
| php-fpm | 1.05 ms | 3.55 ms | 4.79 ms |
| ferro base | 1.97 ms | 16.41 ms | 18.70 ms |
| ferro (this commit) | 1.97 ms | **16.27 ms** | 18.59 ms |

file_exists alone (earlier run): 16.43 → 16.36 ms.

## bench/ab.sh (R=7), base vs this commit

geomean 0.998 over 123 rows; `unserialize_x5` 0.873, `prop_write_1m` 0.949; one row above 1.05,
`explode_implode_1_3mb_x20` 1.055, inside its spreads (7.5 % / 6.0 %) and untouched code.
