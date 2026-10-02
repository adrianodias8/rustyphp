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
| D-03 | `function strlen() {}` compiles and shadows the builtin | `Fatal error: Cannot redeclare function strlen()` | `redeclare-builtin-fatal.phpt` (runner: "compile-time diagnostic not modelled") | session 2 |
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
| D-15 | an uncaught `TypeError` raised by a builtin (`sort(NULL)`) has no frame for the builtin in its trace (`#0 {main}`) | `#0 file(2): sort(NULL)` then `#1 {main}` | `builtin-frame-in-trace.phpt` | session 6 |
| D-16 | float-offset diagnostics on an array write (`$b[1.5] = 3`, `[NAN => 1]`) are reported with the line of the NEXT statement, after its output; `[$k => 1]` with `$k = 1e30` misses the not-representable warning; `isset($a[NAN])` is silent (upstream behaves the same) | reported on the write's own line; every out-of-range float offset warns | `nan-array-key-warning.phpt` | session 6 |
| D-17 | XPath `namespace::` axis evaluates to an empty node-set (the engine has no namespace nodes) | the in-scope namespace nodes, `xml` included | `xpath-namespace-axis.phpt` | session 7 |
| D-18 | a builtin's TypeError names a `false` argument `bool given` | `false given` (zend_zval_value_name) | `false-given-type-name.phpt` | session 7 |
| D-19 | a TypeError for a user callback invoked from prelude code (an SPL iterator) appends `, called in <user file> on line <prelude line>` | no `called in` clause: the caller is internal | `internal-caller-no-called-in.phpt` | session 7 |
| D-20 | `get_defined_constants()` is undefined | returns every constant (`categorize` groups them) | `get-defined-constants.phpt` | session 7 |
| D-21 | `exp("abc")` / `sqrt([])`: the TypeError names the parameter type `int\|float` | `float` (the declared type of the math builtins) | `math-param-type-name.phpt` | session 7 |
| D-22 | `preg_match_all()` ignores its `$offset` argument (matches from 0) | matching starts at the byte offset | `preg-match-all-offset.phpt` | session 7 |
| D-23 | `f(...$args)` where the KNOWN function `f` has by-reference parameters is a compile error ("spread call to a by-reference function") | binds the reference elements (dynamic `$f(...$args)` already does) | `spread-to-known-byref-function.phpt` | session 7 |
| D-24 | a returning function's locals (and `foreach` temporaries) are destructed at the caller's next statement boundary, after the caller used the return value: `echo f();` prints `r[local]` | inside the return, before the value is used: `[local]r` | `destructor-at-return.phpt` | session 9 (both GC engines; the frame's teardown ORDER already matches) |
| D-25 | unsetting a suspended generator destructs what its frames hold at the statement boundary, outer frame first: `G[d2][d1]` | at the unset, inner (`yield from`) generator first: `[d1][d2]G` | `generator-teardown-destructors.phpt` | session 9 |
| D-26 | freeing a container destructs an element's own properties after the container's later elements: `[W(7, o: W(0)), W(9)]` prints `7 9 0` (drop-mode destructors run from a FIFO queue) | depth first, as refcounts reach zero: `7 0 9` | `nested-destructor-order.phpt` | session 12 |
| D-27 | `unserialize()` sets fields without the class's checks: a typed property takes any value, a dynamic property is created silently, a corrupt mangled name (`"\0P-c"`) is accepted | `TypeError: Cannot assign string to property T::$i of type int`; `Deprecated: Creation of dynamic property`; `Notice: Corrupt member variable name` and failure | `unserialize-property-checks.phpt` | session 12 |
| D-28 | `unserialize()` ignores its `$options` (`allowed_classes`, `max_depth`; the default depth limit, 4096, holds) | objects of other classes become `__PHP_Incomplete_Class`; `max_depth` warns and fails | `unserialize-options.phpt` | session 12 |
| D-29 | the "implements the Serializable interface" deprecation is raised before the script runs, with the line of its first statement | when the declaration is reached, with the declaration's line | `serializable-deprecation-line.phpt` | session 12 |

Fixed by the fork (tests in `baseline/repro/`): `count()` on a `Countable` through any dynamic
call; `isset(Class::$static)`; `.=` quadratic for every non-local target and non-string operand;
`.=` never calling `__toString()`; a namespaced call rebinding after a later declaration; host,
by-reference and prelude builtins unshadowable inside a namespace; value-registry builtin names
case-sensitive; `ReflectionFunction::getName()` case of an internal function; enum cases
serialized as `O:` objects and `E:` rejected by `unserialize()` (`unserialize-enum.phpt`, session
10); `unserialize()`'s failure behaviour: error offset always 0, trailing data rejected instead of
warned, no autoload / delayed `__wakeup` / `__unserialize` before a malformed byte, overflowing
`i:` rejected, `S:` unsupported, abstract / interface / trait classes instantiated
(`unserialize-failure-semantics.phpt`, session 12); `var_export(PHP_INT_MIN)`
(`var-export-int-min.phpt`); the cli-server's router run reporting the router in `SCRIPT_FILENAME`/`SCRIPT_NAME` and
not walking a directory without an index back to its parent's (`baseline/cli-server-router.sh`).
