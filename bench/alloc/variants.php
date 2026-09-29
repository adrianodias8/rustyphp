<?php
// Allocation probes for PLAN.md §2.2. One variant per run:
//   phpr variants.php <variant> <n>
// Run each at two values of n under the mem-census build; the difference in
// allocator calls divided by the difference in n is allocations per iteration.
$variant = $argv[1];
$n = (int) $argv[2];

function arith_fn(int $n): int {
    $s = 0;
    for ($i = 0; $i < $n; $i++) { $s = ($s + $i * 3) % 1000003; }
    return $s;
}
function array_write_fn(int $n): int {
    $a = [0, 0, 0, 0, 0, 0, 0, 0];
    $s = 0;
    for ($i = 0; $i < $n; $i++) { $s = ($s + $i * 3) % 1000003; $a[$i & 7] = $s; }
    return $s + array_sum($a);
}
function array_read_fn(int $n): int {
    $a = [1, 2, 3, 4, 5, 6, 7, 8];
    $s = 0;
    for ($i = 0; $i < $n; $i++) { $s += $a[$i & 7]; }
    return $s;
}
function array_append_fn(int $n): int {
    $a = [];
    for ($i = 0; $i < $n; $i++) { $a[] = $i; }
    return count($a);
}
function assoc_write_fn(int $n): int {
    $a = ['k0' => 0, 'k1' => 0];
    for ($i = 0; $i < $n; $i++) { $a['k1'] = $i; }
    return $a['k1'];
}
function float_fn(int $n): float {
    $s = 0.5;
    for ($i = 0; $i < $n; $i++) { $s = $s * 1.0000001 + 0.25; }
    return $s;
}
function call_fn(int $n): int {
    $s = 0;
    for ($i = 0; $i < $n; $i++) { $s += add1($i); }
    return $s;
}
function add1(int $x): int { return $x + 1; }
class P { public int $v = 0; public function inc(): void { $this->v++; } }
function prop_write_fn(int $n): int {
    $o = new P();
    for ($i = 0; $i < $n; $i++) { $o->v = $i; }
    return $o->v;
}
function method_fn(int $n): int {
    $o = new P();
    for ($i = 0; $i < $n; $i++) { $o->inc(); }
    return $o->v;
}
function concat_fn(int $n): int {
    $t = 0;
    for ($i = 0; $i < $n; $i++) { $x = 'id-' . $i; $t += strlen($x); }
    return $t;
}
function foreach_fn(int $n): int {
    $a = [1, 2, 3, 4, 5, 6, 7, 8];
    $s = 0;
    for ($i = 0; $i < $n; $i++) { foreach ($a as $v) { $s += $v; } }
    return $s;
}
function builtin_fn(int $n): int {
    $s = 0;
    for ($i = 0; $i < $n; $i++) { $s += abs($i); }
    return $s;
}

switch ($variant) {
    case 'empty_global':
        for ($i = 0; $i < $n; $i++) { }
        echo $i, "\n"; break;
    case 'arith_global':
        $s = 0;
        for ($i = 0; $i < $n; $i++) { $s = ($s + $i * 3) % 1000003; }
        echo $s, "\n"; break;
    case 'array_write_global':
        $a = [0, 0, 0, 0, 0, 0, 0, 0];
        $s = 0;
        for ($i = 0; $i < $n; $i++) { $s = ($s + $i * 3) % 1000003; $a[$i & 7] = $s; }
        echo $s + array_sum($a), "\n"; break;
    case 'arith_fn':        echo arith_fn($n), "\n"; break;
    case 'array_write_fn':  echo array_write_fn($n), "\n"; break;
    case 'array_read_fn':   echo array_read_fn($n), "\n"; break;
    case 'array_append_fn': echo array_append_fn($n), "\n"; break;
    case 'assoc_write_fn':  echo assoc_write_fn($n), "\n"; break;
    case 'float_fn':        echo float_fn($n), "\n"; break;
    case 'call_fn':         echo call_fn($n), "\n"; break;
    case 'prop_write_fn':   echo prop_write_fn($n), "\n"; break;
    case 'method_fn':       echo method_fn($n), "\n"; break;
    case 'concat_fn':       echo concat_fn($n), "\n"; break;
    case 'foreach_fn':      echo foreach_fn($n), "\n"; break;
    case 'builtin_fn':      echo builtin_fn($n), "\n"; break;
    default: fwrite(STDERR, "unknown variant\n"); exit(2);
}
