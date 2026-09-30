<?php
// Semantic battery for `.=`: every case prints; output must match the oracle.
declare(strict_types=0);
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
function line($l, ...$v) { echo $l, ': '; foreach ($v as $x) { echo var_export($x, true), ' '; } echo "\n"; }

// 1. COW: a copy taken before the append must not see it
$a = 'ab'; $a .= 'c'; $b = $a; $a .= 'd'; $c = $a; $c .= 'e'; $a .= 1; $a .= 2.5; $a .= true; $a .= false; $a .= null;
line('cow', $a, $b, $c);
// 2. literal (const pool) strings must never be modified
function lit() { $s = 'lit'; $s .= 'X'; return $s; }
line('lit', lit(), lit(), lit());
// 3. self-append and overlapping
$s = 'xy'; $s .= $s; $s .= $s; line('self', $s);
// 4. operand kinds
$s = ''; foreach ([0, -0, 7, -12, PHP_INT_MAX, PHP_INT_MIN, 1.0, 0.1 + 0.2, -0.0, 1e100, 1.5e-7, NAN, INF, -INF, true, false, null, '', 'str'] as $v) { $s .= $v; $s .= '|'; }
line('kinds', $s);
// 5. non-string targets
$n = 5; $n .= 3; $f = 1.5; $f .= 'x'; $t = true; $t .= 1; $u = null; $u .= 'z'; $i = 10; $i .= 20;
line('nonstr', $n, $f, $t, $u, $i);
// 6. by-reference parameter, global, static, closure capture
function byref(&$o, $v) { $o .= $v; }
$r = 'r'; $keep = $r; byref($r, 1); byref($r, 'two'); byref($r, 3.5); line('byref', $r, $keep);
function glob_append() { global $g; $g .= 'g'; $g .= 9; }
$g = 'G'; $gk = $g; glob_append(); glob_append(); line('global', $g, $gk);
function static_append() { static $acc = ''; $acc .= 'a'; $acc .= 1; return $acc; }
$x1 = static_append(); $x2 = static_append(); line('static', $x1, $x2);
$cap = 'c'; $capk = $cap; $fn = function () use (&$cap) { $cap .= 'x'; $cap .= 7; }; $fn(); $fn(); line('capture', $cap, $capk);
// 7. two references to the same cell, and a reference broken by unset
$p = 'p'; $q = &$p; $q .= 'q'; $p .= 1; line('alias', $p, $q); unset($q); $p .= 'z'; line('alias2', $p);
// 8. foreach by reference
$arr = ['a', 'b', 'c']; $arrk = $arr; foreach ($arr as $k => &$v) { $v .= $k; $v .= '!'; } unset($v); line('foreach', $arr, $arrk);
// 9. array elements, nested, copy of the array
$h = ['k' => 'v', 'n' => 1, 'z' => null, 'l' => [ 'm' => 'deep' ]]; $hk = $h;
$h['k'] .= 'x'; $h['k'] .= 2; $h['n'] .= 'y'; $h['z'] .= 'w'; $h['l']['m'] .= '!'; $h['new'] = 'N'; $h['new'] .= 1;
line('array', $h, $hk);
$e = ['s' => 'E']; $er = &$e['s']; $er .= 'r'; $e['s'] .= 's'; line('elemref', $e, $er); unset($er);
$l = ['x' => 'a']; $l2 = $l; $l['x'] .= 'b'; $l2['x'] .= 'c'; line('arrcow', $l, $l2);
// 10. properties
$o = new B; $ok = $o->s; $o->s .= 'x'; $o->s .= 3; $o2 = clone $o; $o->s .= 'y'; line('prop', $o->s, $ok, $o2->s, $o->app('z'), $o->app(4));
$o->ts .= 1; $o->ts .= 'q'; $o->ns .= 'n'; $o->u .= 5; line('typed', $o->ts, $o->ns, $o->u);
$o->dyn = 'd'; $o->dyn .= 'D'; $o->dyn .= 1; line('dynprop', $o->dyn);
$pr = &$o->s; $pr .= 'R'; $o->s .= 'S'; line('propref', $o->s, $pr); unset($pr);
$o->parts['a'] .= 'x'; $o->parts['a'] .= 1; $o->parts['n'] .= 'y'; $o->parts['z'] .= 'z'; $pk = $o->parts; $o->parts['a'] .= '!'; line('propdim', $o->parts, $pk);
try { $o->ro .= 'x'; } catch (Error $e) { line('readonly', get_class($e), $e->getMessage(), $o->ro); }
// 11. static properties
$sk = B::$st; B::$st .= 's'; B::$st .= 1; $cn = 'B'; $cn::$st .= 'd'; B::$tst .= 2; line('static_prop', B::$st, $sk, B::$tst);
// 12. magic, __toString, arrays as operand (must keep diagnostics and call order)
$m = new M; $m->v .= 'x'; $m->v .= 1; echo "\n"; line('magic', $m->v);
$s = 's'; $s .= new TS; line('tostring', $s);
$s = 's'; $s .= @[1]; line('arrayrhs', $s);
// 13. expression value of `.=`
$s = 'a'; $r1 = ($s .= 'b'); $r2 = ($s .= 5); $r1 .= 'Z'; line('value', $s, $r1, $r2);
$o->s = 'q'; $v1 = ($o->s .= 'w'); $v1 .= '!'; line('value_prop', $o->s, $v1);
$h = ['k' => 'q']; $v2 = ($h['k'] .= 'w'); $v2 .= '!'; line('value_dim', $h['k'], $v2);
B::$st = 'q'; $v3 = (B::$st .= 'w'); $v3 .= '!'; line('value_static', B::$st, $v3);
// 14. string used as array key and in a hash after being extended in place
$k = 'ke'; $k .= 'y'; $map = [$k => 1]; $k .= '2'; $map[$k] = 2; line('hashkey', $map, isset($map['key']), isset($map['key2']), $map['key'] ?? null);
$k2 = 'a'; $set = []; for ($i = 0; $i < 5; $i++) { $k2 .= $i; $set[$k2] = $i; } line('hashkeys', $set);
// 15. long loop with interleaved copies
$s = ''; $snap = []; for ($i = 0; $i < 2000; $i++) { $s .= $i; if ($i % 500 === 0) { $snap[] = $s; } } line('loop', strlen($s), md5($s), array_map('strlen', $snap), md5(implode(',', $snap)));
