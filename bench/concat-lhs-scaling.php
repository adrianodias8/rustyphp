<?php
// Growth probe for `.=` by LEFT-hand form, string right-hand operand.
// Linear implementations double with N; quadratic ones quadruple.
class Buf { public $s = ''; public static $st = ''; public array $parts = ['a' => '']; }
function by_ref(&$out, $n) { for ($i = 0; $i < $n; $i++) { $out .= 'xxxxxxxx'; } }
function with_global($n) { global $g; for ($i = 0; $i < $n; $i++) { $g .= 'xxxxxxxx'; } }
function with_static($n) { static $acc = ''; $acc = ''; for ($i = 0; $i < $n; $i++) { $acc .= 'xxxxxxxx'; } return strlen($acc); }
function t(callable $f): float { $t0 = hrtime(true); $f(); return (hrtime(true) - $t0) / 1e6; }

foreach ([50000, 100000, 200000, 400000] as $n) {
    $r = [];
    $r['local'] = t(function () use ($n) { $s = ''; for ($i = 0; $i < $n; $i++) { $s .= 'xxxxxxxx'; } });
    $r['ref_param'] = t(function () use ($n) { $s = ''; by_ref($s, $n); });
    $r['global'] = t(function () use ($n) { $GLOBALS['g'] = ''; with_global($n); });
    $r['static_var'] = t(function () use ($n) { with_static($n); });
    $r['property'] = t(function () use ($n) { $o = new Buf; for ($i = 0; $i < $n; $i++) { $o->s .= 'xxxxxxxx'; } });
    $r['static_prop'] = t(function () use ($n) { Buf::$st = ''; for ($i = 0; $i < $n; $i++) { Buf::$st .= 'xxxxxxxx'; } });
    $r['array_elem'] = t(function () use ($n) { $a = ['k' => '']; for ($i = 0; $i < $n; $i++) { $a['k'] .= 'xxxxxxxx'; } });
    $r['prop_array_elem'] = t(function () use ($n) { $o = new Buf; for ($i = 0; $i < $n; $i++) { $o->parts['a'] .= 'xxxxxxxx'; } });
    $r['closure_use_ref'] = t(function () use ($n) { $s = ''; $f = function () use (&$s) { $s .= 'xxxxxxxx'; }; for ($i = 0; $i < $n; $i++) { $f(); } });
    $r['interp_rhs'] = t(function () use ($n) { $s = ''; for ($i = 0; $i < $n; $i++) { $s .= "x{$i}y"; } });
    $r['concat_expr_rhs'] = t(function () use ($n) { $s = ''; for ($i = 0; $i < $n; $i++) { $s .= 'a' . $i . 'b'; } });
    $r['self_assign_concat'] = t(function () use ($n) { $s = ''; for ($i = 0; $i < $n; $i++) { $s = $s . 'xxxxxxxx'; } });
    echo "n=$n";
    foreach ($r as $k => $ms) { printf(" %s=%.2f", $k, $ms); }
    echo "\n";
}
