--TEST--
`.=` semantics across every left-hand form and operand kind (in-place append)
--DESCRIPTION--
`$x .= y` was O(length) per append — hence quadratic per loop — for every
target but a plain local, and for every operand that was not already a string
(bench/concat-scaling.php, bench/concat-lhs-scaling.php: 12.8 s vs the
oracle's 5 ms at 400,000 appends). The in-place path must be invisible:
copy-on-write, aliases, references, typed properties, readonly, magic and
__toString all behave exactly as before. `$s .= $stringable` also called no
__toString at all (a warning and the class name instead) on any form.
--FILE--
<?php
class B {
    public $s = 'p';
    public string $ts = 'T';
    public ?string $ns = null;
    public int|string $u = 'u';
    public static $st = 'S';
    public static string $tst = 'TS';
    public array $parts = ['a' => 'A', 'n' => 5, 'z' => null];
    public readonly string $ro;
    function __construct() { $this->ro = 'RO'; }
    function app($x) { $this->s .= $x; return $this->s; }
}
class M { private $d = []; function __get($n) { echo "[get $n]"; return $this->d[$n] ?? 'm'; } function __set($n, $v) { echo "[set $n]"; $this->d[$n] = $v; } }
class TS { function __toString(): string { echo "[toString]"; return 'obj'; } }
function line($l, ...$v) { echo $l, ': '; foreach ($v as $x) { echo str_replace("\n", "", var_export($x, true)), ' '; } echo "\n"; }
function lit() { $s = 'lit'; $s .= 'X'; return $s; }
function byref(&$o, $v) { $o .= $v; }
function glob_append() { global $g; $g .= 'g'; $g .= 9; }
function static_append() { static $acc = ''; $acc .= 'a'; $acc .= 1; return $acc; }

$a = 'ab'; $a .= 'c'; $b = $a; $a .= 'd'; $c = $a; $c .= 'e'; $a .= 1; $a .= 2.5; $a .= true; $a .= false; $a .= null;
line('cow', $a, $b, $c);
line('lit', lit(), lit(), lit());
$s = 'xy'; $s .= $s; $s .= $s; line('self', $s);
$s = ''; foreach ([0, 7, -12, PHP_INT_MAX, PHP_INT_MIN, 1.0, 0.1 + 0.2, -0.0, 1e100, 1.5e-7, INF, -INF, true, false, null, '', 'str'] as $v) { $s .= $v; $s .= '|'; }
line('kinds', $s);
$n = 5; $n .= 3; $f = 1.5; $f .= 'x'; $t = true; $t .= 1; $u = null; $u .= 'z'; $i = 10; $i .= 20;
line('nonstr', $n, $f, $t, $u, $i);
$r = 'r'; $keep = $r; byref($r, 1); byref($r, 'two'); byref($r, 3.5); line('byref', $r, $keep);
$g = 'G'; $gk = $g; glob_append(); glob_append(); line('global', $g, $gk);
$x1 = static_append(); $x2 = static_append(); line('static', $x1, $x2);
$cap = 'c'; $capk = $cap; $fn = function () use (&$cap) { $cap .= 'x'; $cap .= 7; }; $fn(); $fn(); line('capture', $cap, $capk);
$p = 'p'; $q = &$p; $q .= 'q'; $p .= 1; line('alias', $p, $q); unset($q); $p .= 'z'; line('alias2', $p);
$arr = ['a', 'b', 'c']; $arrk = $arr; foreach ($arr as $k => &$v) { $v .= $k; $v .= '!'; } unset($v); line('foreach', $arr, $arrk);
$h = ['k' => 'v', 'n' => 1, 'z' => null, 'l' => ['m' => 'deep']]; $hk = $h;
$h['k'] .= 'x'; $h['k'] .= 2; $h['n'] .= 'y'; $h['z'] .= 'w'; $h['l']['m'] .= '!'; $h['new'] = 'N'; $h['new'] .= 1;
line('array', $h, $hk);
$e = ['s' => 'E']; $er = &$e['s']; $er .= 'r'; $e['s'] .= 's'; line('elemref', $e, $er); unset($er);
$l = ['x' => 'a']; $l2 = $l; $l['x'] .= 'b'; $l2['x'] .= 'c'; line('arrcow', $l, $l2);
$o = new B; $ok = $o->s; $o->s .= 'x'; $o->s .= 3; $o2 = clone $o; $o->s .= 'y'; line('prop', $o->s, $ok, $o2->s, $o->app('z'), $o->app(4));
$o->ts .= 1; $o->ts .= 'q'; $o->ns .= 'n'; $o->u .= 5; line('typed', $o->ts, $o->ns, $o->u);
$pr = &$o->s; $pr .= 'R'; $o->s .= 'S'; line('propref', $o->s, $pr); unset($pr);
$o->parts['a'] .= 'x'; $o->parts['a'] .= 1; $o->parts['n'] .= 'y'; $o->parts['z'] .= 'z'; $pk = $o->parts; $o->parts['a'] .= '!'; line('propdim', $o->parts, $pk);
try { $o->ro .= 'x'; } catch (Error $e) { line('readonly', get_class($e), $e->getMessage(), $o->ro); }
$sk = B::$st; B::$st .= 's'; B::$st .= 1; $cn = 'B'; $cn::$st .= 'd'; B::$tst .= 2; line('static_prop', B::$st, $sk, B::$tst);
$m = new M; $m->v .= 'x'; $m->v .= 1; echo "\n"; line('magic', $m->v);
$s = 's'; $s .= new TS; line('tostring_local', $s);
$o->s = 's'; $o->s .= new TS; line('tostring_prop', $o->s);
$h = ['k' => 's']; $h['k'] .= new TS; line('tostring_dim', $h['k']);
B::$st = 's'; B::$st .= new TS; line('tostring_static', B::$st);
$o->parts['a'] = 's'; $o->parts['a'] .= new TS; line('tostring_propdim', $o->parts['a']);
$ts = new TS; $ts .= 'x'; line('tostring_lhs', $ts);
$s = 'a'; $r1 = ($s .= 'b'); $r2 = ($s .= 5); $r1 .= 'Z'; line('value', $s, $r1, $r2);
$o->s = 'q'; $v1 = ($o->s .= 'w'); $v1 .= '!'; line('value_prop', $o->s, $v1);
$h = ['k' => 'q']; $v2 = ($h['k'] .= 'w'); $v2 .= '!'; line('value_dim', $h['k'], $v2);
B::$st = 'q'; $v3 = (B::$st .= 'w'); $v3 .= '!'; line('value_static', B::$st, $v3);
$k = 'ke'; $k .= 'y'; $map = [$k => 1]; $k .= '2'; $map[$k] = 2; line('hashkey', $map, isset($map['key']), isset($map['key2']), $map['key'] ?? null);
$k2 = 'a'; $set = []; for ($i = 0; $i < 5; $i++) { $k2 .= $i; $set[$k2] = $i; } line('hashkeys', $set);
$s = ''; $snap = []; for ($i = 0; $i < 2000; $i++) { $s .= $i; if ($i % 500 === 0) { $snap[] = $s; } } line('loop', strlen($s), md5($s), array_map('strlen', $snap), md5(implode(',', $snap)));
?>
--EXPECT--
cow: 'abcd12.51' 'abc' 'abcde' 
lit: 'litX' 'litX' 'litX' 
self: 'xyxyxyxy' 
kinds: '0|7|-12|9223372036854775807|-9223372036854775808|1|0.3|-0|1.0E+100|1.5E-7|INF|-INF|1||||str|' 
nonstr: '53' '1.5x' '11' 'z' '1020' 
byref: 'r1two3.5' 'r' 
global: 'Gg9g9' 'G' 
static: 'a1' 'a1a1' 
capture: 'cx7x7' 'c' 
alias: 'pq1' 'pq1' 
alias2: 'pq1z' 
foreach: array (  0 => 'a0!',  1 => 'b1!',  2 => 'c2!',) array (  0 => 'a',  1 => 'b',  2 => 'c',) 
array: array (  'k' => 'vx2',  'n' => '1y',  'z' => 'w',  'l' =>   array (    'm' => 'deep!',  ),  'new' => 'N1',) array (  'k' => 'v',  'n' => 1,  'z' => NULL,  'l' =>   array (    'm' => 'deep',  ),) 
elemref: array (  's' => 'Ers',) 'Ers' 
arrcow: array (  'x' => 'ab',) array (  'x' => 'ac',) 
prop: 'px3y' 'p' 'px3' 'px3yz' 'px3yz4' 
typed: 'T1q' 'n' 'u5' 
propref: 'px3yz4RS' 'px3yz4RS' 
propdim: array (  'a' => 'Ax1!',  'n' => '5y',  'z' => 'z',) array (  'a' => 'Ax1',  'n' => '5y',  'z' => 'z',) 
readonly: 'Error' 'Cannot modify readonly property B::$ro' 'RO' 
static_prop: 'Ss1d' 'S' 'TS2' 
[get v][set v][get v][set v]
[get v]magic: 'mx1' 
[toString]tostring_local: 'sobj' 
[toString]tostring_prop: 'sobj' 
[toString]tostring_dim: 'sobj' 
[toString]tostring_static: 'sobj' 
[toString]tostring_propdim: 'sobj' 
[toString]tostring_lhs: 'objx' 
value: 'ab5' 'abZ' 'ab5' 
value_prop: 'qw' 'qw!' 
value_dim: 'qw' 'qw!' 
value_static: 'qw' 'qw!' 
hashkey: array (  'key' => 1,  'key2' => 2,) true true 1 
hashkeys: array (  'a0' => 0,  'a01' => 1,  'a012' => 2,  'a0123' => 3,  'a01234' => 4,) 
loop: 6890 'a08232ef8a758330f8698442550157f7' array (  0 => 1,  1 => 1393,  2 => 2894,  3 => 4894,) 'a719ce229542bd69df56c920c3067f60' 
