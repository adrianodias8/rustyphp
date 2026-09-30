--TEST--
isset()/empty()/?? on a static property of an unknown class still throws
--FILE--
<?php
spl_autoload_register(function ($c) { echo "autoload($c)\n"; });
foreach ([fn() => isset(NoSuch::$a), fn() => empty(NoSuch::$a), fn() => NoSuch::$a ?? 1, function () { $c = "NoSuch"; return isset($c::$a); }, function () { $c = 12; return isset($c::$a); }] as $f) {
    try { var_dump($f()); } catch (Error $e) { echo get_class($e), ": ", $e->getMessage(), "\n"; }
}
?>
--EXPECT--
autoload(NoSuch)
Error: Class "NoSuch" not found
autoload(NoSuch)
Error: Class "NoSuch" not found
autoload(NoSuch)
Error: Class "NoSuch" not found
autoload(NoSuch)
Error: Class "NoSuch" not found
Error: Class name must be a valid object or a string
