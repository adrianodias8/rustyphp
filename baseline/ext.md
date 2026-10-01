# Baseline — per-extension `.phpt` results

- phpr git SHA: `51ffe0147b789305c64a0aa758537812c43da957`
- phpt-runner binary sha256: `f3deb22a150fdfe0`
- corpus: php-src `php-8.5.7` (`35eab8c0`)
- runner flags: `--isolate --list-fails --list-skips`, `PHPT_TIMEOUT_SECS=10`, default engine mode (no `PHPR_REG_LOWER` set)
- platform: `Linux 6.17.8-orbstack-00308-g8f9c941121b1 aarch64`, Debian GNU/Linux 13 (trixie), in Docker
- measured: 2026-10-01T23:00:58Z

"pass rate" = pass / (pass + fail); skipped tests never executed.

Skip categories are the runner's own: `extension` = the test's `--EXTENSIONS--` names an
extension outside phpt-runner's allowlist (the suite is then **not exercised at all**, whatever
phpr implements — true today for `dom`, `simplexml` and `filter`); `section` = the test uses a
.phpt section the runner does not execute (e.g. `--SKIPIF--` without `--run-skipif`, `--POST--`,
`--CGI--`); `builtin` = calls a function phpr does not register; `compile-error` = the test expects a
compile-time diagnostic the runner does not model; `parse` / `unsupported` / `vm-unsupported` =
the front end or compiler rejected a construct. 1,767 of the `section` skips across all suites are `--SKIPIF--`.
A pass rate over a handful of runnable tests (`ext/dom`: 2, `ext/pdo`: 7) says nothing about the extension.

| suite | total | pass | fail | skip | pass rate (of runnable) | wall | notes |
|---|---:|---:|---:|---:|---:|---:|---|
| `ext/standard` | 3812 | 1644 | 919 | 1249 | 64.1% | 255s | 5 crashed; 13 timed out; skips: 994 section, 141 builtin, 60 vm-unsupported |
| `ext/pcre` | 165 | 81 | 46 | 38 | 63.8% | 5s | skips: 28 section, 3 expectf-%r, 3 builtin |
| `ext/json` | 88 | 60 | 21 | 7 | 74.1% | 2s | skips: 7 section |
| `ext/mbstring` | 417 | 86 | 129 | 202 | 40.0% | 13s | skips: 139 section, 55 builtin, 2 vm-unsupported |
| `ext/hash` | 80 | 17 | 48 | 15 | 26.2% | 3s | skips: 6 section, 5 builtin, 3 vm-unsupported |
| `ext/ctype` | 49 | 46 | 2 | 1 | 95.8% | 1s | skips: 1 section |
| `ext/dom` | 867 | 1 | 1 | 865 | 50.0% | 27s | skips: 804 extension, 35 section, 26 malformed |
| `ext/simplexml` | 156 | 0 | 1 | 155 | 0.0% | 4s | skips: 150 extension, 5 section |
| `ext/pdo` | 127 | 2 | 5 | 120 | 28.6% | 4s | skips: 120 section |
| `ext/pdo_sqlite` | 85 | 31 | 40 | 14 | 43.7% | 3s | skips: 10 section, 2 extension, 1 malformed |
| `ext/sqlite3` | 96 | 34 | 46 | 16 | 42.5% | 3s | skips: 16 section |
| `ext/date` | 689 | 205 | 396 | 88 | 34.1% | 21s | skips: 50 section, 34 builtin, 2 expectf-%r |
| `ext/spl` | 787 | 221 | 489 | 77 | 31.1% | 64s | 4 timed out; skips: 42 vm-unsupported, 15 section, 12 builtin |
| `ext/reflection` | 493 | 182 | 290 | 21 | 38.6% | 15s | skips: 13 extension, 3 vm-unsupported, 3 section |
| `ext/session` | 259 | 44 | 27 | 188 | 62.0% | 8s | skips: 181 section, 3 compile-error, 2 ini |
| `ext/filter` | 120 | 0 | 0 | 120 | n/a | 3s | skips: 98 extension, 22 section |
| `ext/tokenizer` | 53 | 45 | 7 | 1 | 86.5% | 2s | skips: 1 section |

Per-extension lists: `baseline/ext/<ext>.pass.txt`, `baseline/ext/<ext>.fails.txt`.
