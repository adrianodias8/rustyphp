--TEST--
Interned property/method names: exact-spelling hits and case-insensitive fallbacks resolve the same members
--FILE--
<?php
interface Shape { function area(); }
class Base implements Shape {
    public $items = ['a' => ['b' => 1]];
    protected $hidden = 'base';
    private $mine = 'base-private';
    function area() { return 'Base::area'; }
    function greet() { return 'Base::greet'; }
    function who() { return $this->mine; }
}
#[AllowDynamicProperties]
class Kid extends Base {
    public $extra = 5;
    private $mine = 'kid-private';
    function GREET() { return 'Kid::GREET'; }
    function hidden() { return $this->hidden; }
    function kidMine() { return $this->mine; }
}
class_alias('Kid', 'KidAlias');
$k = new Kid;
foreach (['greet', 'Greet', 'GREET', 'area', 'AREA', 'who', 'hidden', 'kidMine'] as $m) {
    echo $m, ' => ', $k->$m(), "\n";
}
echo $k->greet(), ' ', $k->GrEeT(), ' ', $k->area(), "\n";
var_dump(isset($k->items['a']['b']), isset($k->items['a']['x']), $k->items['a']['b']);
$k->items['a']['b'] = 2; $k->items['n']['m'] = 3;
unset($k->items['a']['b']);
var_dump($k->items, $k->extra);
$k->dyn = 'd';
var_dump(isset($k->dyn), isset($k->nope), $k->dyn);
function takesKid(Kid $x) { return get_class($x); }
function takesAlias(KidAlias $x) { return get_class($x); }
function takesBase(base $x) { return get_class($x); }
function takesShape(\Shape $x) { return get_class($x); }
echo takesKid($k), ' ', takesAlias($k), ' ', takesBase($k), ' ', takesShape($k), "\n";
try { takesKid(new Base); } catch (TypeError $e) { echo get_class($e), "\n"; }
?>
--EXPECT--
greet => Kid::GREET
Greet => Kid::GREET
GREET => Kid::GREET
area => Base::area
AREA => Base::area
who => base-private
hidden => base
kidMine => kid-private
Kid::GREET Kid::GREET Base::area
bool(true)
bool(false)
int(1)
array(2) {
  ["a"]=>
  array(0) {
  }
  ["n"]=>
  array(1) {
    ["m"]=>
    int(3)
  }
}
int(5)
bool(true)
bool(false)
string(1) "d"
Kid Kid Kid Kid
TypeError
