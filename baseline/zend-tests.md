# Baseline — php-src `tests/` + `Zend/tests/`

- phpr git SHA: `e38b1997526820ac2e56834b723c15be36fa9709`
- phpt-runner binary sha256: `2fd2ac396925c4d8`
- corpus: php-src `php-8.5.7` (`35eab8c0`)
- runner flags: `--isolate --list-fails --list-skips`, `PHPT_TIMEOUT_SECS=10`, default engine mode (no `PHPR_REG_LOWER` set)
- platform: `Linux 6.17.8-orbstack-00308-g8f9c941121b1 aarch64`, Debian GNU/Linux 13 (trixie), in Docker
- measured: 2026-09-30T17:51:44Z
- wall time: 242s

| scope | total | pass | fail | skip | pass rate (of runnable) |
|---|---:|---:|---:|---:|---:|
| `tests/` + `Zend/tests/` | 6172 | 3048 | 1625 | 1499 | 65.2% |
| `Zend/tests/` only | 5305 | 2661 | 1414 | 1230 | 65.3% |
| `tests/` only | 867 | 387 | 211 | 269 | 64.7% |

Upstream's claim for `Zend/tests/` (README/COVERAGE, pin S-175, macOS, oracle 8.5.7):
5305 total · 2655 pass · 1412 fail · 1238 skip = 65.3% of runnable.

## Skips by category

```
     810  compile-error
     280  section
     108  unsupported
      80  extension
      79  builtin
      70  parse
      62  vm-unsupported
       6  malformed
       3  ini
       1  expectf-%r
```

## Failure kinds

| kind | count |
|---|---:|
| isolated worker crashed | 48 |
| isolated worker timed out | 5 |

Files: `zend-tests.pass.txt` (regression gate input), `zend-tests.fails.txt`, `zend-tests.skips.tsv` (path, category, reason).
