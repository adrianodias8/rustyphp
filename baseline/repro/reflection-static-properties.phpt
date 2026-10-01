--TEST--
Reflection of static properties: getProperties(IS_STATIC), declared defaults, getValue/setValue, getStaticPropertyValue
--FILE--
<?php
class Html { protected static $seenIds; protected static $seenIdsInit; public static $pub = [1];
  static function id($s) { if (!isset(static::$seenIdsInit)) { static::$seenIdsInit = []; } if (isset(static::$seenIds[$s])) { $s .= "--" . ++static::$seenIds[$s]; } else { static::$seenIds[$s] = 1; } return $s; } }
echo Html::id("a"), " ", Html::id("a"), "\n";
$r = new ReflectionClass("Html");
$d = $r->getDefaultProperties();
foreach ($r->getProperties(ReflectionProperty::IS_STATIC) as $p) {
    echo $p->getName(), " static=", var_export($p->isStatic(), true), " cur=", json_encode($p->getValue()), " default=", json_encode($d[$p->getName()] ?? "MISSING"), "\n";
    $p->setValue(null, $d[$p->getName()] ?? null);
}
echo Html::id("a"), "\n";
var_dump(array_key_exists("seenIds", $d), $r->getStaticPropertyValue("pub"));
class A { public $i = 1; public static $s = [1, 2]; protected static $p; private $q; const X = 1; }
class B extends A { public static $t = "t"; private static $u = 3; }
$r = new ReflectionClass("B");
foreach ([null, ReflectionProperty::IS_STATIC, ReflectionProperty::IS_PUBLIC, ReflectionProperty::IS_PRIVATE] as $f) {
  $names = array_map(fn($p) => $p->getName(), $r->getProperties($f)); sort($names); echo json_encode($f), ": ", implode(",", $names), "\n"; }
$sp = $r->getStaticProperties(); ksort($sp); var_dump($sp);
?>
--EXPECT--
a a--2
seenIds static=true cur={"a":2} default="MISSING"
seenIdsInit static=true cur=[] default="MISSING"
pub static=true cur=[1] default=[1]
a
bool(true)
array(1) {
  [0]=>
  int(1)
}
null: i,p,s,t,u
16: p,s,t,u
1: i,s,t
4: u
array(4) {
  ["p"]=>
  NULL
  ["s"]=>
  array(2) {
    [0]=>
    int(1)
    [1]=>
    int(2)
  }
  ["t"]=>
  string(1) "t"
  ["u"]=>
  int(3)
}
