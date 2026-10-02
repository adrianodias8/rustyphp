--TEST--
file_exists() is access(F_OK): files, directories, symlinks followed (a broken one is false), file:// URLs, empty and NUL paths
--FILE--
<?php
$d = sys_get_temp_dir() . '/fe-' . getmypid();
mkdir($d);
touch("$d/f");
symlink("$d/f", "$d/good");
symlink("$d/nope", "$d/broken");
var_dump(file_exists("$d/f"), file_exists($d), file_exists("$d/good"), file_exists("$d/broken"), file_exists("$d/nope"));
var_dump(file_exists("file://$d/f"), file_exists(''), file_exists("$d/f\0x"), file_exists(new SplFileInfo("$d/f")));
var_dump(file_exists("$d/f/.."), file_exists("$d/../" . basename($d) . "/f"));
unlink("$d/broken"); unlink("$d/good"); unlink("$d/f"); rmdir($d);
var_dump(file_exists($d));
?>
--EXPECT--
bool(true)
bool(true)
bool(true)
bool(false)
bool(false)
bool(true)
bool(false)
bool(false)
bool(true)
bool(false)
bool(true)
bool(false)
