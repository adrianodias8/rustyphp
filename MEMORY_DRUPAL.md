# MEMORY_DRUPAL.md — where one Drupal worker's memory goes

Owner's question (session 10): account for the ~360 MB resident of one warm classic-pool
worker (`ferro -S … --workers 1`, NOTES.md session 9) by allocation site.

Method (`bench/drupal/heap-profile.sh`, run inside the image; heaptrack 1.5 was added to
`docker/Dockerfile`):

- A copy of `/scratch/drupal-base` is warmed first: its first request rebuilds Drupal's caches.
- One classic worker then serves 30 front-page requests and is measured between requests.
- RSS is taken for the shipped build (mimalloc), for the same build with mimalloc's purge
  delay at 0, and for a `--features system-alloc` build (glibc malloc).
- heaptrack runs on the `system-alloc` build, with line tables: mimalloc bypasses malloc, so
  heaptrack cannot see it. It records every allocation still live when the worker is stopped,
  i.e. what the worker keeps from one request to the next. Those bytes are folded by stack and
  bucketed by `bench/drupal/heap-buckets.py`.
- For comparison, `bench/drupal/opcache-size.php` reports what opcache holds after the same
  page.

## The answer

| | MB | notes |
|---|---:|---|
| **RSS, shipped build (mimalloc)** | **294–370** | varies with mimalloc's delayed purge; 370 in session 9 and in one run here, 294 in another |
| RSS, mimalloc with `MIMALLOC_PURGE_DELAY=0` | 283 | freed pages returned at once |
| RSS, `system-alloc` build (glibc) | 262 | |
| **live heap between requests (heaptrack)** | **158.6** (151.2 MiB) | 100 % attributed below |
| per-request peak on top of it | +9.4 | heaptrack peak 160.6 MiB |
| binary text + shared libraries | ~14 | smaps: `ferro` 11 MB, libc and libstdc++ 2.7 MB |
| allocator: fragmentation, partly used pages, thread stack | ~90–110 | RSS − live − text |
| allocator: freed but not yet purged (mimalloc only) | 0–87 | the purge-delay difference |

