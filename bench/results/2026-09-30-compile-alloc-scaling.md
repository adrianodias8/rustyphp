# Compile cost, allocations per iteration, concat scaling — 2026-09-30

Companion to [`2026-09-30.md`](2026-09-30.md) (PLAN.md §2.2). Same environment: phpr `9552080b`
(source identical to upstream `9d4ef5ba`), binary sha256 `f690bd4bb4d0346c`, oracle PHP 8.5.7,
Docker `linux/arm64` on an Apple M2 Max, loadavg 0.6–0.9 during the runs.

## 1. Compile cost — `bench/compile-cost.sh`

Median of **30** runs per cell, interleaved, one warm-up round discarded. Wall time measured
around `fork`+`wait4`, CPU and peak RSS from the child's `rusage`. `empty.php` is `<?php` alone.
`big.php` / `big20k.php` are generated declarations (functions and classes with real bodies) that
are **never called**, so everything above the empty-file floor is lex + parse + compile.

| script | engine | wall ms | user ms | sys ms | peak RSS kB | wall min–max ms |
|---|---|---:|---:|---:|---:|---:|
| `empty.php` (1 line) | phpr | 15.82 | 12.90 | 2.99 | 70,386 | 14.77–17.57 |
| | phpr, `PHPR_UNIT_CACHE=0` | 15.43 | 12.90 | 2.99 | 70,378 | 14.61–17.33 |
| | php, no opcache | 6.29 | 4.17 | 2.00 | 24,072 | 5.96–7.75 |
| | php, opcache | 6.47 | 3.87 | 2.74 | 26,056 | 5.99–7.72 |
| | php, opcache + warm file cache | 6.23 | 4.01 | 2.02 | 26,032 | 5.79–7.96 |
| `big.php` (5,003 lines, 141,468 B) | phpr | 22.59 | 19.37 | 3.00 | 90,864 | 21.37–24.07 |
| | phpr, `PHPR_UNIT_CACHE=0` | 22.26 | 19.18 | 3.01 | 90,866 | 21.25–23.76 |
| | php, no opcache | 8.50 | 6.11 | 2.46 | 25,548 | 8.20–9.35 |
| | php, opcache | 10.49 | 7.91 | 2.92 | 29,998 | 10.03–12.21 |
| | php, opcache + warm file cache | 6.78 | 4.58 | 2.50 | 27,688 | 6.54–8.37 |
| `big20k.php` (20,020 lines, 567,986 B) | phpr | 45.54 | 41.28 | 4.03 | 158,464 | 44.66–47.26 |
| | phpr, `PHPR_UNIT_CACHE=0` | 45.58 | 41.48 | 4.03 | 158,468 | 44.67–65.69 |
| | php, no opcache | 14.83 | 11.80 | 2.98 | 32,976 | 14.28–16.31 |
| | php, opcache | 22.56 | 18.43 | 4.00 | 43,052 | 21.71–23.97 |
| | php, opcache + warm file cache | 8.91 | 5.19 | 3.84 | 32,564 | 8.49–10.70 |

Derived (script − `empty.php`, same engine):

| | phpr | php no opcache | php opcache (one-shot) | php opcache, warm |
|---|---:|---:|---:|---:|
| **startup floor** | **15.82 ms** | 6.29 ms | 6.47 ms | 6.23 ms |
| compile 5,003 lines | **6.77 ms** | 2.21 ms | 4.02 ms | 0.55 ms |
| compile 20,020 lines | **29.72 ms** | 8.54 ms | 16.09 ms | 2.68 ms |
| per 1,000 lines (5k / 20k file) | 1.35 / 1.48 ms | 0.44 / 0.43 ms | 0.80 / 0.80 ms | 0.11 / 0.13 ms |
| memory per 1,000 lines (5k / 20k file) | 4,093 / 4,400 kB | 295 / 445 kB | — | — |

- phpr's startup floor is 9.5 ms above the oracle's and 46 MB heavier. phpr compiles its embedded
  PHP prelude (~9,300 lines) in every process; the oracle's internal classes are C.
