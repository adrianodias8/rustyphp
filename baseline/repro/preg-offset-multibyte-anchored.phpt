--TEST--
preg_match() byte offsets on a non-UTF-8-mode pattern with a multibyte subject (anchored /A, OFFSET_CAPTURE)
--FILE--
<?php
$s = "<em>haven’t x</em>{% endtrans %}";
$p = strpos($s, "{%");
var_dump($p);
var_dump(preg_match("/\{%\s*/A", $s, $m, 0, $p), $m);
var_dump(preg_match("/endtrans/", $s, $m, PREG_OFFSET_CAPTURE), $m[0][1]);
var_dump(preg_match_all("/\{%/", $s, $m, PREG_OFFSET_CAPTURE), $m[0][0][1]);
var_dump(preg_match("/[a-zA-Z_\x7f-\xff][a-zA-Z0-9_\x7f-\xff]*/A", $s, $m, 0, $p + 3), $m);
var_dump(preg_split("/(\{%)/", $s, -1, PREG_SPLIT_OFFSET_CAPTURE | PREG_SPLIT_DELIM_CAPTURE)[1][1]);
var_dump(preg_match("/\s+/A", $s, $m, 0, 22));
?>
--EXPECT--
int(20)
int(1)
array(1) {
  [0]=>
  string(3) "{% "
}
int(1)
int(23)
int(1)
int(20)
int(1)
array(1) {
  [0]=>
  string(8) "endtrans"
}
int(20)
int(1)
