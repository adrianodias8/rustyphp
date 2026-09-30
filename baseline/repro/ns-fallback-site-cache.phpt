--TEST--
Namespaced two-step function lookup: per-site cache invalidation (fork slice 3)
--DESCRIPTION--
An unqualified call inside a namespace (`count($a)` in `namespace App`) is
resolved at run time: `App\count` first, then global `count`. The fork binds
that resolution per call site (Op::CallNsFallback, NsIc). A bound site must
keep the pre-call semantics of the builtin (Countable dispatch), an undefined
function must never be bound (a later declaration binds), and a function
registered by a conditional declaration, eval or include must bind at sites
that have not run yet. Every expected line is oracle output (PHP 8.5.7).
Sites that HAVE run keep their binding: see ns-shadow-builtins.phpt.
--FILE--
<?php
namespace App;
class C implements \Countable { function count(): int { return 42; } }
function probe($v) { return count($v); }
$a = [1, 2, 3];
echo "probe: ";
for ($i = 0; $i < 3; $i++) echo probe($a), " ";
echo probe(new C), "\n";
function late() { return late_fn(1); }
for ($i = 0; $i < 2; $i++) {
    try { late(); } catch (\Error $e) { echo "late: ", $e->getMessage(), "\n"; }
}
eval('namespace App; function late_fn($s) { return "late:" . $s; }');
echo late(), " ", late(), "\n";
function later2() { return ucfirst("x"); }
eval('namespace App; function ucfirst($s) { return "shadowed:" . $s; }');
echo "later2: ", later2(), "\n";
if (true) { function cond() { return "cond"; } }
function callcond() { return cond(); }
echo "cond: ", callcond(), "\n";
$tmp = sys_get_temp_dir() . "/nsic_" . getmypid() . ".php";
file_put_contents($tmp, '<?php namespace App; function inc_f() { return implode(",", [count([1, 2]), strtoupper("b")]); } echo "inc: ", inc_f(), "\n";');
include $tmp;
echo "inc again: ", inc_f(), "\n";
unlink($tmp);
try { undefined_fn_xyz(); } catch (\Error $e) { echo get_class($e), ": ", $e->getMessage(), "\n"; }
try { undefined_fn_xyz(); } catch (\Error $e) { echo $e->getMessage(), "\n"; }
function dyn() { $x = "abc"; return str_replace("b", "B", $x); }
echo "dyn: ", dyn(), dyn(), "\n";
?>
--EXPECT--
probe: 3 3 3 42
late: Call to undefined function App\late_fn()
late: Call to undefined function App\late_fn()
late:1 late:1
later2: shadowed:x
cond: cond
inc: 2,B
inc again: 2,B
Error: Call to undefined function App\undefined_fn_xyz()
Call to undefined function App\undefined_fn_xyz()
dyn: aBcaBc
