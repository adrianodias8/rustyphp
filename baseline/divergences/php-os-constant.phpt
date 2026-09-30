--TEST--
PHP_OS is the running platform, not a build-time constant
--FILE--
<?php
var_dump(PHP_OS === php_uname("s"));
?>
--EXPECTF--
bool(true)
