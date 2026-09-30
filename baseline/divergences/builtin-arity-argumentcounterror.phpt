--TEST--
Wrong argument count to a builtin throws ArgumentCountError, not Error
--FILE--
<?php
try { strlen(); } catch (Error $e) { echo get_class($e), ": ", $e->getMessage(), "\n"; }
try { str_repeat("x"); } catch (Error $e) { echo get_class($e), "\n"; }
?>
--EXPECTF--
ArgumentCountError: strlen() expects exactly 1 argument, 0 given
ArgumentCountError
