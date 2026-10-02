--TEST--
D-27: unserialize() checks fields against the class: typed properties (TypeError), dynamic-property deprecation, corrupt mangled names
--FILE--
<?php
class T { public int $i = 0; }
class P { public $a = 1; }
foreach (['O:1:"T":1:{s:1:"i";s:1:"x";}', 'O:1:"P":1:{s:1:"z";i:1;}', "O:1:\"P\":1:{s:4:\"\0P-c\";i:1;}"] as $s) {
    try { var_dump(unserialize($s)); } catch (Throwable $e) { echo get_class($e), ': ', $e->getMessage(), "\n"; }
}
?>
--EXPECTF--
TypeError: Cannot assign string to property T::$i of type int

Deprecated: Creation of dynamic property P::$z is deprecated in %s on line %d
object(P)#%d (2) {
  ["a"]=>
  int(1)
  ["z"]=>
  int(1)
}

Notice: Corrupt member variable name in %s on line %d

Warning: unserialize(): Error at offset 22 of 27 bytes in %s on line %d
bool(false)
