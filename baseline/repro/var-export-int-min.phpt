--TEST--
var_export(PHP_INT_MIN) writes the expression PHP uses (the literal would parse as a float)
--FILE--
<?php
var_export(PHP_INT_MIN); echo "\n";
var_export([PHP_INT_MIN, PHP_INT_MAX, -1]); echo "\n";
var_dump(eval('return ' . var_export(PHP_INT_MIN, true) . ';') === PHP_INT_MIN);
?>
--EXPECTF--
-9223372036854775807-1
array (
  0 => -9223372036854775807-1,
  1 => 9223372036854775807,
  2 => -1,
)
bool(true)
