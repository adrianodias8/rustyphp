--TEST--
References survive argument and array-literal unpacking; by-value parameters still get copies
--FILE--
<?php
function pp(array &$v, $hook) { $v["added"] = $hook; }
class T { function pre(array &$v, $hook) { $v["m"] = $hook; } }
function invoke(callable $l, array $args) { return $l(...$args); }
$vars = []; invoke("pp", [&$vars, "html"]); var_dump($vars);
$vars = []; invoke([new T, "pre"], [&$vars, "x"]); var_dump($vars);
$vars = []; invoke((new T)->pre(...), [&$vars, "y"]); var_dump($vars);
function v($a, $b) { $a[] = 1; return count($a) . strlen($b); }
$x = [1]; $s = "ab"; $args = [&$x, &$s];
echo v(...$args), " ", count($x), "\n"; echo implode(",", array_map(fn($q) => $q, ...[[&$x]])), "\n"; echo strtoupper(...[&$s]), "\n";
$x = 1; $a = [&$x, 2]; $b = [...$a]; $b[0] = 99; var_dump($x);
?>
--EXPECTF--
array(1) {
  ["added"]=>
  string(4) "html"
}
array(1) {
  ["m"]=>
  string(1) "x"
}
array(1) {
  ["m"]=>
  string(1) "y"
}
22 1

Warning: Array to string conversion in %s on line 10
Array
AB
int(99)
