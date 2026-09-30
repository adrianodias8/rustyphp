--TEST--
Builtin function names are case-insensitive (global namespace)
--DESCRIPTION--
phpr looked up its value-builtin registry with the name as written, so
`StrToUpper("q")` or `COUNT($a)` raised "Call to undefined function": at
compile time, through string callables, call_user_func, array_map,
function_exists, is_callable and first-class callable syntax. Host builtins and
user functions already folded case. Expected output is the oracle's (PHP 8.5.7).
--FILE--
<?php
// Builtin names are case-insensitive — global namespace.
echo StrToUpper("q"), COUNT([1, 2]), Preg_Quote("a.b"), " ", SPRINTF("%05.1f", 3.14159), "\n";
$a = [3, 1, 2]; SORT($a); Array_Push($a, 9); echo Implode(",", $a), " ", END($a), " ", Array_Pop($a), "\n";
echo Preg_Match('/(\d+)/', "ab12", $m), $m[1], " ", Str_Replace("a", "b", "aaa", $cnt), $cnt, "\n";
function f() { return Func_Get_Args(); }
echo Json_Encode(f(1, 2)), " ", Is_Callable('StrLen') ? "y" : "n", Function_Exists('STRLEN') ? "y" : "n", "\n";
$x = 1; $y = 2; echo Json_Encode(Compact('x', 'y')), "\n";
$fn = 'StrToUpper'; echo $fn("dyn"), " ", Call_User_Func('StrRev', "abc"), " ", implode(",", Array_Map('StrToUpper', ["a", "b"])), "\n";
$fc = StrToLower(...); echo $fc("FCC"), " ", (new ReflectionFunction('StrLen'))->getName(), "\n";
echo IntDiv(7, 2), Max(1, 5), Abs(-3), " ", Is_Array([]) ? "t" : "f", Is_Int(1) ? "t" : "f", "\n";
class C implements Countable { function count(): int { return 42; } }
echo COUNT(new C), SizeOf(new C), "\n";
try { Nope_Fn(); } catch (Error $e) { echo $e->getMessage(), "\n"; }
echo Var_Export(true, true), Array_Sum([1, 2]), UcFirst("x"), Trim(" t "), StrLen("abc"), "\n";
?>
--EXPECT--
Q2a\.b 003.1
1,2,3,9 9 9
112 bbb3
[1,2] yy
{"x":1,"y":2}
DYN cba A,B
fcc strlen
353 tt
4242
Call to undefined function Nope_Fn()
true3Xt3
