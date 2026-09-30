--TEST--
Compound assignment on an ArrayAccess element is an indirect modification (offsetGet only)
--FILE--
<?php
class AA implements ArrayAccess {
    public $d = ["k" => "v", "n" => 1];
    function offsetExists($o): bool { echo "E($o)"; return isset($this->d[$o]); }
    function offsetGet($o): mixed { echo "G($o)"; return $this->d[$o] ?? null; }
    function offsetSet($o, $v): void { echo "S($o)"; $this->d[$o] = $v; }
    function offsetUnset($o): void { echo "U($o)"; unset($this->d[$o]); }
}
$o = new AA;
$o["k"] .= "x";
$o["n"]++;
echo "\n", json_encode($o->d), "\n";
?>
--EXPECTF--
G(k)S(k)G(n)
Notice: Indirect modification of overloaded element of AA has no effect in %s on line 11

{"k":"vx","n":1}
