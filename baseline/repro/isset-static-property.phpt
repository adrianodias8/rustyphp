--TEST--
isset()/empty()/?? on static properties of a named or dynamic class
--DESCRIPTION--
Found by Zend/micro_bench.php, which phpr rejected at parse time
("unsupported construct (assignment target)" on `$x = isset(Foo::$a)`).
Fixed by a quiet (BP_VAR_IS) static-property fetch: an undeclared property,
one not visible from the calling scope, and an uninitialized typed property
all read as unset, without throwing. Class resolution still throws (see
isset-static-property-class-not-found.phpt).
--FILE--
<?php
class Foo {
    public static $a = 1;
    public static $n = null;
    public static $arr = ["k" => 5, "z" => null];
    public static ?Foo $inst = null;
    public static int $typed_uninit;
    private static $priv = 7;
    protected static $prot = 8;
    public $p = 3;
    static function inside() { return [isset(Foo::$priv), isset(Foo::$prot), isset(self::$priv), isset(static::$prot), isset(self::$nope), empty(Foo::$priv)]; }
}
class Bar extends Foo { static function inside2() { return [isset(Foo::$priv), isset(Foo::$prot), isset(Bar::$a)]; } }
function d(...$v) { echo implode(" ", array_map(fn($x) => var_export($x, true), $v)), "\n"; }

d(isset(Foo::$a), isset(Foo::$n), isset(Foo::$nope), isset(Foo::$priv), isset(Foo::$prot), isset(Foo::$typed_uninit));
d(isset(Foo::$arr["k"]), isset(Foo::$arr["z"]), isset(Foo::$arr["q"]), isset(Foo::$arr["k"]["x"]), isset(Foo::$nope["q"]));
d(isset(Foo::$inst->p)); Foo::$inst = new Foo; d(isset(Foo::$inst->p), isset(Foo::$inst->zz));
d(Foo::inside(), Bar::inside2());
$c = "Foo"; d(isset($c::$a), isset($c::$nope), isset($c::$priv), empty($c::$nope), $c::$a ?? "d", $c::$nope ?? "d");
$x = 5; d(isset($x, Foo::$a), isset(Foo::$a, $nope), isset(Foo::$n, $x), isset(Foo::$a, Foo::$arr["k"]));
$p = "a"; d(isset(Foo::$$p), empty(Foo::$$p), Foo::$$p ?? "d"); $p = "nope"; d(isset(Foo::$$p), empty(Foo::$$p), Foo::$$p ?? "d");
d(empty(Foo::$a), empty(Foo::$n), empty(Foo::$nope), empty(Foo::$priv), empty(Foo::$typed_uninit), empty(Foo::$arr["q"]), empty(Foo::$nope[1]));
d(Foo::$nope ?? "dflt", Foo::$priv ?? "dflt", Foo::$a ?? "dflt", Foo::$typed_uninit ?? "dflt", Foo::$n ?? "dflt");
d(Foo::$arr["k"] ?? "d", Foo::$arr["q"] ?? "d", Foo::$nope["q"] ?? "d", Foo::$inst->p ?? "d");
Foo::$typed_uninit ??= 9; Foo::$n ??= 4; d(Foo::$typed_uninit, Foo::$n);
// a loud read stays loud
foreach ([fn() => Foo::$nope, fn() => Foo::$priv, fn() => Foo::$nope ??= 1] as $f) {
    try { $f(); echo "no error\n"; } catch (Error $e) { echo get_class($e), ": ", $e->getMessage(), "\n"; }
}
?>
--EXPECT--
true false false false false false
true false false false false
false
true false
array (
  0 => true,
  1 => true,
  2 => true,
  3 => true,
  4 => false,
  5 => false,
) array (
  0 => false,
  1 => true,
  2 => true,
)
true false false true 1 'd'
true false false true
true false 1
false true 'd'
false true true true true true true
'dflt' 'dflt' 1 'dflt' 'dflt'
5 'd' 'd' 3
9 4
Error: Access to undeclared static property Foo::$nope
Error: Cannot access private property Foo::$priv
Error: Access to undeclared static property Foo::$nope
