--TEST--
D-29: the "implements the Serializable interface" deprecation is raised when the declaration is reached, with its line
--FILE--
<?php
echo "start\n";

class Ser implements Serializable { function serialize() { return ''; } function unserialize($s) {} }
?>
--EXPECTF--
start

Deprecated: Ser implements the Serializable interface, which is deprecated. Implement __serialize() and __unserialize() instead (or in addition, if support for old PHP versions is necessary) in %s on line 4
