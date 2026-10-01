--TEST--
$v = &C::m(): a static method returning by reference binds; a by-value one binds with a notice
--FILE--
<?php
class NA {
    public static function &getValue(array &$array, array $parents, &$key_exists = NULL) { $ref = &$array; foreach ($parents as $p) { if (is_array($ref) && array_key_exists($p, $ref)) { $ref = &$ref[$p]; } else { $key_exists = FALSE; $null = NULL; return $null; } } $key_exists = TRUE; return $ref; }
    public static function plain() { return 5; }
}
class FS {
    protected $values = ["a" => ["b" => 1]];
    public function &getValues() { return $this->values; }
    public function &getValue($key, $default = NULL) { $exists = NULL; $value = &NA::getValue($this->getValues(), (array) $key, $exists); if (!$exists) { $value = $default; } return $value; }
}
$f = new FS;
$v = &$f->getValue(["a", "b"]);
$v = 42;
var_dump($f->getValues());
var_dump($f->getValue("zz", "dflt"));
$p = &NA::plain();
var_dump($p);
?>
--EXPECTF--
array(1) {
  ["a"]=>
  array(1) {
    ["b"]=>
    &int(42)
  }
}
string(4) "dflt"

Notice: Only variables should be assigned by reference in %s on line 16
int(5)
