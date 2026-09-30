--TEST--
count()/sizeof() on a Countable object reached through a dynamic call
--DESCRIPTION--
Same root cause as count-countable-in-namespace.phpt (fixed together): the
dynamic builtin path (string callables, call_user_func, array_map, first-class
callable syntax, the run-time namespace fallback) skipped the pre-call
Countable dispatch that a directly compiled `count($x)` performed.
--FILE--
<?php
class Pool implements Countable { public function count(): int { return 3; } }
$p = new Pool();
$f = "count";
echo "1: ", $f($p), "\n";
echo "2: ", call_user_func("count", $p), "\n";
echo "3: ", implode(",", array_map("count", [$p, [1, 2]])), "\n";
echo "4: ", call_user_func_array("sizeof", [$p]), "\n";
$g = count(...);
echo "5: ", $g($p), "\n";
echo "6: ", call_user_func("\\count", $p), "\n";
?>
--EXPECT--
1: 3
2: 3
3: 3,2
4: 3
5: 3
6: 3
