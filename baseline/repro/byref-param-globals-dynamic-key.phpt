--TEST--
$GLOBALS[$name] sent to a by-reference parameter writes (or creates) the global variable
--FILE--
<?php
class SE {
    public static function setGlobal(&$ref, $data) { $ref = is_array($ref) ? array_merge($ref, $data) : $data; }
    public static function rewrite(array $settings) {
        foreach ($settings as $setting => $data) { self::setGlobal($GLOBALS[$setting], $data); }
    }
}
$config = ["a" => 1];
SE::rewrite(["config" => ["b" => 2], "fresh" => ["x"]]);
var_dump($config, $fresh);
function viaFn(&$r) { $r = "set"; }
$k = "dyn";
viaFn($GLOBALS[$k]);
var_dump($dyn);
?>
--EXPECT--
array(2) {
  ["a"]=>
  int(1)
  ["b"]=>
  int(2)
}
array(1) {
  [0]=>
  string(1) "x"
}
string(3) "set"
