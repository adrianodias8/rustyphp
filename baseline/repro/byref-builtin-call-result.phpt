--TEST--
A by-reference builtin given a call result works on the temporary and raises the notice
--FILE--
<?php
function g() { return [1, 2, 3]; }
class O { function a() { return ["p", "q"]; } static function s() { return [7, 8]; } }
$o = new O;
var_dump(array_shift(g()));
var_dump(array_pop($o->a()));
var_dump(end(g()));
var_dump(reset($o->a()));
var_dump(array_shift(O::s()));
var_dump(current(g()), key($o->a()));
?>
--EXPECTF--
Notice: Only variables should be passed by reference in %s on line 5
int(1)

Notice: Only variables should be passed by reference in %s on line 6
string(1) "q"

Notice: Only variables should be passed by reference in %s on line 7
int(3)

Notice: Only variables should be passed by reference in %s on line 8
string(1) "p"

Notice: Only variables should be passed by reference in %s on line 9
int(7)
int(1)
int(0)
