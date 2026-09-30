--TEST--
Using null as an array offset is deprecated (PHP 8.5)
--FILE--
<?php
$t = [];
$t[null] = 4;
echo json_encode($t), "\n";
?>
--EXPECTF--

Deprecated: Using null as an array offset is deprecated, use an empty string instead in %s on line 3
{"":4}
