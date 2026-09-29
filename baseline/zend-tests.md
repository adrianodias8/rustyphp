# Baseline — php-src `tests/` + `Zend/tests/`

- phpr git SHA: `9d4ef5ba3945545a0d7d9f53437b2d703ebd0ed9`
- phpt-runner binary sha256: `65578f7e69cc12c5`
- corpus: php-src `php-8.5.7` (`35eab8c0`)
- runner flags: `--isolate --list-fails --list-skips`, `PHPT_TIMEOUT_SECS=10`, default engine mode (no `PHPR_REG_LOWER` set)
- platform: `Linux 6.17.8-orbstack-00308-g8f9c941121b1 aarch64`, Debian GNU/Linux 13 (trixie), in Docker
- measured: 2026-09-29T22:31:53Z
- wall time: 229s

| scope | total | pass | fail | skip | pass rate (of runnable) |
|---|---:|---:|---:|---:|---:|
| `tests/` + `Zend/tests/` | 6172 | 3041 | 1623 | 1508 | 65.2% |
| `Zend/tests/` only | 5305 | 2655 | 1412 | 1238 | 65.3% |
| `tests/` only | 867 | 386 | 211 | 270 | 64.7% |

Upstream's claim for `Zend/tests/` (README/COVERAGE, pin S-175, macOS, oracle 8.5.7):
5305 total · 2655 pass · 1412 fail · 1238 skip = 65.3% of runnable.

## Skips by category

```
     810  compile-error
     280  section
     116  unsupported
      80  builtin
      80  extension
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
