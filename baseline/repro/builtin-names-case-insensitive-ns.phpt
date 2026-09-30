--TEST--
Builtin function names are case-insensitive (inside a namespace)
--DESCRIPTION--
The same as builtin-names-case-insensitive.phpt through the namespaced
two-step lookup (App\StrToUpper, then global strtoupper), including the
by-reference and out-parameter builtins that compile behind Op::NsShadowGuard.
Expected output is the oracle's (PHP 8.5.7).
--FILE--
<?php
namespace App;
echo StrToUpper("q"), COUNT([1, 2]), Preg_Quote("a.b"), " ", SPRINTF("%05.1f", 3.14159), \StrToLower("Q"), "\n";
$a = [3, 1, 2]; SORT($a); Array_Push($a, 9); echo Implode(",", $a), " ", END($a), " ", Array_Pop($a), "\n";
echo Preg_Match('/(\d+)/', "ab12", $m), $m[1], " ", Str_Replace("a", "b", "aaa", $cnt), $cnt, "\n";
function f() { return Func_Get_Args(); }
echo Json_Encode(f(1, 2)), " ", Is_Callable('StrLen') ? "y" : "n", Function_Exists('STRLEN') ? "y" : "n", "\n";
$x = 1; $y = 2; echo Json_Encode(Compact('x', 'y')), "\n";
class C implements \Countable { function count(): int { return 42; } }
echo COUNT(new C), SizeOf(new C), "\n";
try { Nope_Fn(); } catch (\Error $e) { echo $e->getMessage(), "\n"; }
function G() { return "g"; }
echo g(), G(), \App\g(), "\n";
for ($i = 0; $i < 3; $i++) echo Max($i, 1), StrRev("ab"), Array_Key_Last([1, 2]), " ";
echo "\n";
?>
--EXPECT--
Q2a\.b 003.1q
1,2,3,9 9 9
112 bbb3
[1,2] yy
{"x":1,"y":2}
4242
Call to undefined function App\Nope_Fn()
ggg
1ba1 1ba1 2ba1 
