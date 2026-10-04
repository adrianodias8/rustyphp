# Instructions per warm Drupal request — trend

Primary metric of the engine program (owner, 2026-10-04): cachegrind
instructions (Ir) per warm Drupal front-page request, ferro classic
(`ferro -S`, fresh Vm per request), 20 measured requests minus start-up and
warm-up (`SIM=0 bench/drupal/cachegrind.sh`, HWCOUNTERS_DRUPAL.md §1). php
8.5.7 + opcache on the same harness: **38.8M**. Repeat runs of one binary
agree within 0.02 %. Secondary: Drupal `handle()` ms, `bench/drupal/phases.sh`
interleaved (FERRO = before, FERRO_B = after, php-fpm in the same run).

| commit | change | Ir/request | Δ vs before | ratio to php | handle() before → after (php-fpm) |
|---|---|---:|---:|---:|---|
| fd14a8c3 | baseline (HWCOUNTERS_DRUPAL.md) | 180.74M | — | 4.66× | 17.67 ms (3.79) |
| (step 1) | Zval drop: inline scalar test (`zdrop`/`zset`) at the hot placeholder drops (call binding, ns-fallback args, slot stores after `gc_note`, `Ret`), `unwrap_or_else(Null)`, `resize_with(Undef)` | 179.81M | −0.93M (−0.5 %) | 4.63× | 18.04 → 17.96 ms (3.80) |
