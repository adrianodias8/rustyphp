--TEST--
A TypeError for a callback invoked by an internal iterator has no called in clause
--FILE--
<?php
class F { public function accept(int $x): bool { return true; } }
$it = new CallbackFilterIterator(new ArrayIterator(["a"]), [new F, "accept"]);
try { foreach ($it as $v) {} } catch (TypeError $e) { echo basename($e->getFile()), ":", $e->getLine(), " ", $e->getMessage(), "\n"; }
?>
--EXPECT--
internal-caller-no-called-in.php:2 F::accept(): Argument #1 ($x) must be of type int, string given
