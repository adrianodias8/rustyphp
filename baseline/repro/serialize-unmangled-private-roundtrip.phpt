--TEST--
__serialize() with unmangled private names: wire order kept, unserialize() restores the private (and inherited private) properties
--FILE--
<?php
class P { private ?string $class = null; private bool $pub = false; protected array $args = []; private array $changes = [];
  function set() { $this->class = "K"; $this->pub = true; $this->args = [1]; $this->changes = ["class" => true]; return $this; }
  function dump() { return [$this->class, $this->pub, $this->args, $this->changes]; }
  // Symfony's Definition: __serialize() with unmangled names, no __unserialize().
  public function __serialize(): array { $d = []; foreach ((array) $this as $k => $v) { if (false !== $i = strrpos($k, "\0")) { $k = substr($k, 1 + $i); } if (!$v) continue; $d[$k] = $v; } return $d; } }
class C extends P { private $parent; function __construct($p) { $this->parent = $p; } function getParent() { return $this->parent; } }
$s = serialize((new C("par"))->set());
echo $s, "\n";
$u = unserialize($s);
var_dump($u->dump(), $u->getParent());
?>
--EXPECT--
O:1:"C":5:{s:5:"class";s:1:"K";s:3:"pub";b:1;s:4:"args";a:1:{i:0;i:1;}s:7:"changes";a:1:{s:5:"class";b:1;}s:6:"parent";s:3:"par";}
array(4) {
  [0]=>
  string(1) "K"
  [1]=>
  bool(true)
  [2]=>
  array(1) {
    [0]=>
    int(1)
  }
  [3]=>
  array(1) {
    ["class"]=>
    bool(true)
  }
}
string(3) "par"
