--TEST--
Redeclaring a builtin function is a compile-time fatal
--FILE--
<?php
echo "before\n";
function strlen($s) { return -1; }
echo strlen("abc"), "\n";
?>
--EXPECTF--
Fatal error: Cannot redeclare function strlen() in %s on line 3%A
