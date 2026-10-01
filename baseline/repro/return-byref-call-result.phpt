--TEST--
return of a method/static call result from a function &f(): a returned reference passes, a value notices
--FILE--
<?php
class C { public $c = ["x" => 1]; public static $s = [1];
  function &conditions() { return $this->c; } function plain() { return [9]; }
  static function &sref() { return self::$s; }
  function &viaRef() { return $this->conditions(); }
  function &viaPlain() { return $this->plain(); }
  static function &viaStatic() { return static::sref(); }
  function &literal() { return 5; } }
$o = new C;
$r = &$o->viaRef(); $r["y"] = 2; var_dump($o->c);
$p = &$o->viaPlain(); var_dump($p);
$s = &C::viaStatic(); $s[] = 2; var_dump(C::$s);
$l = &$o->literal(); var_dump($l);
?>
--EXPECTF--
array(2) {
  ["x"]=>
  int(1)
  ["y"]=>
  int(2)
}

Notice: Only variable references should be returned by reference in %s on line 6
array(1) {
  [0]=>
  int(9)
}
array(2) {
  [0]=>
  int(1)
  [1]=>
  int(2)
}

Notice: Only variable references should be returned by reference in %s on line 8
int(5)
