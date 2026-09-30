--TEST--
Unqualified calls in a namespace: every builtin can be shadowed; a site binds once
--DESCRIPTION--
An unqualified call inside a namespace binds the namespaced function if one is
declared when the site first runs, else the global one, and keeps that binding
(Zend's INIT_NS_FCALL_BY_NAME runtime-cache slot). phpr bound host builtins,
by-reference builtins, prelude functions and global user functions statically,
so `App\sprintf`, `App\sort`, `App\preg_match`, `App\time` could never
shadow them (336 of 762 builtins), and it re-resolved by-value registry
builtins on every call, so an already-bound site switched to a shadow declared
later. Covers: by-reference builtins behind the guard with no shadow, argument
side effects evaluated once, spreads, shadows with by-reference parameters,
stickiness of sites that already ran, fully qualified names, named arguments,
methods / closures / generators, undefined-then-declared. Expected output is
the oracle's (PHP 8.5.7).
--FILE--
<?php
namespace App;
// 1. By-reference builtins behind the guard, no shadow: must behave as before.
function refs() {
    $a = [3, 1, 2]; sort($a); array_push($a, 9, 8); $last = array_pop($a); array_unshift($a, 0);
    $first = array_shift($a); $e = end($a); $r = reset($a); $k = key($a); $c = current($a); next($a);
    usort($a, fn($x, $y) => $y <=> $x);
    preg_match('/(\d)(\d)/', "x42", $m); preg_match_all('/\d/', "a1b2", $mm);
    $n = sscanf("12 apples", "%d %s", $num, $what);
    $d1 = [3, 1, 2]; $d2 = ['c', 'a', 'b']; array_multisort($d1, $d2);
    $s = str_replace("a", "b", "aaa", $cnt);
    $o = new \stdClass; $o->list = [2, 1]; sort($o->list); $arr = ['k' => [5, 4]]; sort($arr['k']);
    parse_str("a=1&b=2", $ps); $w = [1, 2]; array_walk($w, function (&$v, $k) { $v *= 2; });
    return json_encode([$a, $last, $first, $e, $r, $k, $c, $m, $mm, $n, $num, $what, $d1, $d2, $s, $cnt, $o->list, $arr, $ps, $w]);
}
echo refs(), "\n", refs() === refs() ? "stable" : "UNSTABLE", "\n";
// 2. Argument side effects happen exactly once, in order.
function tick($v) { echo "[", $v, "]"; return $v; }
$a = []; array_push($a, tick(1), tick(2)); echo count($a), " ";
echo sprintf("%s-%s", tick("a"), tick("b")), " ", max(tick(3), tick(4)), " ", json_encode(array_map(fn($v) => $v + 1, [tick(5)])), "\n";
// 3. Spread on host and registry builtins.
$args = ["%s+%s", 1, 2]; echo sprintf(...$args), " ", max(...[1, 7, 3]), " ", implode(...[",", [1, 2]]), "\n";
// 4. Shadows declared BEFORE the first call bind; by-reference parameters of the shadow work.
eval('namespace App; function sort(array &$a) { $a = ["shadow-sort"]; return true; }
      function preg_match($p, $s, &$m = null) { $m = ["shadow-match"]; return 7; }
      function sprintf(...$a) { return "shadow-sprintf"; }
      function array_map(...$a) { return ["shadow-map"]; }
      function date_create(...$a) { return "shadow-date_create"; }
      function time() { return 42; }
      function compact(...$a) { return ["shadow-compact"]; }');
function late() {
    $a = [2, 1]; $r = sort($a); $n = preg_match('/x/', "x", $m); $v = 1;
    return json_encode([$a, $r, $n, $m, sprintf("%d", 1), array_map('strtoupper', ["a"]), date_create("now"), time(), compact('v')]);
}
echo late(), "\n", late(), "\n";
$a = [2, 1]; sort($a); echo json_encode($a), " ", sprintf(...["%d", 5]), "\n";
// 5. Sites that ALREADY ran keep the global function (refs() ran above, before the shadows).
echo refs(), "\n";
// 6. Fully qualified and imported names never look at the namespace.
$b = [2, 1]; \sort($b); echo json_encode($b), \sprintf("%d", 3), \time() > 1000 ? "T" : "F", "\n";
// 7. Named arguments.
echo json_encode(array_slice([1, 2, 3, 4], offset: 1, length: 2)), " ", str_pad("x", 3, pad_string: "-"), "\n";
// 8. Closures, methods, static methods, generators: each site binds on its own.
class K {
    function m() { $a = [2, 1]; rsort($a); return implode(",", $a) . strtoupper("m"); }
    static function s() { return ucfirst("s") . implode("", array_reverse([1, 2])); }
}
$cl = function () { $a = ['b' => 1, 'a' => 2]; ksort($a); return implode(",", array_keys($a)); };
function gen() { $a = [1, 2]; while ($a) { yield array_shift($a); } }
echo (new K)->m(), K::s(), $cl(), implode("", iterator_to_array(gen())), "\n";
eval('namespace App; function rsort(array &$a) { $a = ["shadow"]; } function ksort(array &$a) { $a = ["shadow" => 1]; } function array_shift(array &$a) { $a = []; return "shadow"; } function ucfirst($s) { return "shadow"; }');
echo (new K)->m(), K::s(), $cl(), implode("", iterator_to_array(gen())), "\n";
// 9. Undefined stays undefined until declared; the error names the namespaced function.
function undef() { return nope_fn(1); }
try { undef(); } catch (\Error $e) { echo $e->getMessage(), "\n"; }
eval('namespace App; function nope_fn($v) { return "now:" . $v; }');
echo undef(), "\n";
?>
--EXPECT--
[[9,3,2,1],8,0,9,1,0,1,["42","4","2"],[["1","2"]],2,12,"apples",[1,2,3],["a","b","c"],"bbb",3,[1,2],{"k":[4,5]},{"a":"1","b":"2"},[2,4]]
stable
[1][2]2 [a][b]a-b [3][4]4 [5][6]
1+2 7 1,2
[["shadow-sort"],true,7,["shadow-match"],"shadow-sprintf",["shadow-map"],"shadow-date_create",42,["shadow-compact"]]
[["shadow-sort"],true,7,["shadow-match"],"shadow-sprintf",["shadow-map"],"shadow-date_create",42,["shadow-compact"]]
["shadow-sort"] shadow-sprintf
[[9,3,2,1],8,0,9,1,0,1,["42","4","2"],[["1","2"]],2,12,"apples",[1,2,3],["a","b","c"],"bbb",3,[1,2],{"k":[4,5]},{"a":"1","b":"2"},[2,4]]
[1,2]3T
[2,3] x--
2,1MS21a,b12
2,1MS21a,b12
Call to undefined function App\nope_fn()
now:1
