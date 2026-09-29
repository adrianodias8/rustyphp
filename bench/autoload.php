<?php
// PLAN §2.1: Composer autoload + instantiate 2,000 classes from a generated
// package. Measures autoloader lookup + include + per-file compile + `new`.
//   AUTOLOAD_DIR=<dir produced by gen/gen-autoload.php + composer dump-autoload>
require __DIR__ . '/lib/harness.php';

$dir = getenv('AUTOLOAD_DIR') ?: '/scratch/autoload';
$n = (int) (getenv('AUTOLOAD_N') ?: 2000);

bench('require_composer_autoloader', function () use ($dir) {
    require $dir . '/vendor/autoload.php';
    return 1;
});

bench('autoload_instantiate_cold', function () use ($n) {
    $s = 0;
    for ($i = 0; $i < $n; $i++) {
        $cls = 'Bench\\Gen\\Pkg' . intdiv($i, 100) . '\\C' . $i;
        $o = new $cls();
        $s += $o->id() + $o->weight() + strlen($o->describe());
    }
    return $s;
});

// Second pass: every class is already loaded, so this is `new $cls` + calls
// only. cold − warm = the autoload/include/compile cost.
bench('instantiate_warm', function () use ($n) {
    $s = 0;
    for ($i = 0; $i < $n; $i++) {
        $cls = 'Bench\\Gen\\Pkg' . intdiv($i, 100) . '\\C' . $i;
        $o = new $cls();
        $s += $o->id() + $o->weight() + strlen($o->describe());
    }
    return $s;
});

printf("INFO declared_classes %d\n", count(array_filter(get_declared_classes(), fn($c) => str_starts_with($c, 'Bench\\Gen\\'))));
