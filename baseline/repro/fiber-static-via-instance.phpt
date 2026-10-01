--TEST--
Fiber::suspend()/getCurrent() called through an instance
--FILE--
<?php
$f = new Fiber(function () { $me = Fiber::getCurrent(); $x = $me->suspend("via-instance"); echo "got $x\n"; var_dump($me->getCurrent() === $me); });
echo $f->start(), "\n"; $f->resume("R");
?>
--EXPECT--
via-instance
got R
bool(true)
