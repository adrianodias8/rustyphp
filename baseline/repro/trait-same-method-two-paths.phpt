--TEST--
A trait reached directly and through another trait contributes one method, not a collision
--FILE--
<?php
trait T { function f() { return "T::f"; } }
trait U { use T; function g() { return $this->f() . "+g"; } }
class C { use T; use U; }
echo (new C)->g(), "\n";
?>
--EXPECT--
T::f+g
