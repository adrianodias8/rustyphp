--TEST--
`.=` on a missing nested key of a null variable warns for each missing key
--FILE--
<?php
$n = null;
$n["s"]["t"] .= "u";
echo json_encode($n), "\n";
?>
--EXPECTF--

Warning: Undefined array key "s" in %s on line 3

Warning: Undefined array key "t" in %s on line 3
{"s":{"t":"u"}}
