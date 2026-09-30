--TEST--
ini_set('precision') takes effect at run time
--FILE--
<?php
echo ini_get('precision'), "|", ini_set('precision', '5') === false ? 'false' : 'ok', "|", ini_get('precision'), "|", 1 / 3, "\n";
?>
--EXPECTF--
14|ok|5|0.33333
