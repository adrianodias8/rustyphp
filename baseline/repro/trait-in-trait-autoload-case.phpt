--TEST--
A trait used by an included trait is autoloaded under its name as written (PSR-4 is case-sensitive)
--FILE--
<?php
spl_autoload_register(function ($c) {
    echo "autoload($c)\n";
    if ($c === "Inner\\T1") {
        eval('namespace Inner; trait T1 { public function hi() { return "hi from T1"; } }');
    }
});
$f = sys_get_temp_dir() . "/trait-in-trait-" . getmypid() . ".inc";
file_put_contents($f, '<?php
namespace Outer;
trait T2 {
    use \Inner\T1 { \Inner\T1::hi as parentHi; }
    public function hello() { return $this->parentHi() . " via T2"; }
}');
include $f;
unlink($f);
class C { use \Outer\T2; }
echo (new C)->hello(), "\n";
?>
--EXPECT--
autoload(Inner\T1)
hi from T1 via T2
