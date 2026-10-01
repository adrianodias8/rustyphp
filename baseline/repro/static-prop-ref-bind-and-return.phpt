--TEST--
Class::$p = &$x (self::, static::, named) and return of a static property from a by-reference function
--FILE--
<?php
class A {
    public static $p = null;
    public static function &viaSelf(array &$e) { self::$p = &$e; return self::$p; }
    public static function &viaStatic(array &$e) { static::$p = &$e; return static::$p; }
}
class B extends A {}
$x = [1];
A::viaSelf($x); $x[] = 2; var_dump(A::$p);
$y = ["y"];
B::viaStatic($y); $y[] = "z"; var_dump(B::$p);
A::$p = &$x; $x[] = 3; var_dump(count(A::$p));
?>
--EXPECT--
array(2) {
  [0]=>
  int(1)
  [1]=>
  int(2)
}
array(2) {
  [0]=>
  string(1) "y"
  [1]=>
  string(1) "z"
}
int(3)
