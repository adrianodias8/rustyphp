--TEST--
A $this->prop argument to an [$obj, m] / method-closure callable is read in the caller
--FILE--
<?php
class RMx { function name() { return "front"; } }
class Hooks { public function help($a, $b) { return "$a:" . gettype($b); } }
class Block {
    public $rm;
    function __construct() { $this->rm = new RMx; }
    function t($hook) {
        echo "1 ", $hook("lit", $this->rm), "\n";
        echo "2 ", $hook($this->rm->name(), $this->rm), "\n";
        $x = $this->rm; echo "3 ", $hook($this->rm->name(), $x), "\n";
        echo "4 ", $hook(strtoupper("a"), $this->rm), "\n";
    }
}
(new Block)->t([new Hooks, "help"]);
(new Block)->t((new Hooks)->help(...));
(new Block)->t(fn($a, $b) => "$a:" . gettype($b));
?>
--EXPECT--
1 lit:object
2 front:object
3 front:object
4 A:object
1 lit:object
2 front:object
3 front:object
4 A:object
1 lit:object
2 front:object
3 front:object
4 A:object
