--TEST--
A method call result sent to a by-reference parameter: a returned reference binds, a value passes with a notice
--FILE--
<?php
class NestedArray { public static function setValue(array &$array, array $parents, $value) { $ref = &$array; foreach ($parents as $p) { $ref = &$ref[$p]; } $ref = $value; } }
class FS {
    protected $values = [];
    public function &getValues() { return $this->values; }
    public function getCopy() { return $this->values; }
    public function setValue($key, $value) { NestedArray::setValue($this->getValues(), (array) $key, $value); return $this; }
    public function setCopy($key, $value) { NestedArray::setValue($this->getCopy(), (array) $key, $value); return $this; }
}
$f = new FS;
$f->setValue(["a", "b"], 1)->setValue("c", 2);
var_dump($f->getValues());
$f->setCopy("lost", 3);
var_dump(count($f->getValues()));
?>
--EXPECTF--
array(2) {
  ["a"]=>
  array(1) {
    ["b"]=>
    int(1)
  }
  ["c"]=>
  int(2)
}

Notice: Only variables should be passed by reference in %s on line 8
int(2)
