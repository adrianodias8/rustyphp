--TEST--
D-28: unserialize() honours its options: allowed_classes and max_depth
--FILE--
<?php
class C {}
var_dump(get_class(unserialize('O:1:"C":0:{}', ['allowed_classes' => false])));
var_dump(get_class(unserialize('O:1:"C":0:{}', ['allowed_classes' => ['D']])));
var_dump(unserialize('a:1:{i:0;a:1:{i:0;N;}}', ['max_depth' => 1]));
?>
--EXPECTF--
string(22) "__PHP_Incomplete_Class"
string(22) "__PHP_Incomplete_Class"

Warning: unserialize(): Maximum depth of 1 exceeded. The depth limit can be changed using the max_depth unserialize() option or the unserialize_max_depth ini setting in %s on line %d

Warning: unserialize(): Error at offset 14 of 22 bytes in %s on line %d
bool(false)
