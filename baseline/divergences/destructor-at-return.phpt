--TEST--
D-24: a returning function's locals are destructed inside the return, before the caller uses the value
--FILE--
<?php
class D {
    function __construct(public $t) {}
    function __destruct() { echo "[{$this->t}]"; }
}
function f() { $a = new D('local'); return 'r'; }
function g() { foreach ([new D('a1'), new D('a2')] as $i => $v) { if ($i === 1) { return 'g'; } } }
echo f(), "\n";
echo g(), "\n";
$x = f() . '!';
echo $x, "\n";
?>
--EXPECT--
[local]r
[a1][a2]g
[local]r!
