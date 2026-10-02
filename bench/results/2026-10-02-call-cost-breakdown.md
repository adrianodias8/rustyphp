# Call-cost breakdown — ferro vs PHP 8.5.7 — 2026-10-02T11:24:48Z

ferro df27aec6 (rebased onto upstream main). PHP: oracle 8.5.7 CLI, opcache on, JIT off, optimizer off (`opcache.optimization_level=0`) for the unit costs — with the optimizer on, empty calls, `strlen` of a known string and constant concatenation fold away (0 ns), which Drupal's cross-file calls do not.

## 1. Unit costs (`bench/calls/callcost.php`, ns per operation net of the empty loop, best of 3 × 2M)

```
loop (baseline)            php    1.98  ferro    8.94    4.5x
call f()                   php    5.47  ferro   52.02    9.5x
call f($a)                 php    5.60  ferro   62.59   11.2x
call f($a,$b,$c)           php    8.14  ferro   82.74   10.2x
call f(int x3): void       php   10.43  ferro  136.13   13.1x
call f() used return       php    6.87  ferro   52.29    7.6x
call f($a) 2 defaults      php   17.63  ferro  113.66    6.4x
call f(&$a)                php    7.12  ferro   83.42   11.7x
call f(...$a) variadic     php   20.80  ferro  129.23    6.2x
method $o->m()             php    8.16  ferro   69.65    8.5x
method $o->m($a,$b,$c)     php   12.47  ferro  119.90    9.6x
getter $o->getP()          php   11.65  ferro   74.65    6.4x
static C::s()              php    6.10  ferro   68.43   11.2x
closure $c()               php    9.30  ferro   84.48    9.1x
builtin strlen($s)         php    1.80  ferro   94.25   52.4x
builtin count($a)          php    1.96  ferro   47.87   24.4x
recursion depth 10/10      php    9.21  ferro   54.68    5.9x
prop read $o->p            php    2.60  ferro   18.11    7.0x
prop write $o->p = 1       php    2.80  ferro   52.28   18.7x
typed prop write $o->ti    php    4.98  ferro   73.94   14.8x
prop chain $o->child->p    php    4.80  ferro   34.15    7.1x
isset($o->p)               php    4.75  ferro   30.15    6.3x
isset($o->child->p)        php    7.21  ferro  134.46   18.6x
array read $a['k']         php    3.57  ferro   34.92    9.8x
array read $a[3]           php    3.47  ferro   31.57    9.1x
array write $a['k']=1      php    4.52  ferro   47.81   10.6x
isset($a['k'])             php    3.26  ferro   43.69   13.4x
$a['k'] ?? 0               php    6.02  ferro   40.39    6.7x
foreach, per element       php    1.06  ferro    7.99    7.5x
string concat $s.$t        php    9.58  ferro   66.48    6.9x
```

## 2. One warm front page, by kind of work (`bench/calls/breakdown.py` on `data/census-2026-10-02.txt`)

```
category                                                    ferro ms    count   php ms   gap ms  note
user calls (setup + return)                                     4.35    17047     0.13     4.22  8262 method, 1188 static, 786 ctor, 868 dynamic, ~5943 plain/unit returns
everything else (loads, stores, jumps, compares, objects, ...)      2.49   276251     0.28     2.21  priced at 1 ns per op
include / class declaration                                     2.20     1062        ?        ?  PHP side not measured (opcache include + early binding)
builtin work: PDO / SQLite                                      1.90       72        ?        ?  PHP side not measured (same SQLite library)
builtin work: all other builtins                                1.54    10605        ?        ?  PHP side not measured
property isset/empty                                            1.22     7540     0.05     1.16  priced as isset($o->a->b)
property writes                                                 1.11     5139     0.01     1.10  
builtin work: file_exists                                       0.90      668     0.19     0.71  ferro 1394 ns/call in the request vs php 280 ns (micro)
builtin work: unserialize                                       1.37      270     0.73     0.64  ferro/php 1.88 on the site's cache rows
builtin calls: call machinery                                   0.57    11615     0.02     0.55  49 ns vs 2.0 ns per call (trivial builtin)
property reads                                                  0.52    14130     0.04     0.49  
nested paths ($a[..][..], $o->a[..])                            0.47     8809     0.04     0.44  
foreach                                                         0.29    11347     0.01     0.28  
string concat / interpolation                                   0.29    11314     0.11     0.18  
array reads                                                     0.17     7275     0.03     0.15  
property unset                                                  0.15      592     0.00     0.15  
array writes / literals                                         0.14     6595     0.03     0.11  
total (census, whole request)                                  19.69
gap explained by priced categories: 12.38 ms
```

## 3. Reading

- **The census** is the per-op net time of one warm front page (405k ops, 19.7 ms over the whole
  request). It is net of a 29 ns/op clock floor, so the categories are approximate (±10–20 %
  run to run).
- **PHP's side** is count × the unit cost above. These are hot-loop prices, so each priced gap
  is an upper bound.
- **Unmeasured categories** (includes and declarations, PDO/SQLite, other builtins' own work)
  are bounded by subtraction. PHP's whole request is 5.34 ms (`phases.sh`, same day), of which
  1.67 ms is priced above, so these cost PHP ≤ 3.7 ms against ferro's 5.64 ms: a gap of
  ≈ 2.0 ms.
- **Cross-check:** 12.4 (priced) + ≈ 2.0 = ≈ 14.4 ms, against the measured whole-request gap of
  15.0 ms (20.36 vs 5.34 ms).

Against the 13.9 ms `handle()` gap (17.80 vs 3.92 ms):

| | gap | share |
|---|---:|---:|
| user calls (setup + return, 17k calls ≈ 255 ns each in the request vs ≈ 8 ns) | 4.2 ms | ~30 % |
| property access (isset 1.2, writes 1.1, reads 0.5, unset 0.15) | 2.9 ms | ~21 % |
| everything else (276k simple ops ≈ 9 ns each vs ≈ 1 ns) | 2.2 ms | ~16 % |
| builtin call machinery (11.6k × ≈ 49 ns) | 0.55 ms | 4 % |
| `file_exists` (668 Composer PSR-4 probes: 1.4 µs each in the request vs 0.28 µs) | 0.7 ms | 5 % |
| `unserialize` (×1.88 on the site's own cache rows) | 0.6 ms | 5 % |
| arrays, nested paths, foreach | 1.0 ms | 7 % |
| includes/declarations, PDO, other builtins (by subtraction) | ≈ 2.0 ms | ~14 % |

Calls are the largest single item, but explain under a third. No single mechanism is most of
it: every VM operation is 6–19× PHP's in the micro-benchmarks. The in-request call is about 4×
its micro-benchmark price (255 vs 52–70 ns). Drupal's calls carry arguments, type coercion
(3,991 `CoerceParams`), defaults and by-ref binding, and run cold in instruction and data
caches; the Ret path's notes (drop-mode possible roots) also add.
