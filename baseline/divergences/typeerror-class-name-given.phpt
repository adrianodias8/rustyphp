--TEST--
An argument TypeError names the class of an object argument, not "object"
--FILE--
<?php
class C {}
function f(int $x) {}
try { f(new C); } catch (TypeError $e) { echo $e->getMessage(), "\n"; }
?>
--EXPECTF--
f(): Argument #1 ($x) must be of type int, C given, called in %s on line 4
