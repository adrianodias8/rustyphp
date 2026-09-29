<?php
// PLAN §2.1: packed arrays (1M), string-keyed arrays (200k), nested arrays with
// COW copies, array_map/filter/usort with closures.
require __DIR__ . '/lib/harness.php';

const N_PACKED = 1000000;
const N_ASSOC  = 200000;

bench('packed_build_1m', function () {
    $a = [];
    for ($i = 0; $i < N_PACKED; $i++) { $a[] = $i; }
    return count($a);
});

$packed = range(0, N_PACKED - 1);

bench('packed_foreach_sum_1m', function () use ($packed) {
    $s = 0;
    foreach ($packed as $v) { $s += $v; }
    return $s;
});

bench('packed_foreach_kv_1m', function () use ($packed) {
    $s = 0;
    foreach ($packed as $k => $v) { $s += $k ^ $v; }
    return $s;
});

bench('packed_index_read_1m', function () use ($packed) {
    $s = 0;
    for ($i = 0; $i < N_PACKED; $i++) { $s += $packed[$i]; }
    return $s;
});

bench('packed_index_write_1m', function () use ($packed) {
    $a = $packed;                       // COW: first write separates 1M elements
    for ($i = 0; $i < N_PACKED; $i++) { $a[$i] = $a[$i] * 2 + 1; }
    return $a[N_PACKED - 1] + count($a);
});

bench('packed_foreach_byref_1m', function () use ($packed) {
    $a = $packed;
    foreach ($a as &$v) { $v++; }
    unset($v);
    return $a[0] + $a[N_PACKED - 1];
});

bench('assoc_build_200k', function () {
    $a = [];
    for ($i = 0; $i < N_ASSOC; $i++) { $a['key_' . $i] = $i; }
    return count($a);
});

$assoc = [];
for ($i = 0; $i < N_ASSOC; $i++) { $assoc['key_' . $i] = $i; }

bench('assoc_lookup_200k_x5', function () use ($assoc) {
    $s = 0;
    for ($r = 0; $r < 5; $r++) {
        for ($i = 0; $i < N_ASSOC; $i++) { $s += $assoc['key_' . $i]; }
    }
    return $s;
});

bench('assoc_isset_miss_200k_x5', function () use ($assoc) {
    $n = 0;
    for ($r = 0; $r < 5; $r++) {
        for ($i = 0; $i < N_ASSOC; $i++) { if (isset($assoc['nope_' . $i])) { $n++; } }
    }
    return $n;
});

bench('assoc_foreach_200k_x5', function () use ($assoc) {
    $s = 0;
    for ($r = 0; $r < 5; $r++) {
        foreach ($assoc as $k => $v) { $s += $v + strlen($k); }
    }
    return $s;
});

bench('assoc_unset_reinsert_200k', function () use ($assoc) {
    $a = $assoc;
    for ($i = 0; $i < N_ASSOC; $i += 2) { unset($a['key_' . $i]); }
    for ($i = 0; $i < N_ASSOC; $i += 2) { $a['key_' . $i] = -$i; }
    return count($a);
});

bench('nested_build_rows_100k', function () {
    $rows = [];
    for ($i = 0; $i < 100000; $i++) {
        $rows[] = ['id' => $i, 'name' => 'n' . $i, 'tags' => [$i, $i + 1, $i + 2], 'meta' => ['a' => 1, 'b' => [2, 3]]];
    }
    return count($rows);
});

$rows = [];
for ($i = 0; $i < 100000; $i++) {
    $rows[] = ['id' => $i, 'name' => 'n' . $i, 'tags' => [$i, $i + 1, $i + 2], 'meta' => ['a' => 1, 'b' => [2, 3]]];
}

bench('nested_cow_copy_modify_100k', function () use ($rows) {
    $s = 0;
    foreach ($rows as $row) {
        $copy = $row;                   // shares
        $copy['meta']['b'][] = $row['id'];  // separates 3 levels
        $copy['tags'][0] = -1;
        $s += count($copy['meta']['b']) + $copy['tags'][0] + $row['tags'][0];
    }
    return $s;
});

bench('nested_pass_by_value_100k', function () use ($rows) {
    $f = function (array $r) { $r['id']++; return $r['id']; };
    $s = 0;
    foreach ($rows as $row) { $s += $f($row); }
    return $s;
});

bench('array_map_closure_1m', function () use ($packed) {
    $r = array_map(fn($x) => $x * 2, $packed);
    return $r[N_PACKED - 1];
});

bench('array_filter_closure_1m', function () use ($packed) {
    $r = array_filter($packed, fn($x) => ($x & 1) === 0);
    return count($r);
});

bench('usort_closure_200k', function () {
    mt_srand(42);
    $a = [];
    for ($i = 0; $i < 200000; $i++) { $a[] = mt_rand(); }
    usort($a, fn($x, $y) => $x <=> $y);
    return $a[0] . ':' . $a[199999];
});

bench('sort_builtin_1m', function () {
    mt_srand(7);
    $a = [];
    for ($i = 0; $i < N_PACKED; $i++) { $a[] = mt_rand(); }
    sort($a);
    return $a[0] . ':' . $a[N_PACKED - 1];
});

bench('array_merge_slice_keys_x50', function () use ($packed, $assoc) {
    $s = 0;
    for ($i = 0; $i < 50; $i++) {
        $m = array_merge(array_slice($packed, 0, 20000), array_keys($assoc));
        $s += count($m);
    }
    return $s;
});

bench('in_array_array_search_2k_x2k', function () {
    $a = range(0, 1999);
    $n = 0;
    for ($i = 0; $i < 2000; $i++) {
        if (in_array($i, $a, true)) { $n++; }
        $n += (int) array_search($i, $a, true);
    }
    return $n;
});
