--TEST--
stream_get_wrappers() lists user wrappers; .= on an array element / property / variable holding a Stringable object
--FILE--
<?php
class W { public $context; function url_stat($p, $f) { return false; } }
var_dump(stream_wrapper_register("public", "W"));
var_dump(in_array("public", stream_get_wrappers(), true));
var_dump(stream_wrapper_unregister("public"), in_array("public", stream_get_wrappers(), true), stream_wrapper_register("public", "W"));
class M { function __toString(): string { return "msg"; } }
$b = ["m" => new M];
$b["m"] .= "<br/>";
var_dump($b["m"]);
$o = new stdClass; $o->m = new M; $o->m .= "!"; var_dump($o->m);
$s = new M; $s .= "?"; var_dump($s);
?>
--EXPECT--
bool(true)
bool(true)
bool(true)
bool(false)
bool(true)
string(8) "msg<br/>"
string(4) "msg!"
string(4) "msg?"
