--TEST--
NAN as an array key warns that it is not representable as an int
--FILE--
<?php
$a = [NAN => 1];
echo count($a), "\n";
?>
--EXPECTF--
Warning: The float NAN is not representable as an int, cast occurred in %s on line 2

Deprecated: Implicit conversion from float NAN to int loses precision in %s on line 2
1
