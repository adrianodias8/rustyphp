--TEST--
foreach by value holds the array instead of snapshotting it: semantics unchanged
--DESCRIPTION--
The fork's by-value foreach keeps one Rc of the array for the loop and walks it
by position (IterState::ByVal), where it used to copy every (key, value) pair
into a Vec first. Everything that copy guaranteed must still hold: the body
modifying, unsetting from, appending to or replacing the source (including
through a reference to it) never disturbs the loop; reference elements are read
live; the lingering-reference gotcha; tombstones; destructuring targets; the
value target aliasing the array; properties and static properties as sources;
hash escalation during the loop; generators and objects as controls. Expected
output is the oracle's (PHP 8.5.7).
--FILE--
<?php
function show($x) { echo json_encode($x), "\n"; }
// 1. Modifying the source inside the loop: the loop iterates the original.
$a = [1, 2, 3]; foreach ($a as $k => $v) { $a[] = $v * 10; if ($k == 0) unset($a[1]); $a[$k] = $v + 100; } show($a);
$a = [1, 2, 3]; foreach ($a as $v) { $a = null; echo $v; } echo "\n"; show($a);
$a = ['x' => 1, 'y' => 2]; foreach ($a as $k => $v) { $a['z'] = 3; unset($a[$k]); echo "$k=$v "; } show($a);
// 2. Reference elements are read live; the lingering-reference gotcha.
$a = [1, 2, 3]; foreach ($a as &$v) { $v *= 2; } unset($v); show($a);
$a = [1, 2, 3]; foreach ($a as &$v) {} foreach ($a as $v) {} show($a);
$b = 5; $a = [1, &$b, 3]; foreach ($a as $k => $v) { $b = 99; echo $v, " "; } echo "\n";
$a = [1, 2]; $r = &$a[0]; foreach ($a as $v) { $r = 50; echo $v, " "; } echo "\n"; show($a);
// 3. Iterating through a reference variable; a reference to the array modified inside.
$a = [1, 2, 3]; $ref = &$a; foreach ($ref as $v) { $ref[] = $v; echo $v; } echo "\n"; show($a);
$a = [1, 2, 3]; $ref = &$a; foreach ($a as $v) { $ref = [9]; echo $v; } echo "\n"; show($a);
// 4. Nested loops over the same array, breaks, continues, keys of all kinds.
$a = [5 => 'a', 'k' => 'b', -1 => 'c', '7' => 'd', 'x y' => 'e', 3 => 'f']; unset($a['k']);
foreach ($a as $k => $v) { foreach ($a as $k2 => $v2) { if ($k2 === -1) continue; if ($k2 === 3) break; echo "$k$k2$v2,"; } } echo "\n";
foreach ($a as $k => $v) { var_dump($k); }
// 5. Tombstones: unset before and during; packed with holes; reinsert after unset.
$a = [0, 1, 2, 3, 4, 5]; unset($a[0], $a[2], $a[5]); foreach ($a as $k => $v) { echo "$k=$v "; } echo "\n";
$a = [0, 1, 2, 3]; foreach ($a as $k => $v) { unset($a[$k + 1]); echo "$k=$v "; } echo "\n"; show($a);
// 6. Destructuring targets, properties and static properties as sources, function results.
$rows = [[1, 'a'], [2, 'b']]; foreach ($rows as [$n, $s]) { echo "$n$s"; } foreach ($rows as ['0' => $n]) { echo $n; } echo "\n";
class C { public $items = [1, 2, 3]; public static $s = ['p' => 1, 'q' => 2]; function it() { foreach ($this->items as $k => $v) { $this->items[] = $v; echo $k; } return $this->items; } }
$c = new C; show($c->it()); foreach (C::$s as $k => $v) { C::$s[$k] = $v * 2; echo $k; } show(C::$s);
function mk() { return range(1, 4); } foreach (mk() as $v) { echo $v; } echo "\n";
// 7. Value slot aliased to the array itself, key slot written in the body, empty and non-arrays.
$a = [1, 2, 3]; foreach ($a as $k => $a) { echo json_encode($a), "|"; } echo "\n"; show($a);
$a = [1, 2, 3]; foreach ($a as $k => $v) { $k = 'changed'; $v = 'changed'; } echo "$k$v\n";
foreach ([] as $v) { echo "never"; } $n = null; foreach ($n ?? [] as $v) { echo "never"; }
foreach ((array) "s" as $v) { echo $v; } echo "\n";
// 8. Big loops: sum by value, by key/value; strings as keys with hash escalation mid-loop.
$big = range(1, 100000); $s = 0; foreach ($big as $v) { $s += $v; } $t = 0; foreach ($big as $k => $v) { $t += $k; } echo $s, " ", $t, "\n";
$h = []; for ($i = 0; $i < 1000; $i++) { $h["k$i"] = $i; } $u = 0; foreach ($h as $k => $v) { $u += $v; if ($v == 500) { $h["late"] = 1; } } echo $u, " ", count($h), "\n";
// 9. Generators and objects are untouched but still here as controls.
function g() { yield 'a' => 1; yield 'b' => 2; } foreach (g() as $k => $v) { echo "$k$v"; }
$o = new stdClass; $o->p = 1; $o->q = 2; foreach ($o as $k => $v) { $o->r = 3; echo "$k$v"; } echo "\n";
// 10. Memory/identity: the loop holds the array; a copy taken in the body is independent.
$a = [1, 2, 3]; foreach ($a as $k => $v) { $copy = $a; $copy[] = 0; $a[$k] = -$v; } show($a); show($copy);
$a = [[1], [2]]; foreach ($a as $k => $v) { $v[] = 9; $a[$k][] = 8; } show($a);
?>
--EXPECT--
{"0":101,"2":103,"3":10,"4":20,"1":102,"5":30}
123
null
x=1 y=2 {"z":3}
[2,4,6]
[1,2,2]
1 99 3 
1 2 
[50,2]
123
[1,2,3,1,2,3]
123
[9]
55a,57d,5x ye,-15a,-17d,-1x ye,75a,77d,7x ye,x y5a,x y7d,x yx ye,35a,37d,3x ye,
int(5)
int(-1)
int(7)
string(3) "x y"
int(3)
1=1 3=3 4=4 
0=0 1=1 2=2 3=3 
[0]
1a2b12
012[1,2,3,1,2,3]
pq{"p":2,"q":4}
1234
1|2|3|
3
changedchanged
s
5000050000 4999950000
499500 1001
a1b2p1q2r3
[-1,-2,-3]
[-1,-2,3,0]
[[1,8],[2,8]]
