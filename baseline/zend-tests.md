# Baseline — php-src `tests/` + `Zend/tests/`

- phpr git SHA: `4565b1e56d6f7928336298246cd84fee3fd9fef3`
- phpt-runner binary sha256: `af51152f80b8aeef`
- corpus: php-src `php-8.5.7` (`35eab8c0`)
- runner flags: `--isolate --list-fails --list-skips`, `PHPT_TIMEOUT_SECS=10`, default engine mode (no `PHPR_REG_LOWER` set)
- platform: `Linux 6.17.8-orbstack-00308-g8f9c941121b1 aarch64`, Debian GNU/Linux 13 (trixie), in Docker
- measured: 2026-10-01T13:19:17Z
- wall time: 240s

| scope | total | pass | fail | skip | pass rate (of runnable) |
|---|---:|---:|---:|---:|---:|
| `tests/` + `Zend/tests/` | 6172 | 3103 | 1598 | 1471 | 66.0% |
| `Zend/tests/` only | 5305 | 2712 | 1386 | 1207 | 66.2% |
| `tests/` only | 867 | 391 | 212 | 264 | 64.8% |

Upstream's claim for `Zend/tests/` (README/COVERAGE, pin S-175, macOS, oracle 8.5.7):
5305 total · 2655 pass · 1412 fail · 1238 skip = 65.3% of runnable.

## Skips by category

```
     811  compile-error
     280  section
      85  unsupported
      80  extension
      79  builtin
      70  parse
      56  vm-unsupported
       6  malformed
       3  ini
       1  expectf-%r
```

## Failure kinds

| kind | count |
|---|---:|
| isolated worker crashed | 13 |
| isolated worker timed out | 5 |

Files: `zend-tests.pass.txt` (regression gate input), `zend-tests.fails.txt`, `zend-tests.skips.tsv` (path, category, reason).
