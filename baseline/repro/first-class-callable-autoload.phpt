--TEST--
A first-class callable / Closure::fromCallable() on a class not yet loaded triggers the autoloader
--FILE--
<?php
spl_autoload_register(function ($c) { echo "autoload($c)\n"; if ($c === "U\\Unicode") eval("namespace U; class Unicode { static function strcasecmp(\$a, \$b) { return strcasecmp(\$a, \$b); } }"); });
$f = U\Unicode::strcasecmp(...);
var_dump($f("A", "a"));
$g = Closure::fromCallable("U\\Unicode::strcasecmp");
var_dump($g("b", "a"));
?>
--EXPECT--
autoload(U\Unicode)
int(0)
int(1)
