# Ferrophant

[![CI](https://github.com/adrianodias8/rustyphp/actions/workflows/ci.yml/badge.svg?branch=next)](https://github.com/adrianodias8/rustyphp/actions/workflows/ci.yml)

**A PHP 8.5 runtime written in Rust — measured against the real thing, and
vibe-coded end to end.**

```bash
ferro script.php                                    # a drop-in for `php` on the CLI
ferro -S 0.0.0.0:8080 -t web web/index.php --workers 8   # php-fpm-like pool: a fresh VM per request
ferro -S 0.0.0.0:8080 --worker app.php              # boot once, serve requests in a loop
ferro -S 0.0.0.0:8080 --worker boot.php --zygote --workers 8   # boot once, fork a child per request
ferro-edge --upstream 127.0.0.1:8080                # cache-tag aware reverse proxy (Purge BAN, SWR, coalescing)
```

Ferrophant is a fork of [francescotinti/php-rust](https://github.com/francescotinti/php-rust)
(`phpr`), a from-scratch reimplementation of PHP 8.5 in safe Rust. Upstream's
work is the foundation and stays credited (see [NOTICE.md](NOTICE.md) and
the [license](#license)). This fork changes three things: the performance
work is decided by a profiler, every claim is a number in a committed file,
and the runtime serves HTTP itself — a php-fpm-like pool (a fresh VM per
request) and a worker mode — aimed at Symfony, Laravel and Drupal.

## Why this exists

In his Rails World 2026 keynote, DHH argued that the release of Opus 4.5 on
24 November 2025 was software's "Brownie camera moment": the point where
writing code by hand stops being the economical way to make software, where
languages that are hostile to humans but fine for agents — Rust first among
them — become the natural choice for backends, and where "English is the
programming language". He described rewriting his own backends in Rust with
agents and serving peak traffic from a single Raspberry Pi.

That talk is why this fork exists: I got inspired by DHH to try vibe coding
a serious system in Rust, and picked the language I know best as the target.
This repository is that thesis applied to PHP. **Not one line of Rust in the
fork's commits was typed by a person.** The work — profiling, the decision
document, five performance slices, the worker mode, the benchmark harness,
the tests, this README — was done by Claude (Fable 5.1 and Opus 5.5 in
Claude Code) directed in English, one session at a time. The human wrote
the plan, chose the name, and read the numbers. Upstream was built the same
way by its author, in Italian; this fork translated the parts it kept.

What keeps a vibe-coded runtime honest is the oracle: the real `php` 8.5.7
binary and its 21,548 `.phpt` tests. Every change lands with a regression
gate that must show zero pass→fail on that corpus, a differential of 37,835
operator cases at zero mismatches, and an interleaved A/B benchmark against
the previous build. Nothing is "believed to be faster".

## Status (2026-10-02)

| what | number | where |
|---|---:|---|
| `Zend/tests` + `tests/` passing (of 6,172) | 3,111 | `baseline/zend-tests.md` |
| operator differential vs `php` | 37,835 cases, 0 mismatches | `cargo test -p php-types --test differential` |
| Composer 2.10 `require monolog/monolog` | runs; `vendor/` byte-identical | `baseline/smoke-composer.sh` |
| Doctrine DBAL 4.5 PHPUnit suite | 4,146 tests, 1 failure (the oracle's same 1) | `baseline/smoke-dbal.sh` |
| Drupal 11 `drush site:install standard` (SQLite) | completes; database equivalent to the oracle's | `bench/drupal/install.sh`, `MISSING_FOR_DRUPAL.md` |
| Drupal 11 front page, one-shot and worker mode | byte-identical to `php -S` (worker: 10/10 with the documented reset) | `bench/drupal/frontpage.sh`, `worker-leaks.sh` |
| Drupal 11 front page, warm, one request (classic) | 19 ms (was 166 ms; `php -S` without opcache: 8–10 ms) | `bench/drupal/oneshot-time.sh`, NOTES.md sessions 8–9 |
| Drupal under `wrk`, 8 workers, classic pool | **294 req/s** vs nginx+php-fpm+opcache 1,425 (0.21×) | `bench/results/2026-10-01-drupal-wrk-classic4-w8.md` |
| Drupal under `wrk`, 8 workers, zygote mode (boot once, fork per request) | 289 req/s, 1.05× the classic pool in the same run | `bench/results/2026-10-02-drupal-wrk-zygote-w8.md` |
| Drupal behind `ferro-edge` (page max-age 3600 s), 4 origin workers | ~246 k req/s cached; BAN `node_list` every 100 ms: 145–225 k req/s, 9.3 origin req/s | `bench/results/2026-10-02-edge-wrk-w4.md` |
| one warm Drupal worker's memory | 65 MiB live (bytecode 42, HIR 17; was 151); RSS 223 MB (was 294–370); 1.6 GB for 8 workers; opcache: ~34 MiB shared | `MEMORY_DRUPAL.md` |
| Drupal under `wrk`, 8 workers, worker mode | 141 req/s vs FrankenPHP worker 340, php-fpm 1,425 (both workers degrade run to run: Drupal state) | `bench/results/2026-10-02-drupal-wrk-worker-w8.md` |
| Symfony HttpKernel request, in-process | 7.9× the time of `php -n` | `bench/results/2026-09-30-slices-2-3.md` |
| Symfony under `wrk`, worker mode, 4 workers | **1.08× nginx+php-fpm+opcache**, **0.63× FrankenPHP worker** | `bench/results/2026-09-30-wrk-w4.md` |
| same, 8 workers | 1.27× php-fpm, 1.13× FrankenPHP | `bench/results/2026-09-30-wrk-w8.md` |
| hello world under `wrk`, 4 workers | 164 k req/s (4.6× php-fpm, 6.7× FrankenPHP) | same |
| allocations per array write / typed call / `foreach` | 0 / 0 / 0 (were 1 / 1 / 1) | `bench/alloc/count.sh` |

The interpreter is still several times slower than Zend on CPU-bound PHP:
calls and property writes 7–19×, arrays and strings 1.4–5× on the micro
benchmarks, and a Drupal request ~5× php-fpm+opcache per worker. Compilation
is no longer in that number (every unit, deferred class and include is cached
across requests); what is left is spread over dispatch, property access,
value copies and allocation — see [DECISION_KERNEL.md](DECISION_KERNEL.md)
and NOTES.md session 8. Behind HTTP on a framework that resets cheaply
(Symfony), the worker mode already competes; on Drupal it does not yet.

Known behaviour differences from PHP 8.5.7 are listed one per row, each
with a failing test, in [KNOWN_DIVERGENCES.md](KNOWN_DIVERGENCES.md).

## Try it

Nothing is installed on the host; everything runs in one Docker image that
holds the oracle PHP 8.5.7, the pinned Rust toolchain and the measurement
tools. You need Docker and a checkout of
[php-src](https://github.com/php/php-src) at tag `php-8.5.7` next to this
repository (`../php-src`) for the corpus and the benchmarks.

```bash
docker build -t rustyphp-dev:8.5.7 -f docker/Dockerfile docker
docker/run.sh cargo build --release -p php-cli          # -> /target/release/ferro (volume)
docker/run.sh /target/release/ferro -r 'echo PHP_VERSION, "\n";'

# the gates
docker/run.sh cargo test --release
docker/run.sh ../baseline/gate.sh                       # zero pass->fail on the corpus

# classic pool: N threads, a fresh VM per request (add --reuse-port to run several processes)
docker/run.sh /target/release/ferro -S 0.0.0.0:8080 -t public public/index.php --workers 4

# worker mode: boot the app once per worker, then loop
docker/run.sh /target/release/ferro -S 0.0.0.0:8080 --worker bench/worker/symfony-worker.php --workers 4
```

A worker script is a loop around one builtin; the same file runs under
FrankenPHP unchanged:

```php
<?php
$kernel = build_kernel(__DIR__);                     // boot once
$handle = function_exists('frankenphp_handle_request')
    ? 'frankenphp_handle_request' : 'ferro_handle_request';
while ($handle(static function () use ($kernel) {
    $request  = Request::createFromGlobals();
    $response = $kernel->handle($request);
    $response->send();
    $kernel->terminate($request, $response);
})) {
    gc_collect_cycles();
}
```

Globals, statics, static properties and boot-time objects persist across
requests; output, headers, superglobals, handlers and ini values are reset
per request. `bench/worker/isolation.sh` is the gate for that contract.
`--max-requests N` recycles a worker onto a fresh VM after N requests, like
php-fpm's `pm.max_requests`; `bench/worker/soak.sh` is the long-run leak
test (RSS sampled under `wrk`).

## How the work is done

- **Measure first.** [PROFILE.md](PROFILE.md) is the profile of the
  unmodified engine (flame graphs in `bench/profiles/`); every slice in
  [DECISION_KERNEL.md](DECISION_KERNEL.md) §6 names the bucket it attacks
  and the number it moved. [NOTES.md](NOTES.md) is the session log — every
  measurement, in order, including the ones that went wrong.
- **The oracle is always right.** A `.phpt` in `baseline/repro/` is written
  for every fix, validated on `php` with php-src's own `run-tests.php`, and
  must pass on Ferrophant; `baseline/divergences/` holds the ones that do
  not, yet.
- **No slowdown on any benchmark.** `bench/ab.sh` runs the whole set
  interleaved (ABBA, 7 rounds) between the previous and the new binary; a
  section above 1.05 is investigated, not waved through — which is how the
  ±12 % heap-placement artefact of the `oop` benchmark was found
  (`bench/path-sensitivity.sh`).
- **Agents, not chat.** Each session is a written brief; the agent runs the
  gates itself and reports numbers. See [CLAUDE.md](CLAUDE.md) for the rules
  an agent works under here.

## Layout

```
php-rust/            the Cargo workspace (crates: php-types, php-runtime, php-builtins,
                     php-cli -> `ferro`, phpt-runner, php-server, ferro-edge)
baseline/            corpus gate, repro and divergence .phpt files, smoke tests
bench/               benchmark set, A/B and profiling scripts, worker-mode harness, results
docker/              the one image everything runs in
ARCHITECTURE_NOTES.md  how the engine is built (value model, VM, unit cache)
PROFILE.md             where the time went before any change
DECISION_KERNEL.md     the kernel decision and the ordered slices
KNOWN_DIVERGENCES.md   what still differs from PHP, with a failing test each
NOTES.md               the session log
```

## Roadmap

1. The call/frame layout: a contiguous value stack with the frame state in
   locals — the safe-Rust prototype (`bench/proto/`) halves a call; a
   multi-site refactor of the VM.
2. The per-worker working set (`MEMORY_DRUPAL.md`: 65 MiB live after
   dropping method bodies from the seed image, 223 MB resident): bytecode
   density next (42 MiB vs opcache's ~16 for the same files).
3. Destructor timing at return and generator teardown (D-24, D-25).
4. Zygote mode: overlap a zygote's re-boot with its replacement (a single
   zygote has a ~2 s gap), and cut the copy-on-write faults refcount writes
   cause in children. Drupal's own worker-mode growth is an upstream issue
   (`MISSING_FOR_DRUPAL.md`).
5. An on-disk bytecode cache for the CLI (the 15.8 ms startup floor), and
   a bare-metal run with hardware counters.

## License

Ferrophant is distributed under the **PHP License, version 3.01**, the
license of the upstream project it derives from — see [LICENSE](LICENSE)
and [NOTICE.md](NOTICE.md). Upstream copyright: Francesco Tinti, 2026. The
name "Ferrophant" (ferrum + elephant) was chosen so that "PHP" does not
appear in the product's name, as clause 4 of that license requires of
derived products. Ferrophant is not endorsed by the PHP Group.
