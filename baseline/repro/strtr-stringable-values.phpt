--TEST--
strtr() with Stringable objects among the replacement values
--FILE--
<?php
class Mk { function __construct(private $s) {} function __toString(): string { return $this->s; } }
var_dump(strtr("Hello @name, %x", ["@name" => new Mk("World"), "%x" => 5]));
var_dump(strtr(new Mk("abc"), ["b" => "B"]));
var_dump(str_replace("@a", new Mk("A"), "x@a"));
?>
--EXPECT--
string(14) "Hello World, 5"
string(3) "aBc"
string(2) "xA"
