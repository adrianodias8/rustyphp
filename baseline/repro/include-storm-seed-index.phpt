--TEST--
Include storm: the seed class index cache must be invisible (conditionals, late declarations, aliases, eval)
--DESCRIPTION--
The VM caches the lowering's seed class index across includes and extends it
incrementally when only appended classes were registered in between
(vm/mod.rs seed_class_index). Every event that must force a rebuild happens
here: a guarded (conditional) class declared at run time, a conditional
declared only on a later re-include, a class_alias, duplicate conditional
declarations in two branches, an eval against the image, and a run of
autoloaded files in between. Expected output is the oracle's (PHP 8.5.7).
--FILE--
<?php
$d = sys_get_temp_dir() . "/phpr_seed_storm_" . getmypid();
@mkdir($d);
$w = function ($n, $src) use ($d) { file_put_contents("$d/$n.php", $src); };
$w("a", '<?php if (!class_exists("X", false)) { class X { const K = "x-a"; } } if (defined("WANT_Z")) { class Z { const K = "z"; } }');
$w("b", '<?php class Y extends X { function k() { return self::K . "/y"; } }');
$w("c", '<?php class C extends Y implements Countable { function count(): int { return 3; } } class D extends Legacy {}');
$w("z", '<?php class UsesZ extends Z {}');
$w("dup", '<?php if (PHP_VERSION_ID > 0) { class Dup { const V = 1; } } else { class Dup { const V = 2; } }');
$w("usedup", '<?php class UseDup extends Dup { function v() { return self::V; } }');
for ($i = 1; $i <= 40; $i++) { $w("gen$i", "<?php class Gen$i { public static \$n = $i; function id() { return self::\$n; } }"); }
spl_autoload_register(function ($c) use ($d) { $f = "$d/" . strtolower($c) . ".php"; if (is_file($f)) require $f; });
require "$d/a.php";
require "$d/b.php";
echo (new Y)->k(), "\n";
class_alias("Y", "Legacy");
require "$d/c.php";
echo count(new C), " ", get_parent_class("D"), " ", (new D)->k(), "\n";
$s = 0; for ($i = 1; $i <= 40; $i++) { $cls = "Gen$i"; $s += (new $cls)->id(); } echo $s, "\n";
echo class_exists("Z", false) ? "Z declared" : "Z absent", "\n";
define("WANT_Z", 1);
require "$d/a.php";
echo class_exists("Z", false) ? "Z declared" : "Z absent", "\n";
require "$d/z.php";
echo get_parent_class("UsesZ"), " ", UsesZ::K, "\n";
require "$d/dup.php"; require "$d/usedup.php";
echo (new UseDup)->v(), "\n";
eval('class E extends UseDup { function w() { return self::V + 10; } }');
echo (new E)->w(), "\n";
echo implode(",", array_map(fn($c) => (int) class_exists($c, false), ["X","Y","C","D","Legacy","Z","UsesZ","Dup","UseDup","E","Gen40"])), "\n";
$s = 0; for ($i = 1; $i <= 40; $i++) { $cls = "Gen$i"; $s += $cls::$n; } echo $s, "\n";
foreach (glob("$d/*.php") as $f) { unlink($f); } rmdir($d);
?>
--EXPECT--
x-a/y
3 Y x-a/y
820
Z absent
Z declared
Z z
1
11
1,1,1,1,1,1,1,1,1,1,1
820
