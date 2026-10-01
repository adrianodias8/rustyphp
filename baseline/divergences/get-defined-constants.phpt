--TEST--
get_defined_constants() exists and lists user and builtin constants
--FILE--
<?php
define("MY_C", 7);
$c = get_defined_constants();
var_dump($c["MY_C"], $c["PHP_INT_SIZE"], isset(get_defined_constants(true)["user"]["MY_C"]));
?>
--EXPECT--
int(7)
int(8)
bool(true)
