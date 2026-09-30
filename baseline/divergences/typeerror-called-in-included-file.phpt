--TEST--
"called in" names the included file of a strict caller
--FILE--
<?php
function f(int $x) { return $x; }
$tmp = sys_get_temp_dir() . "/kd_" . getmypid() . ".php";
file_put_contents($tmp, '<?php declare(strict_types=1); function caller() { try { return f("2"); } catch (TypeError $e) { return $e->getMessage(); } }');
include $tmp; unlink($tmp);
echo str_replace($tmp, "INCLUDED", caller()), "\n";
?>
--EXPECTF--
f(): Argument #1 ($x) must be of type int, string given, called in INCLUDED on line 1
