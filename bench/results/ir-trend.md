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
| (step 2) | property resolution: per-class magic bitmask early exit, slot-indexed presence/read, `declares_private_props` (one lookup when the scope is the object class), pointer-keyed resolve cache for `FieldIsset`, single-lookup unserialize fields, inline `deref_object`/`lazy_prop_access` fast paths, no clone on `PropSetPop`; `memmem` string search; prelude-prefix count on `Module` + prelude name index in `run_linked` | 161.49M | −18.32M (−10.2 %) | 4.16× | 17.49 → 16.25 ms (3.88) |
| (step 3) | include path: bridge an include-in-function scope from the includer's few names (seed prefixes share indices) instead of ~40 scans/include, fresh-cell list keyed by slot index (no name copy); borrowed `memmem` scan in `user_wrapper_url`; debug-only class-name compare gated on `log_enabled!`. Per-process linking not built (see NOTES) | 157.92M | −3.57M (−2.2 %) | 4.07× | 15.24 → 15.04 ms (3.62) |
