--TEST--
"Creation of dynamic property" is reported on the line of the assignment, not the next statement
--FILE--
<?php
class K {}
$k = new K;
$k->a = 1;
echo "next\n";
$k->b = 2; echo "same line\n";
$r = &$k->c;
var_dump($k->a, $k->b);
?>
--EXPECTF--
Deprecated: Creation of dynamic property K::$a is deprecated in %s on line 4
next

Deprecated: Creation of dynamic property K::$b is deprecated in %s on line 6
same line

Deprecated: Creation of dynamic property K::$c is deprecated in %s on line 7
int(1)
int(2)
