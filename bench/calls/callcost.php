<?php
// bench/calls/callcost.php — what one operation costs, net of the loop that
// repeats it: user calls decomposed (frame setup + return, per argument,
// per type check, return value, method / static / closure dispatch),
// property access and array access. Each section is a loop of N iterations
// timed with hrtime; the empty loop's time is subtracted and the result
// printed as ns per operation, the best of R repetitions (php vs ferro:
// bench/calls/callcost.sh).
//   php|ferro callcost.php [N] [R]
$N = (int) ($argv[1] ?? 2000000);
$R = (int) ($argv[2] ?? 5);

function f0() {}
function f1($a) {}
function f3($a, $b, $c) {}
function f3t(int $a, int $b, int $c): void {}
function fret() { return 1; }
function fdef($a, $b = 2, $c = 3) {}
function fby(&$a) {}
function fvar(...$a) {}
function frec($n) { return $n <= 0 ? 0 : frec($n - 1); }

final class C {
    public int $ti = 0;
    public $p = 1;
    public ?C $child = null;
    public function m() {}
    public function m3($a, $b, $c) {}
    public static function s() {}
    public function getP() { return $this->p; }
}

$o = new C;
$o->child = new C;
$cl = function () {};
$arr = ['k' => 1, 'x' => 2, 3 => 4];
$list = range(0, 99);
$x = 0;

$sections = [
    'loop (baseline)'        => function ($n) { for ($i = 0; $i < $n; $i++) {} },
    'call f()'               => function ($n) { for ($i = 0; $i < $n; $i++) { f0(); } },
    'call f($a)'             => function ($n) { $a = 1; for ($i = 0; $i < $n; $i++) { f1($a); } },
    'call f($a,$b,$c)'       => function ($n) { $a = 1; for ($i = 0; $i < $n; $i++) { f3($a, $a, $a); } },
    'call f(int x3): void'   => function ($n) { $a = 1; for ($i = 0; $i < $n; $i++) { f3t($a, $a, $a); } },
    'call f() used return'   => function ($n) { for ($i = 0; $i < $n; $i++) { $r = fret(); } },
    'call f($a) 2 defaults'  => function ($n) { $a = 1; for ($i = 0; $i < $n; $i++) { fdef($a); } },
    'call f(&$a)'            => function ($n) { $a = 1; for ($i = 0; $i < $n; $i++) { fby($a); } },
    'call f(...$a) variadic' => function ($n) { $a = 1; for ($i = 0; $i < $n; $i++) { fvar($a, $a); } },
    'method $o->m()'         => function ($n) use ($o) { for ($i = 0; $i < $n; $i++) { $o->m(); } },
    'method $o->m($a,$b,$c)' => function ($n) use ($o) { $a = 1; for ($i = 0; $i < $n; $i++) { $o->m3($a, $a, $a); } },
    'getter $o->getP()'      => function ($n) use ($o) { for ($i = 0; $i < $n; $i++) { $r = $o->getP(); } },
    'static C::s()'          => function ($n) { for ($i = 0; $i < $n; $i++) { C::s(); } },
    'closure $c()'           => function ($n) use ($cl) { for ($i = 0; $i < $n; $i++) { $cl(); } },
    'builtin strlen($s)'     => function ($n) { $s = 'abc'; for ($i = 0; $i < $n; $i++) { $r = strlen($s); } },
    'builtin count($a)'      => function ($n) use ($list) { for ($i = 0; $i < $n; $i++) { $r = count($list); } },
    'recursion depth 10/10'  => function ($n) { for ($i = 0; $i < $n / 10; $i++) { frec(9); } },
    'prop read $o->p'        => function ($n) use ($o) { for ($i = 0; $i < $n; $i++) { $r = $o->p; } },
    'prop write $o->p = 1'   => function ($n) use ($o) { for ($i = 0; $i < $n; $i++) { $o->p = 1; } },
    'typed prop write $o->ti' => function ($n) use ($o) { for ($i = 0; $i < $n; $i++) { $o->ti = 1; } },
    'prop chain $o->child->p' => function ($n) use ($o) { for ($i = 0; $i < $n; $i++) { $r = $o->child->p; } },
    'isset($o->p)'           => function ($n) use ($o) { for ($i = 0; $i < $n; $i++) { $r = isset($o->p); } },
    'isset($o->child->p)'    => function ($n) use ($o) { for ($i = 0; $i < $n; $i++) { $r = isset($o->child->p); } },
    'array read $a[\'k\']'   => function ($n) use ($arr) { for ($i = 0; $i < $n; $i++) { $r = $arr['k']; } },
    'array read $a[3]'       => function ($n) use ($arr) { for ($i = 0; $i < $n; $i++) { $r = $arr[3]; } },
    'array write $a[\'k\']=1' => function ($n) use ($arr) { for ($i = 0; $i < $n; $i++) { $arr['k'] = 1; } },
    'isset($a[\'k\'])'       => function ($n) use ($arr) { for ($i = 0; $i < $n; $i++) { $r = isset($arr['k']); } },
    '$a[\'k\'] ?? 0'         => function ($n) use ($arr) { for ($i = 0; $i < $n; $i++) { $r = $arr['k'] ?? 0; } },
    'foreach, per element'   => function ($n) use ($list) { for ($i = 0; $i < $n / 100; $i++) { foreach ($list as $v) {} } },
    'string concat $s.$t'    => function ($n) { $s = 'ab'; $t = 'cd'; for ($i = 0; $i < $n; $i++) { $r = $s . $t; } },
];

$best = [];
for ($rep = 0; $rep < $R; $rep++) {
    foreach ($sections as $name => $fn) {
        $t = hrtime(true);
        $fn($N);
        $ns = (hrtime(true) - $t) / $N;
        $best[$name] = min($best[$name] ?? INF, $ns);
    }
}
$base = $best['loop (baseline)'];
foreach ($best as $name => $ns) {
    printf("%-26s %7.2f ns%s\n", $name, $name === 'loop (baseline)' ? $ns : $ns - $base, $name === 'loop (baseline)' ? ' (per iteration)' : '');
}
