<?php
// Growth probe for `$s .= <non-string>`: doubles N four times. A linear
// implementation doubles its time with N; a quadratic one quadruples it.
// (`$s .= 'x'` with a string operand is measured alongside as the control.)
foreach ([50000, 100000, 200000, 400000] as $n) {
    $t0 = hrtime(true);
    $s = '';
    for ($i = 0; $i < $n; $i++) { $s .= $i; }
    $int_ms = (hrtime(true) - $t0) / 1e6;
    $len = strlen($s);

    $t0 = hrtime(true);
    $s = '';
    for ($i = 0; $i < $n; $i++) { $s .= (string) $i; }
    $cast_ms = (hrtime(true) - $t0) / 1e6;

    $t0 = hrtime(true);
    $s = '';
    for ($i = 0; $i < $n; $i++) { $s .= 1.5; }
    $float_ms = (hrtime(true) - $t0) / 1e6;

    printf("n=%d bytes=%d append_int_ms=%.2f append_cast_string_ms=%.2f append_float_ms=%.2f\n", $n, $len, $int_ms, $cast_ms, $float_ms);
}
