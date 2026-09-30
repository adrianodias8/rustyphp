--TEST--
NAN coerced to string warns (PHP 8.5)
--FILE--
<?php
$s = "x" . NAN; echo $s, "\n";
$t = ""; $t .= NAN; echo $t, "\n";
?>
--EXPECTF--

Warning: unexpected NAN value was coerced to string in %s on line 2
xNAN

Warning: unexpected NAN value was coerced to string in %s on line 3
NAN
