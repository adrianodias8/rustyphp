# VM-core spike (step 6b) — killed

Owner's criterion: handle() must drop ≥ 2.5 ms (~15 %) on the interleaved phase bench.
Prototype: local branch `spike/vm-core` (`b354c884`), feature `vm-core`, safe Rust so far
(no unsafe was needed for the prototype; none added).

## What was built

`vm/core.rs`: an inner dispatch loop entered from `run_loop` on 17 hot ops, keeping `ip` and the
op slice in locals across ops, switching frames itself on `MethodCall` (via `methodcall_fast`)
and on the flag-free `Ret`, with verbatim fast paths (`ThisPropGet`/`PropSetPop` IC hits,
loads/stores, compare-jumps, `Sweep`); every other op or case falls back to the unchanged arm.
gate.sh PASS on the vm-core build (3112, 57 repro). One bug found and fixed on the way:
progress must be a flag, not a (depth, ip) comparison — a constant thunk returning into a newly
entered method lands on the same depth and ip.

## Measured

| | base | vm-core |
|---|---:|---:|
| empty loop (ns/iter, 5 ops) | 20.5 | 13.4 |
| two method calls + `$this` write (ns/iter) | 322 | 285 |
| one call + 4 `$this` reads (ns/iter) | 124 | 100 |
| **Drupal handle()** (phases.sh, N=180, interleaved; php-fpm 3.67 ms) | **16.71 ms** | **17.27 ms** |

A first version that entered the inner loop before every op: 16.41 → 16.77 ms.

## Why it cannot reach 2.5 ms (warm Drupal profile, 300 requests, frame-pointer build)

| area | share of a ~19 ms request | optimistic saving of a full rebuild |
|---|---:|---:|
| per-op dispatch at the `run_loop` head (depth check, op fetch, ip store: 18 % of run_loop's 19 % self) | 3.4 % | ~0.45 ms |
| call/return machinery (methodcall_fast 1.8, frame buffers 1.6, gc_note_frame 1.5, recycle_frame 3.0, enter/decay 0.8, Ret arm) | ~9 % | ~0.85 ms |
| property access (field_isset 5.4, prop fallbacks 1.1; IC hits already lean) | ~6.5 % | ~0.5 ms |
| **total** | | **~1.8 ms < 2.5 ms** |

Drupal executes ~450k ops per request across a wide op mix with short straight-line runs, so a
hot-op loop is entered and left constantly; the measured prototype lost more on the transitions
than it saved. Even a complete rebuild of dispatch, frames and property access (the spike's
scope) tops out below the kill line.

## Where the profile points instead (not started; owner's decision)

- value drops: `drop_glue` 8.0 % incl / 6.2 % self, `mi_free` 2.6 % — refcount teardown of
  temporaries and frames;
- builtin calls through the namespace fallback (`CallNsFallback`: 9.9k per request);
- `field_isset` (`isset($o->a['b'])` paths): 5.4 % on its own.
