--TEST--
$x =& f() where &f() is declared in a file included after the caller was compiled
--FILE--
<?php
function run() {
    $b =& later_get();
    $b["n"] = ($b["n"] ?? 0) + 1;
    $c =& later_get();
    echo $c["n"], "\n";
    $d =& time_value();
    echo gettype($d), "\n";
}
$f = sys_get_temp_dir() . "/later-" . getmypid() . ".inc";
file_put_contents($f, '<?php function &later_get() { static $s = []; return $s; } function time_value() { return 5; }');
include $f;
unlink($f);
run();
run();
?>
--EXPECTF--
1

Notice: Only variables should be assigned by reference in %s on line 7
integer
2

Notice: Only variables should be assigned by reference in %s on line 7
integer
