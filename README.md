# Ferrophant

**A PHP 8.5 runtime written in Rust — measured against the real thing, and
vibe-coded end to end.**

```bash
ferro script.php                         # a drop-in for `php` on the CLI
ferro -S 0.0.0.0:8080 --worker app.php   # boot once, serve requests in a loop
```

Ferrophant is a fork of [francescotinti/php-rust](https://github.com/francescotinti/php-rust)
(`phpr`), a from-scratch reimplementation of PHP 8.5 in safe Rust. Upstream's
work is the foundation and stays credited (see [NOTICE.md](NOTICE.md) and
the [license](#license)). This fork changes three things: the performance
work is decided by a profiler, every claim is a number in a committed file,
and the runtime has a worker mode aimed at Symfony, Laravel and Drupal.

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

## Status (2026-10-01)

| what | number | where |
|---|---:|---|
| `Zend/tests` + `tests/` passing (of 6,172) | 3,048 | `baseline/zend-tests.md` |
| operator differential vs `php` | 37,835 cases, 0 mismatches | `cargo test -p php-types --test differential` |
| Composer 2.10 `require monolog/monolog` | runs; `vendor/` byte-identical | `baseline/smoke-composer.sh` |
| Doctrine DBAL 4.5 PHPUnit suite | 4,146 tests, 10 errors, 1 failure (oracle: 1 failure) | `baseline/smoke-dbal.sh` |
| Symfony HttpKernel request, in-process | 7.9× the time of `php -n` | `bench/results/2026-09-30-slices-2-3.md` |
| Symfony under `wrk`, worker mode, 4 workers | **1.08× nginx+php-fpm+opcache**, **0.63× FrankenPHP worker** | `bench/results/2026-09-30-wrk-w4.md` |
| same, 8 workers | 1.27× php-fpm, 1.13× FrankenPHP | `bench/results/2026-09-30-wrk-w8.md` |
| hello world under `wrk`, 4 workers | 164 k req/s (4.6× php-fpm, 6.7× FrankenPHP) | same |
| allocations per array write / typed call / `foreach` | 0 / 0 / 0 (were 1 / 1 / 1) | `bench/alloc/count.sh` |

The interpreter is still several times slower than Zend on CPU-bound PHP
(3.5–10× on the micro benchmarks; the dispatch loop is what is left, see
[DECISION_KERNEL.md](DECISION_KERNEL.md)). Behind HTTP, where most of a
request is SAPI work, the worker mode already competes.

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
                     php-cli -> `ferro`, phpt-runner, php-server)
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

1. Slice 6, the dispatch loop (34 % of a Symfony request) — on bare metal
   with hardware counters, not in the VM.
2. An on-disk bytecode cache for the CLI (the 15.8 ms startup floor).
3. Worker recycling (`max_requests`, memory ceiling) and a long-run leak test.
4. Laravel and Drupal under the worker mode.

## License

Ferrophant is distributed under the **PHP License, version 3.01**, the
license of the upstream project it derives from — see [LICENSE](LICENSE)
and [NOTICE.md](NOTICE.md). Upstream copyright: Francesco Tinti, 2026. The
name "Ferrophant" (ferrum + elephant) was chosen so that "PHP" does not
appear in the product's name, as clause 4 of that license requires of
derived products. Ferrophant is not endorsed by the PHP Group.
