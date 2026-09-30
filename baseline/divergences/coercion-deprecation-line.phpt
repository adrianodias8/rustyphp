--TEST--
The implicit float-to-int deprecation of an argument reports the callee, like RECV
--FILE--
<?php
function dep(int $i) {
    return $i;
}

echo dep(1.5), "\n";
?>
--EXPECTF--

Deprecated: Implicit conversion from float 1.5 to int loses precision in %s on line 2
1
