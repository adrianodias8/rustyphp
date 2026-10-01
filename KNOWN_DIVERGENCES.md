# KNOWN_DIVERGENCES.md — behaviour that differs from PHP 8.5.7, each with a failing test

One entry per divergence found by the fork and not yet fixed. Each has a minimal `.phpt` in
`baseline/divergences/` whose expected output is the oracle's (every file PASSES under php-src's
`run-tests.php` on PHP 8.5.7) and which FAILS on phpr. `baseline/gate.sh` runs the directory and
reports any test that starts passing: fixing a divergence means moving its test to
`baseline/repro/` (where the gate requires a pass) and deleting the row here.

Upstream's own register (`PHPR_DIVERGENCES_FROM_PHP.md` at the tag `upstream-9d4ef5ba`,
principle "correct-or-absent") is not carried here; this file is for what the fork found.

Not listed: performance-only findings (they live in `NOTES.md`), and the two session-2 flags
that did not reproduce — a diagnostic raised while evaluating call arguments reporting the
callee's line (not reproducible with `.=`, `.`, undefined variables, undefined keys or
properties in an argument list), and `Op::StaticPropRef` missing from the include-unit class-id
relocation (the scenario passes; `baseline/repro/static-prop-ref-across-include.phpt` guards it).

| id | what phpr does | what PHP does | test | found |
|---|---|---|---|---|
| D-01 | `echo A::$t;` on an uninitialised typed static property prints nothing | throws `Error: Typed static property A::$t must not be accessed before initialization` | `typed-static-uninit-read.phpt` | session 2 |
| D-02 | `"x" . NAN`, `$s .= NAN` are silent | `Warning: unexpected NAN value was coerced to string` (PHP 8.5) | `nan-to-string-warning.phpt` | session 2 |
| D-03 | `function strlen() {}` compiles and shadows the builtin | `Fatal error: Cannot redeclare function strlen()` | `redeclare-builtin-fatal.phpt` (runner: "compile-time diagnostic not modelled") | session 2 |
| D-04 | `strlen()` with the wrong argument count throws `Error` | throws `ArgumentCountError` | `builtin-arity-argumentcounterror.phpt` | session 4 |
| D-05 | `function assert()` inside a namespace is accepted | `Fatal error: Defining a custom assert() function is not allowed` | `assert-declared-in-namespace.phpt` (runner: skip, as D-03) | session 4 |
| D-06 | the `Deprecated: Implicit conversion from float 1.5 to int` of a typed argument reports the call line | reports the callee's line (Zend's `RECV` runs in the callee) | `coercion-deprecation-line.phpt` | session 4 |
| D-07 | an argument TypeError for an object says `object given` | names the class: `C given` | `typeerror-class-name-given.phpt` | session 4 |
| D-08 | `called in %s` of a TypeError raised from a strict included file names the main script | names the included file | `typeerror-called-in-included-file.phpt` | session 4 |
| D-09 | `$n["s"]["t"] .= "u"` on `$n = null` is silent | `Warning: Undefined array key "s"`, then `"t"` | `concat-assign-nested-vivify-warning.phpt` | session 4 |
| D-10 | `$o["k"] .= "x"` / `$o["n"]++` on an `ArrayAccess` object call `offsetGet` then `offsetSet` and modify the value | `Notice: Indirect modification of overloaded element of AA has no effect`, `offsetGet` only, nothing stored | `arrayaccess-indirect-modification.phpt` | session 4 |
| D-11 | `$a[[]] = 1` throws `TypeError: Illegal offset type` | `Cannot access offset of type array on array` (read: `… on array`) | `illegal-offset-message.phpt` | session 4 |
| D-12 | `$t[null] = 4` is silent | `Deprecated: Using null as an array offset is deprecated, use an empty string instead` (PHP 8.5) | `null-array-offset-deprecation.phpt` | session 4 |
| D-13 | `PHP_OS` is `Darwin` on every platform (compile-time constant) | the running platform (`Linux` in the container) | `php-os-constant.phpt` | session 1 |
| D-14 | `ini_set('precision', '5')` returns `false` and has no effect (`ini_get` and float rendering keep 14; upstream's `ini.rs` marks engine-hardwired directives read-only) | takes effect: `14\|14\|5\|0.33333` | `ini-set-precision.phpt` | session 5 (worker isolation battery) |

Fixed by the fork (tests in `baseline/repro/`): `count()` on a `Countable` through any dynamic
call; `isset(Class::$static)`; `.=` quadratic for every non-local target and non-string operand;
`.=` never calling `__toString()`; a namespaced call rebinding after a later declaration; host,
by-reference and prelude builtins unshadowable inside a namespace; value-registry builtin names
case-sensitive; `ReflectionFunction::getName()` case of an internal function.
