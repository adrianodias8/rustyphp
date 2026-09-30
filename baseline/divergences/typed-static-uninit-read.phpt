--TEST--
Reading an uninitialised typed static property must throw
--FILE--
<?php
class A { public static int $t; }
try { echo A::$t, "\n"; } catch (Error $e) { echo get_class($e), ": ", $e->getMessage(), "\n"; }
echo "done\n";
?>
--EXPECTF--
Error: Typed static property A::$t must not be accessed before initialization
done