So of the ~360 MB, about **160 MB is live data**. The rest is the allocator (100–200 MB,
depending on mimalloc's purge timing) and code (14 MB).

### The 151 MiB of live heap, by owner

| owner | MiB | % |
|---|---:|---:|
| **compiled bytecode** (`compile_*`, `bytecode.rs`) | **74.0** | 48.9 |
| **lowered HIR** (parse + lower) | **70.6** | 46.7 |
| unit link / class tables (`run_linked`, seeds) | 3.5 | 2.3 |
| deferred-declaration cache index | 1.9 | 1.2 |
| unit cache bookkeeping | 0.8 | 0.5 |
| include index, preg cache, other | 0.5 | 0.3 |
| interned strings, realpath/stat caches | ~0 | 0 |

The same bytes, by the unit they were built for:

| | include units | deferred declarations | main script |
|---|---:|---:|---:|
| bytecode | 35.9 MiB | 27.1 MiB | 11.0 MiB |
| HIR | 24.6 MiB | 38.9 MiB | 7.2 MiB |

Largest single sites:

- 44.4 MiB in `finish_grow`, i.e. `Vec` growth during lowering and compilation; the
  allocation's owner is the bucket above.
- 18.3 MiB in `compile_program_impl_mode`.
- 18.0 MiB in `bytecode.rs::shrink`: the shrink-to-fit copy of finished op arrays.
- 17.2 MiB in `compile_class`, and 12.5 MiB in `compile_body`.
- 9.3 MiB in `lower_stmts`, and 6.9 MiB in `lower_expr`.

### Why HIR is still alive

A compiled unit should not need its HIR. Three things keep it anyway:

- **`SeedDelta::new_classes: Vec<Rc<hir::ClassDecl>>`** (`vm/mod.rs`). Every cached include
  or deferred-declaration unit keeps the full HIR `ClassDecl` of each class it declares,
  method bodies included. On a cache hit, the next request's VM folds them into its *class
  image*: lowering a later late-bound class (`extends`/`implements` something declared
  elsewhere) runs against that image. That is the 63.5 MiB of HIR under include units and
  deferred declarations.
- **`CachedUnit::main_program`**: the main script's whole lowered `Program`, kept for
  eval-against-image (7.2 MiB).
- Traits are kept lowered (`LoweredTrait`) for their consumers, but measured at 0.05 MiB on
  this page.

Drupal triggers the deferred path for almost every class, because autoloaded classes extend
classes from other files. That is why the "deferred declarations" column is the biggest.

### Against opcache

For the same warm front page, PHP 8.5.7 includes **999 files (5.2 MiB of PHP source)**.
opcache holds them in **15.7 MiB of scripts**, plus interned strings (≤ 18.2 MiB of a 64 MiB
buffer, 37,021 strings). That is ~34 MiB in all, shared by every php-fpm worker. ferro holds
**145 MiB** of HIR + bytecode for the same code, **per worker thread**:

- **4.3× opcache** for one worker, and 4.3 × N for N workers;
- about 28 bytes per byte of source, against opcache's ~6.5.

## After session 11 (method bodies dropped from the seed image)

`ClassDecl::seed_copy` (`hir.rs`) drops method bodies from the copies of a unit's
unconditional classes that go into `SeedDelta` (`vm/mod.rs::seed_delta_of`); conditional classes
keep theirs.

Same profile (`heap-profile.sh`, 30 requests):

| | before | after |
|---|---:|---:|
| live heap between requests | 151.2 MiB | **64.7 MiB** |
| HIR | 70.6 MiB | 17.4 MiB (7.2 of it the main script's `Program`) |
| bytecode | 74.0 MiB | 41.7 MiB |
| per-request peak | +9.4 MiB | +5.9 MiB |
| RSS, mimalloc default | 294–370 MB | **223 MB** |
| RSS, `MIMALLOC_PURGE_DELAY=0` | 283 MB | 212 MB |
| RSS, glibc | 262 MB | 188 MB |
| RSS on a fresh copy (first requests rebuild Drupal's caches) | 583 MB | 411 MB |

The bytecode fell with the HIR, by 40–50 % at every compile site: units compiled bodies from
the seed image that nothing ran. The new copies cost ~7 MiB (`clone` sites). Responses are
unchanged: every gate, the Drupal front pages, the worker-mode battery, Composer and DBAL
(DBAL's peak RSS: 408 → 337 MB).

`MIMALLOC_PURGE_DELAY=0`: −11 MB after this change, but large-buffer work pays for the
purge-and-refault (`str_replace` on 1.3 MB ×1.43, `unserialize` ×1.14, startup ×1.06–1.08); not
adopted.

## Levers, by size (as identified before session 11)

1. **Allocator retention: up to −87 MB RSS per worker, no engine change.** Set mimalloc's
   purge delay to 0 at startup (`mi_option_set(mi_option_purge_delay, 0)`, or the
   environment variable). It needs an A/B on the bench set first: immediate purging trades
   RSS for page faults on reuse.
2. **Seed the class image with signature-only `ClassDecl`s: up to −60 MiB.** Lowering a
   child class against its parent needs the parent's signatures, properties and constants,
   not its method bodies. Bodies make up most of the 63.5 MiB of seed HIR. Trait bodies are
   already kept separately (`LoweredTrait`), so they are unaffected.
3. **Bytecode density: 74 MiB vs opcache's 15.7.** One cause is the `finish_grow` slack in
   the vectors that are not shrunk. The fuller answer is a denser op encoding, which is a
   layout question for DECISION_KERNEL (`run_loop` is layout-sensitive).
4. **Share across workers.**
   - Threads cannot share today: every cache is thread-local, and scaling measured the same on
     threads and processes (NOTES.md session 9).
   - The zygote mode shares it between processes copy-on-write. But refcount writes touch
     the shared pages: about 3.6k minor faults, ~14 MB copied per forked child (9–10k when the
     zygote's snapshot has gone stale; NOTES.md session 10).
   - Either way, sharing pays only once levers 2–3 have shrunk what is shared.

## Reproduce

```bash
docker build -t rustyphp-dev:8.5.7 -f docker/Dockerfile docker     # heaptrack
docker/run.sh bash /work/php-rust/bench/drupal/heap-profile.sh      # N=30 OUT=/scratch/heap
docker/run.sh python3 /work/php-rust/bench/drupal/heap-buckets.py --sites 50 < /scratch/heap/heap-leaked.folded
```

`heap-buckets.py` charges each stack to its innermost frame outside the standard library,
and to the first matching owner rule, scanning from the leaf. The rules are in the script.
`/scratch/heap/heaptrack.gz` opens in `heaptrack --analyze` for anything the buckets do not
answer.
