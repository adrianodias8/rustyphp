--TEST--
touch() on an existing directory sets its times; a missing parent warns
--FILE--
<?php
$d = sys_get_temp_dir() . "/tch-" . getmypid(); mkdir($d);
var_dump(touch($d, 1000000000), filemtime($d), touch("$d/f"), file_exists("$d/f"), touch("$d/missing/x"));
unlink("$d/f"); rmdir($d);
?>
--EXPECTF--

Warning: touch(): Unable to create file %s/missing/x because No such file or directory in %s on line 3
bool(true)
int(1000000000)
bool(true)
bool(true)
bool(false)
