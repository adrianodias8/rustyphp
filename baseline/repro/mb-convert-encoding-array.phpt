--TEST--
mb_convert_encoding() converts an array element by element, keys included
--FILE--
<?php
$in = ["caf\xe9" => "na\xefve", "n" => 42, "nested" => ["\xe0 la" => "cr\xe8me"], "f" => 1.5, "b" => true, "z" => null];
$out = mb_convert_encoding($in, "UTF-8", "ISO-8859-1");
var_dump($out);
var_dump(mb_convert_encoding(["abc   ", "de"], "utf8", "ASCII"));
?>
--EXPECT--
array(6) {
  ["café"]=>
  string(6) "naïve"
  ["n"]=>
  int(42)
  ["nested"]=>
  array(1) {
    ["à la"]=>
    string(6) "crème"
  }
  ["f"]=>
  float(1.5)
  ["b"]=>
  bool(true)
  ["z"]=>
  NULL
}
array(2) {
  [0]=>
  string(6) "abc   "
  [1]=>
  string(2) "de"
}
