--TEST--
A float parameter of a math builtin is named float, not int|float, in its TypeError
--FILE--
<?php
try { exp("abc"); } catch (TypeError $e) { echo $e->getMessage(), "\n"; }
try { sqrt([]); } catch (TypeError $e) { echo $e->getMessage(), "\n"; }
?>
--EXPECT--
exp(): Argument #1 ($num) must be of type float, string given
sqrt(): Argument #1 ($num) must be of type float, array given
