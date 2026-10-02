--TEST--
D-26: freeing a container destructs an element's own properties before the next element
--FILE--
<?php
class W { function __construct(public $n, public $o = null) {} function __destruct() { echo "destruct {$this->n}\n"; } }
$v = [new W(7, new W(0)), new W(9)];
unset($v);
echo "end\n";
?>
--EXPECT--
destruct 7
destruct 0
destruct 9
end
