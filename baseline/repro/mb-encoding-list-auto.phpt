--TEST--
The encoding list "auto" expands to ASCII, UTF-8 (mb_detect_encoding, mb_convert_encoding)
--FILE--
<?php
var_dump(mb_detect_encoding("admin", "auto"), mb_detect_encoding("caf\xc3\xa9", "auto"), mb_detect_encoding("caf\xe9", "auto"), mb_detect_encoding("x", "AUTO, ISO-8859-1"));
var_dump(mb_convert_encoding("caf\xc3\xa9", "UTF-8", "auto"));
?>
--EXPECT--
string(5) "ASCII"
string(5) "UTF-8"
string(5) "ASCII"
string(5) "ASCII"
string(5) "café"
