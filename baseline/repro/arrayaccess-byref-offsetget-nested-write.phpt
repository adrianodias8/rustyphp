--TEST--
Nested writes through WeakMap and a by-reference ArrayAccess::offsetGet (variable- and property-rooted)
--FILE--
<?php
$m = new WeakMap; $k = new stdClass; $k2 = new stdClass;
if (!isset($m[$k])) { $m[$k] = []; }
$m[$k][] = "a"; $m[$k][] = "b";
var_dump($m[$k], count($m));
class AA implements ArrayAccess { private $d = []; function offsetExists($o): bool { return isset($this->d[$o]); } function &offsetGet($o): mixed { return $this->d[$o]; } function offsetSet($o, $v): void { $this->d[$o] = $v; } function offsetUnset($o): void { unset($this->d[$o]); } }
$a = new AA; $a["k"] = []; $a["k"][] = 1; var_dump($a["k"]);
class P { private ?WeakMap $pc = null; function run($e, $ep) { if ($this->pc === null) { $this->pc = new WeakMap; } if (!isset($this->pc[$e])) { $this->pc[$e] = []; } $this->pc[$e][] = $ep; $this->pc[$e][] = $ep . "2"; return $this->pc[$e]; } }
var_dump((new P)->run(new stdClass, "x"));
?>
--EXPECT--
array(2) {
  [0]=>
  string(1) "a"
  [1]=>
  string(1) "b"
}
int(1)
array(1) {
  [0]=>
  int(1)
}
array(2) {
  [0]=>
  string(1) "x"
  [1]=>
  string(2) "x2"
}
