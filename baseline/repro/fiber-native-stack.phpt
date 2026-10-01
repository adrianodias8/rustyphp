--TEST--
Fiber::suspend across generators, callbacks and nested calls; resume at another depth; throw(); nested fibers; traces through fibers
--FILE--
<?php
function gen() { echo "gen start\n"; $x = Fiber::suspend("from-gen"); echo "gen resumed with $x\n"; yield 1; yield 2; }
$f = new Fiber(function () {
    foreach (gen() as $v) { echo "v=$v\n"; }
    return "done";
});
$s = $f->start();
echo "suspended: $s\n";
$f->resume("R");
echo "fiber returned: ", $f->getReturn(), "\n";
// exception thrown inside a generator inside a fiber, caught in the fiber
$g = new Fiber(function () {
    $gen = (function () { yield 1; throw new RuntimeException("boom"); })();
    try { foreach ($gen as $v) { echo "g=$v\n"; } } catch (RuntimeException $e) { echo "caught ", $e->getMessage(), "\n"; }
});
$g->start();
// resume from a different call depth than start
function starter($f) { return $f->start(1); }
function deep($f, $n) { return $n === 0 ? $f->resume("deep") : deep($f, $n - 1); }
$f = new Fiber(function ($a) {
    $x = Fiber::suspend("s1:$a");
    echo "got $x\n";
    $cb = array_map(function ($v) { return Fiber::suspend("in-map-$v"); }, [1, 2]);
    echo "map=", implode(",", $cb), "\n";
    try { Fiber::suspend("before-throw"); } catch (LogicException $e) { echo "caught in fiber: ", $e->getMessage(), "\n"; }
    $inner = new Fiber(function () { echo "inner sees ", Fiber::getCurrent() !== null ? "a fiber" : "none", "\n"; return Fiber::suspend("inner-s") . "!"; });
    echo "inner start -> ", $inner->start(), "\n";
    $inner->resume("ir");
    echo "inner ret ", $inner->getReturn(), "\n";
    return "end";
});
echo starter($f), "\n";
echo deep($f, 5), "\n";
echo $f->resume("m1"), "\n";
echo $f->resume("m2"), "\n";
var_dump($f->throw(new LogicException("thrown in")));
var_dump($f->isTerminated(), $f->getReturn());
$g = new Fiber(function () { Fiber::suspend(1); throw new RuntimeException("escapes"); });
$g->start();
try { $g->resume(); } catch (RuntimeException $e) { echo "escaped: ", $e->getMessage(), "\n"; }
var_dump($g->isTerminated());
function inner() { throw new RuntimeException("x"); }
function body() { Fiber::suspend(1); inner(); }
function outerResume($f) { $f->resume(); }
$f = new Fiber(function () { body(); });
$f->start();
try { outerResume($f); } catch (Exception $e) { echo $e->getTraceAsString(), "\n"; }
$g = new Fiber(function () { debug_print_backtrace(); });
function st($g) { $g->start(); }
st($g);

?>
--EXPECTF--
gen start
suspended: from-gen
gen resumed with R
v=1
v=2
fiber returned: done
g=1
caught boom
s1:1
got deep
in-map-1
in-map-2
map=m1,m2
before-throw
caught in fiber: thrown in
inner start -> inner sees a fiber
inner-s
inner ret ir!
NULL
bool(true)
string(3) "end"
escaped: escapes
bool(true)
#0 %s(43): inner()
#1 %s(45): body()
#2 [internal function]: {closure:%s:45}()
#3 %s(44): Fiber->resume()
#4 %s(47): outerResume(Object(Fiber))
#5 {main}
#0 [internal function]: {closure:%s:48}()
#1 %s(49): Fiber->start()
#2 %s(50): st(Object(Fiber))
