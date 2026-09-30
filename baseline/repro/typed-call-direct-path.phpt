--TEST--
Typed functions on the direct call path; single-key array writes without a Vec
--DESCRIPTION--
Fork slice 5. A function or method with parameter type hints (and no
by-reference or variadic parameter) is now a `simple_call`: the call site moves
the arguments straight into the callee's slots and the body starts with
Op::CoerceParams, which coerces / checks them in the callee frame with the
caller's line and strict_types (before, every such call built an argument Vec
and enter_callee coerced). `$a[k] = v`, `$a[k] op= v` and `$a[k]++` pop their
key directly instead of splitting off a one-element Vec. Covers weak-mode
coercion, nullable / union / class hints, TypeError messages and lines, methods
through the inline cache, closures, generators, callbacks, recursion, and every
single- and multi-key array write form including ArrayAccess and string
offsets. Expected output is the oracle's (PHP 8.5.7).
--FILE--
<?php
// Typed parameters through the direct call path.
function add1(int $x): int { return $x + 1; }
function f2(int $a, ?string $b, float $c = 1.5, int|string $d = 0): string { return json_encode([$a, $b, $c, $d]); }
function nn(?int $x) { return var_export($x, true); }
function bo(bool $b, string $s, array $a, iterable $i, callable $c, object $o, mixed $m, ?C $k = null) { return gettype($b) . gettype($s) . count($a) . gettype($c) . gettype($o) . gettype($m) . var_export($k, true); }
class C { public function m(int $x, string $y): string { return $x . $y; } public function n(self $o, ?int $z = null): static { return $this; } public static function s(float $f): float { return $f * 2; } }
echo add1(1), add1("2"), add1(3.0), add1(true), "\n";
echo f2("7", null), f2(1, "s", 2, "d"), f2(1, "s", 3), "\n";
echo nn(null), nn(5), nn("6"), "\n";
echo bo(1, 5, [1], [2], 'strlen', new C, null), "\n";
$c = new C; for ($i = 0; $i < 3; $i++) { echo $c->m($i, "y"), $c->m("1", 2), C::s(2), $c->n($c) === $c ? "S" : "-", $c->n($c, "3") === $c ? "S" : "-"; } echo "\n";
foreach ([["x"], [null], [[]], ["12abc"]] as $args) {
    try { echo add1(...$args), "|"; } catch (TypeError $e) { echo get_class($e), ": ", $e->getMessage(), " @", $e->getLine(), "|"; }
}
echo "\n";
try { $c->m("a", "b"); } catch (TypeError $e) { echo $e->getMessage(), " @", $e->getLine(), "\n", $e->getTraceAsString(), "\n"; }
try { add1(); } catch (ArgumentCountError $e) { echo $e->getMessage(), "\n"; }
$r = 5; function byval(int $x) { $x++; return $x; } echo byval($r), $r, "\n";
$ref = &$r; echo byval($ref), add1($ref), "\n";
// Recursion and generators/closures with hints.
function fib(int $n): int { return $n < 2 ? $n : fib($n - 1) + fib($n - 2); } echo fib(15), "\n";
$cl = function (int $x, string ...$r) { return $x . implode("", $r); }; echo $cl(1, "a", "b"), $cl("2"), "\n";
function gen(int $n) { for ($i = 0; $i < $n; $i++) yield $i; } echo implode(",", iterator_to_array(gen("3"))), "\n";
$arr = array_map(fn(int $v): int => $v * 2, ["1", 2, 3.0]); echo json_encode($arr), "\n";
usort($arr, fn(int $a, int $b): int => $b <=> $a); echo json_encode($arr), "\n";
// Path writes with one key and more.
$a = [1, 2]; $a[0] = 5; $a[1] .= "x"; $a[0]++; $a[0] += 2; $a[1] = 7; --$a[1]; $a["k"] = 1; $a["k"] ??= 2; $a["z"] ??= 3; $a[] = 9;
$a["l"][] = 1; $a["l"][] = 2; $a["l"]["m"][] = 3; $a["l"]["m"][0] .= "z"; $a[5][6] = 7; $a[5][6] .= "y"; $a[5][6]++; $a[5][7][8] = 1; $a[5][7][8]--; echo json_encode($a), "\n";
$s = "abc"; $s[1] = "X"; $s[5] = "Y"; echo $s, "\n";
class AA implements ArrayAccess { public $d = []; function offsetExists($o): bool { echo "E($o)"; return isset($this->d[$o]); } function offsetGet($o): mixed { echo "G($o)"; return $this->d[$o] ?? null; } function offsetSet($o, $v): void { echo "S(", var_export($o, true), ")"; if ($o === null) $this->d[] = $v; else $this->d[$o] = $v; } function offsetUnset($o): void { echo "U($o)"; unset($this->d[$o]); } }
$o = new AA; $o["k"] = 1; $o[] = 2; $x = $o["k"] = 3; $o["n"] ??= 4; unset($o["k"]); echo isset($o["m"]) ? "I" : "-", empty($o["z"]) ? "E" : "-", json_encode($o->d), $x, "\n";
$b = [1]; $r2 = &$b[0]; $b[0] = 2; $b[0]++; echo $r2, "\n";
$g = []; function w() { global $g; $g["x"] = 1; $g["x"] .= "y"; $g["x"]++; $GLOBALS["g"]["q"] = 2; $_GET["p"] = 3; $_GET["p"]++; } w(); echo json_encode($g), $_GET["p"], "\n";
?>
--EXPECTF--
2342
[7,null,1.5,0][1,"s",2,"d"][1,"s",3,0]
NULL56
booleanstring1stringobjectNULLNULL
0y124SS1y124SS2y124SS
TypeError: add1(): Argument #1 ($x) must be of type int, string given, called in %s on line 14 @3|TypeError: add1(): Argument #1 ($x) must be of type int, null given, called in %s on line 14 @3|TypeError: add1(): Argument #1 ($x) must be of type int, array given, called in %s on line 14 @3|TypeError: add1(): Argument #1 ($x) must be of type int, string given, called in %s on line 14 @3|
C::m(): Argument #1 ($x) must be of type int, string given, called in %s on line 17 @7
#0 %s(17): C->m('a', 'b')
#1 {main}
Too few arguments to function add1(), 0 passed in %s on line 18 and exactly 1 expected
65
66
610
1ab2
0,1,2
[2,4,6]
[6,4,2]

Deprecated: Increment on non-numeric string is deprecated, use str_increment() instead in %s on line 29
{"0":8,"1":6,"k":1,"z":3,"2":9,"l":{"0":1,"1":2,"m":["3z"]},"5":{"6":"7z","7":{"8":0}}}
aXc  Y
S('k')S(NULL)S('k')E(n)S('n')U(k)E(m)-E(z)E{"0":2,"n":4}3
3

Deprecated: Increment on non-numeric string is deprecated, use str_increment() instead in %s on line 34
{"x":"1z","q":2}4
