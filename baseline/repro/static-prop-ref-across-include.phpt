--TEST--
A reference to a static property taken inside an included unit aliases the property
--DESCRIPTION--
Session 2 flagged, from reading `relocate_module_class_ids`, that `Op::StaticPropRef` might be
missing from the include-unit class-id relocation. This is the scenario that would show it
(references to a main-image class and to two unit-local classes, taken inside the unit); it
passes, so the flag is closed as not reproduced and this guards it. Expected output is the
oracle's (PHP 8.5.7).
--FILE--
<?php
class Main { public static $p = 1; }
$tmp = sys_get_temp_dir() . "/kd2_" . getmypid() . ".php";
file_put_contents($tmp, '<?php
class Extra { public static $q = 10; }
class Extra2 { public static $r = 20; }
$r = &Extra2::$r; $r = 21;
$m = &Main::$p; $m = 2;
$q = &Extra::$q; $q = 11;
');
include $tmp; unlink($tmp);
echo Main::$p, " ", Extra::$q, " ", Extra2::$r, "\n";
?>
--EXPECT--
2 11 21
