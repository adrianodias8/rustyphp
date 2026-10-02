# Baseline — php-src `tests/` + `Zend/tests/`

- phpr git SHA: `51ffe0147b789305c64a0aa758537812c43da957`
- phpt-runner binary sha256: `f3deb22a150fdfe0`
- corpus: php-src `php-8.5.7` (`35eab8c0`)
- runner flags: `--isolate --list-fails --list-skips`, `PHPT_TIMEOUT_SECS=10`, default engine mode (no `PHPR_REG_LOWER` set)
- platform: `Linux 6.17.8-orbstack-00308-g8f9c941121b1 aarch64`, Debian GNU/Linux 13 (trixie), in Docker
- measured: 2026-10-01T22:53:45Z
- wall time: 243s
- advanced by hand 2026-10-02 (session 10): +7 `Zend/tests/enum/{unserialize,unserialize-non-enum,unserialize-non-existent-case,unserialize-const,unserialize-missing-colon,serialize,serialization-round-trip}.phpt` (enum `E:` serialization), reported as new passes by `gate.sh`

| scope | total | pass | fail | skip | pass rate (of runnable) |
|---|---:|---:|---:|---:|---:|
| `tests/` + `Zend/tests/` | 6172 | 3111 | 1590 | 1471 | 66.2% |
| `Zend/tests/` only | 5305 | 2719 | 1379 | 1207 | 66.4% |
| `tests/` only | 867 | 392 | 211 | 264 | 65.0% |

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
