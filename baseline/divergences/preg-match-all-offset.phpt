--TEST--
preg_match_all() honours its offset argument
--FILE--
<?php
var_dump(preg_match_all("/a/", "aXaXa", $m, PREG_OFFSET_CAPTURE, 2), $m[0][0][1]);
?>
--EXPECT--
int(2)
int(2)
