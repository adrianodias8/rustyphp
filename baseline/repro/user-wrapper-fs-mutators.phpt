--TEST--
chmod/touch/chown/chgrp/unlink/rename/mkdir/rmdir on a user stream wrapper call its methods (or warn when missing)
--FILE--
<?php
class Rec {
    public $context;
    function stream_metadata($p, $o, $v) { echo "metadata(", $p, ", ", $o, ", ", json_encode($v), ")\n"; return true; }
    function unlink($p) { echo "unlink($p)\n"; return true; }
    function rename($a, $b) { echo "rename($a, $b)\n"; return true; }
    function mkdir($p, $m, $o) { echo "mkdir($p, ", decoct($m), ", $o)\n"; return true; }
    function rmdir($p, $o) { echo "rmdir($p, $o)\n"; return true; }
}
class Bare { public $context; }
stream_wrapper_register("rec", "Rec");
stream_wrapper_register("bare", "Bare");
var_dump(chmod("rec://a", 0444), touch("rec://a", 100, 200), chown("rec://a", 0), chown("rec://a", "root"), chgrp("rec://a", 0), chgrp("rec://a", "root"));
var_dump(unlink("rec://a"), rename("rec://a", "rec://b"), mkdir("rec://d"), mkdir("rec://d/e", 0755, true), rmdir("rec://d"));
var_dump(chmod("bare://a", 0444), unlink("bare://a"), mkdir("bare://d"), rmdir("bare://d"), rename("bare://a", "bare://b"));
?>
--EXPECTF--
metadata(rec://a, 6, 292)
metadata(rec://a, 1, [100,200])
metadata(rec://a, 3, 0)
metadata(rec://a, 2, "root")
metadata(rec://a, 5, 0)
metadata(rec://a, 4, "root")
bool(true)
bool(true)
bool(true)
bool(true)
bool(true)
bool(true)
unlink(rec://a)
rename(rec://a, rec://b)
mkdir(rec://d, 777, 8)
mkdir(rec://d/e, 755, 9)
rmdir(rec://d, 8)
bool(true)
bool(true)
bool(true)
bool(true)
bool(true)

Warning: chmod(): Bare::stream_metadata is not implemented! in %s on line 15

Warning: unlink(): Bare::unlink is not implemented! in %s on line 15

Warning: mkdir(): Bare::mkdir is not implemented! in %s on line 15

Warning: rmdir(): Bare::rmdir is not implemented! in %s on line 15

Warning: rename(): Bare::rename is not implemented! in %s on line 15
bool(false)
bool(false)
bool(false)
bool(false)
bool(false)
