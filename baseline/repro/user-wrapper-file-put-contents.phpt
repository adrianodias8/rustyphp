--TEST--
file_put_contents() on a user stream wrapper (LOCK_EX refused, plain write through stream_write)
--FILE--
<?php
class LW {
    public $context; private $h;
    function stream_open($path, $mode, $options, &$opened) { $this->h = fopen("" . sys_get_temp_dir() . "/lw-" . getmypid() . "-" . md5($path), $mode); return (bool) $this->h; }
    function stream_write($d) { return fwrite($this->h, $d); }
    function stream_close() { fclose($this->h); }
    function stream_lock($op) { return flock($this->h, $op); }
    function url_stat($p, $f) { $r = @stat("" . sys_get_temp_dir() . "/lw-" . getmypid() . "-" . md5($p)); return $r ?: false; }
}
stream_wrapper_register("pub", "LW");
var_dump(file_put_contents("pub://.htaccess", "deny", LOCK_EX));
var_dump(file_put_contents("pub://plain", "x"));
var_dump(file_exists("pub://.htaccess"));
foreach (glob(sys_get_temp_dir() . "/lw-" . getmypid() . "-*") as $x) unlink($x);
?>
--EXPECTF--

Warning: file_put_contents(): Exclusive locks may only be set for regular files in %s on line 11
bool(false)
int(1)
bool(false)
