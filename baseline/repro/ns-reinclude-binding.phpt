--TEST--
Namespaced call sites of a re-included file bind afresh; its functions keep theirs
--DESCRIPTION--
PHP compiles a fresh op array, with an empty runtime cache, each time a file is
included: the top-level code and the closures of a re-included file bind
shadows declared since the previous include, while a function the FIRST include
declared keeps the bindings it made. phpr serves the repeat include from its
unit cache with the same compiled module, so it resets the namespaced-call
sites of the unit's top-level code and closures (Module::reset_ns_sites).
Also: a binding made while one unit runs holds when the site runs from
another. Expected output is the oracle's (PHP 8.5.7).
--FILE--
<?php
namespace App;
$d = sys_get_temp_dir() . "/phpr_reinc_" . getmypid(); @mkdir($d);
file_put_contents("$d/t.php", '<?php namespace App;
$a = [2, 1]; sort($a);
echo "top: ", ucwords("a b"), " ", implode(",", $a), " ", sprintf("%02d", 7), " ";
$c = function () { return strrev("ab") . implode("", array_reverse([1, 2])); };
echo "closure: ", $c(), " ";
if (!function_exists("App\\\\tf")) { function tf() { $b = [2, 1]; sort($b); return lcfirst("TF") . implode("", $b) . str_repeat("x", 2); } }
echo "fn: ", tf(), "\n";');
include "$d/t.php";
include "$d/t.php";
eval('namespace App;
function ucwords($s) { return "S-ucwords"; } function sort(array &$a) { $a = ["S-sort"]; } function sprintf(...$a) { return "S-sprintf"; }
function strrev($s) { return "S-strrev"; } function array_reverse($a) { return ["S-rev"]; }
function lcfirst($s) { return "S-lcfirst"; } function str_repeat(...$a) { return "S-rep"; }');
// A fresh include executes a fresh op array: its top-level code and its closures bind the
// shadows; the function declared by the FIRST include keeps the bindings it already made.
include "$d/t.php";
for ($i = 0; $i < 2; $i++) include "$d/t.php";
unlink("$d/t.php"); rmdir($d);
// Functions of the main script called from an included unit, and a unit's function called
// back from the main script: the binding made in one context holds in the other.
function helper($v) { return "helper:" . $v; }
file_put_contents("$d.u.php", '<?php namespace App;
function from_unit($v) { return helper($v) . strtolower("|U") . implode("", array_map("strrev", ["ab"])); }
echo "in unit: ", helper(1), " ", from_unit(2), "\n";');
include "$d.u.php"; unlink("$d.u.php");
echo "in main: ", helper(3), " ", from_unit(4), "\n";
$g = function () { return from_unit(5); }; echo "closure: ", $g(), "\n";
?>
--EXPECT--
top: A B 1,2 07 closure: ba21 fn: tF12xx
top: A B 1,2 07 closure: ba21 fn: tF12xx
top: S-ucwords S-sort S-sprintf closure: S-strrevS-rev fn: tF12xx
top: S-ucwords S-sort S-sprintf closure: S-strrevS-rev fn: tF12xx
top: S-ucwords S-sort S-sprintf closure: S-strrevS-rev fn: tF12xx
in unit: helper:1 helper:2|uba
in main: helper:3 helper:4|uba
closure: helper:5|uba
