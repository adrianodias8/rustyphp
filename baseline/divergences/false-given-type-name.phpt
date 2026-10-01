--TEST--
A TypeError names a false argument as false, not bool
--FILE--
<?php
try { get_class(false); } catch (TypeError $e) { echo $e->getMessage(), "\n"; }
try { strlen([]); } catch (TypeError $e) { echo $e->getMessage(), "\n"; }
?>
--EXPECT--
get_class(): Argument #1 ($object) must be of type object, false given
strlen(): Argument #1 ($string) must be of type string, array given
