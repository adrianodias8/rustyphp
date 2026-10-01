--TEST--
An ArrayAccess object indexing itself through $this (isset, read, write, append, compound, unset)
--FILE--
<?php
class A extends ArrayObject {
    function has($k) { return isset($this[$k]); }
    function get($k) { return $this[$k]; }
    function run() {
        $this["a"] = 1;
        $this[] = "appended";
        $this["b"] ??= "dflt";
        $this["b"] ??= "ignored";
        $this["s"] = "x";
        $this["s"] .= "y";
        $this["n"] = 1;
        $this["n"] += 5;
        unset($this["a"]);
        return empty($this["zz"]);
    }
}
$a = new A(["x" => 1]);
var_dump($a->has("x"), $a->has("y"), $a->get("x"), $a->run());
var_dump($a->getArrayCopy());
?>
--EXPECT--
bool(true)
bool(false)
int(1)
bool(true)
array(5) {
  ["x"]=>
  int(1)
  [0]=>
  string(8) "appended"
  ["b"]=>
  string(4) "dflt"
  ["s"]=>
  string(2) "xy"
  ["n"]=>
  int(6)
}
