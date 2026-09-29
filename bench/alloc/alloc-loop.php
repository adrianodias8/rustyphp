<?php
// PLAN §2.2: "a simple for loop with integer arithmetic and one array write".
// Run at two iteration counts; (allocs(N2) - allocs(N1)) / (N2 - N1) is the
// steady-state allocations per iteration, with startup cancelled out.
$n = (int) ($argv[1] ?? 1000000);
$a = [0, 0, 0, 0, 0, 0, 0, 0];
$s = 0;
for ($i = 0; $i < $n; $i++) {
    $s = ($s + $i * 3) % 1000003;
    $a[$i & 7] = $s;
}
echo $s, ' ', array_sum($a), "\n";
