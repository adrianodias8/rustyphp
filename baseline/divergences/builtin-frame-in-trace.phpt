--TEST--
An uncaught TypeError from a builtin has the builtin's frame in its trace
--FILE--
<?php
sort($undefined_is_null);
?>
--EXPECTF--
Fatal error: Uncaught TypeError: sort(): Argument #1 ($array) must be of type array, null given in %s:2
Stack trace:
#0 %s(2): sort(NULL)
#1 {main}
  thrown in %s on line 2
