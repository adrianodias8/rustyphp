--TEST--
Declaring assert() in a namespace is a fatal
--FILE--
<?php
namespace App;
echo "before\n";
function assert($x) { return true; }
echo "after\n";
?>
--EXPECTF--
Fatal error: Defining a custom assert() function is not allowed, as the function has special semantics in %s on line 4%A
