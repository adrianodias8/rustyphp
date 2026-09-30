--TEST--
Writing with an array as key: "Cannot access offset of type array on array"
--FILE--
<?php
$a = [1];
try { $a[[]] = 1; } catch (TypeError $e) { echo $e->getMessage(), "\n"; }
try { echo $a[[]]; } catch (TypeError $e) { echo $e->getMessage(), "\n"; }
?>
--EXPECTF--
Cannot access offset of type array on array
Cannot access offset of type array on array
