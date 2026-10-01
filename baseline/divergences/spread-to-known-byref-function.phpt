--TEST--
Argument unpacking into a known function with by-reference parameters
--FILE--
<?php
function pp(array &$v, $h) { $v[] = $h; }
$x = []; $a = [&$x, "z"]; pp(...$a); var_dump($x);
?>
--EXPECT--
array(1) {
  [0]=>
  string(1) "z"
}
