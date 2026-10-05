--TEST--
Method return releases locals, operands and $this in Zend's order; as-is hint checks keep coercion and errors
--FILE--
<?php
class D { function __construct(public string $n) {} function __destruct() { echo "~{$this->n} "; } }
class M {
    function __destruct() { echo "~M "; }
    function run(D $a, ?D $b, int $i, float $f, string $s, bool $t, array $arr, ?object $o): string {
        $local = new D("local");
        return "$i|$f|$s|" . ($t ? 'T' : 'F') . '|' . count($arr) . '|' . ($b ? $b->n : 'null');
    }
    function self(): static { return $this; }
    function num(): int { return "42"; }
    function bad(): int { return "x"; }
}
// (Each result is stored first: destruction at the return itself is D-24.)
$r = (new M)->run(new D("arg"), null, 7, 1, "s", true, [1, 2], null);
echo $r, "\n";
$r = (new M)->run(new D("a2"), new D("b2"), "8", 2.5, 3, 0, [], new stdClass);
echo $r, "\n";
$r = get_class((new M)->self());
echo $r, "\n";
$r = (new M)->num();
var_dump($r);
try { (new M)->bad(); } catch (TypeError $e) { $m = $e->getMessage(); }
echo $m, "\n";
try { (new M)->run(new stdClass, null, 1, 1.0, "", true, [], null); } catch (TypeError $e) { $m = $e->getMessage(); }
echo substr($m, 0, 60), "\n";
class P { public int $i = 0; public ?D $d = null; public float $f = 0.0; }
$p = new P; $p->i = "5"; $p->f = 3; $p->d = new D("prop"); $p->d = null;
var_dump($p->i, $p->f);
try { $p->i = "nope"; } catch (TypeError $e) { echo $e->getMessage(), "\n"; }
function f(int ...$xs): int { return array_sum($xs); }
echo f(1, "2", 3), "\n";
echo "end\n";
?>
--EXPECT--
~arg ~local ~M 7|1|s|T|2|null
~a2 ~b2 ~local ~M 8|2.5|3|F|0|b2
~M M
~M int(42)
~M M::bad(): Return value must be of type int, string returned
~M M::run(): Argument #1 ($a) must be of type D, stdClass given
~prop int(5)
float(3)
Cannot assign string to property P::$i of type int
6
end