- phpr compiles at about **one third of the oracle's speed** (3.1–3.5× the time per line) and
  retains **10–14× the memory per line**.
- One-shot CLI opcache is *slower* than no opcache (the optimizer runs, nothing is reused). Only
  the warm file cache shows what a persistent cache gives: 0.11–0.13 ms per 1,000 lines.
- `PHPR_UNIT_CACHE=0` changes nothing here: the unit cache is in-process and a CLI run has one
  process.

## 2. Allocations per loop iteration — `bench/alloc/count.sh`

phpr built with upstream's own `mem-census` feature (separate target dir; its global-allocator
wrapper counts every call). Each variant run at N = 200,000 and N = 400,000; the figure is
Δcount ÷ ΔN, so startup cancels. All results are exact integers. Output of every variant matches
the oracle. The census binary is used for counting only, never for timing.

| loop body | allocs / iter | frees / iter | reallocs / iter | `Zval` clones / iter |
|---|---:|---:|---:|---:|
| empty `for` (global scope) | 0 | 0 | 0 | 0 |
| integer arithmetic (global scope) | 0 | 0 | 0 | 1 |
| integer arithmetic (in a function) | 0 | 0 | 0 | 1 |
| float arithmetic | 0 | 0 | 0 | 0 |
| array element read `$s += $a[$i & 7]` | 0 | 0 | 0 | 2 |
| **array element write `$a[$i & 7] = $s`** (global scope) | **1** | 1 | 0 | 4 |
| **array element write** (in a function) | **1** | 1 | 0 | 4 |
| **string-key write `$a['k1'] = $i`** | **1** | 1 | 0 | 3 |
| array append `$a[] = $i` | 0 | 0 | 0 | 3 |
| **`foreach` over an 8-element array** (per loop entry) | **1** | 1 | 1 | **42** |
| **user function call `$s += add1($i)`** | **1** | 1 | 0 | 2 |
| builtin call `$s += abs($i)` | 0 | 0 | 0 | 1 |
| property write `$o->v = $i` | 0 | 0 | 0 | 4 |
| method call `$o->inc()` | 0 | 0 | 0 | 9 |
| concat + strlen `$x = 'id-' . $i; strlen($x)` | 2 | 2 | 0 | 3 |

The PLAN's reference loop (integer arithmetic + one array write, `bench/alloc/alloc-loop.php`):
1,183,457 allocator calls at N = 1,000,000 and 2,183,457 at N = 2,000,000 →
**exactly 1.000 allocation and 1.000 free per iteration.**

## 3. `.=` with a non-string operand is quadratic — `bench/concat-scaling.php`

One run per engine, times in ms.

| N | result bytes | php `.= $i` | phpr `.= $i` | php `.= (string)$i` | phpr `.= (string)$i` | php `.= 1.5` | phpr `.= 1.5` |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 50,000 | 238,890 | 1.04 | 127.35 | 1.01 | 2.56 | 1.97 | 85.13 |
| 100,000 | 488,890 | 1.92 | 486.35 | 1.90 | 4.75 | 3.76 | 325.23 |
| 200,000 | 1,088,890 | 4.06 | 2,091.17 | 4.20 | 9.65 | 7.93 | 1,273.92 |
| 400,000 | 2,288,890 | 9.00 | 8,930.59 | 8.70 | 19.04 | 15.89 | 5,078.40 |

| growth per doubling of N | php `.= $i` | phpr `.= $i` | phpr `.= (string)$i` |
|---|---:|---:|---:|
| 50k → 100k | 1.85× | 3.82× | 1.86× |
| 100k → 200k | 2.11× | 4.30× | 2.03× |
| 200k → 400k | 2.22× | 4.27× | 1.97× |

At N = 400,000 phpr is **992× slower** than the oracle for `$s .= $i` and 2.2× slower for
`$s .= (string) $i`. The in-place append fast path applies only when the right-hand side is
already a string; an integer or float operand falls back to allocating a new string and copying
the whole accumulated buffer every iteration. In the first dry run of `bench/strings.php`, the
1,000,000-iteration version of this loop took 61.8 s under phpr against 34 ms on the oracle,
which is why the committed benchmark uses 200,000.
