--TEST--
D-25: unsetting a suspended generator destructs what its frames hold at the unset, inner generator first
--FILE--
<?php
class D {
    function __construct(public $t) {}
    function __destruct() { echo "[{$this->t}]"; }
}
function inner() { yield new D('d1'); }
function outer() { foreach ([new D('d2')] as $v) { yield from inner(); } }
$g = outer();
$g->current();
unset($g);
echo "G\n";
?>
--EXPECT--
[d1][d2]G
