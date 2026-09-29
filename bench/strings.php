<?php
// PLAN §2.1: concatenation loops, sprintf, str_replace, preg_replace,
// implode/explode, json_encode/decode of a ~5 MB structure, serialize/unserialize.
require __DIR__ . '/lib/harness.php';

bench('concat_append_1m', function () {
    $s = '';
    for ($i = 0; $i < 1000000; $i++) { $s .= 'x'; }
    return strlen($s);
});

// `.=` with a NON-string right-hand side. 200k, not 1M: under phpr this loop is
// quadratic (see bench/concat-scaling.php, which measures the growth) and at
// 1M it alone took 61.8 s, swamping every other section of this file.
bench('concat_append_int_200k', function () {
    $s = '';
    for ($i = 0; $i < 200000; $i++) { $s .= $i; }
    return strlen($s) . ':' . md5($s);
});

bench('concat_binary_temp_1m', function () {
    $n = 0;
    for ($i = 0; $i < 1000000; $i++) { $t = 'id-' . $i . '-suffix'; $n += strlen($t); }
    return $n;
});

bench('interpolation_1m', function () {
    $n = 0;
    $name = 'world';
    for ($i = 0; $i < 1000000; $i++) { $t = "hello $name #{$i}!"; $n += strlen($t); }
    return $n;
});

bench('sprintf_500k', function () {
    $n = 0;
    for ($i = 0; $i < 500000; $i++) {
        $t = sprintf('%05d|%s|%.2f|%x', $i, 'abc', $i / 7, $i);
        $n += strlen($t);
    }
    return $n;
});

bench('strlen_substr_strpos_1m', function () {
    $hay = str_repeat('lorem ipsum dolor sit amet ', 40);
    $n = 0;
    for ($i = 0; $i < 1000000; $i++) {
        $n += strlen(substr($hay, $i % 900, 16));
        $n += (int) strpos($hay, 'amet', $i % 900);
    }
    return $n;
});

$text = str_repeat("The quick brown fox jumps over the lazy dog. 12345 foo@example.com\n", 20000); // ~1.36 MB

bench('str_replace_1_3mb_x20', function () use ($text) {
    $n = 0;
    for ($i = 0; $i < 20; $i++) {
        $r = str_replace(['quick', 'lazy', 'dog'], ['slow', 'eager', 'cat'], $text);
        $n += strlen($r);
    }
    return $n . ':' . md5($r);
});

bench('strtr_array_1_3mb_x10', function () use ($text) {
    $n = 0;
    for ($i = 0; $i < 10; $i++) {
        $r = strtr($text, ['quick' => 'slow', 'lazy' => 'eager', 'dog' => 'cat']);
        $n += strlen($r);
    }
    return $n . ':' . md5($r);
});

bench('preg_replace_1_3mb_x10', function () use ($text) {
    $n = 0;
    for ($i = 0; $i < 10; $i++) {
        $r = preg_replace('/\b(\w+)@(\w+)\.com\b/', '$2 at $1', $text);
        $n += strlen($r);
    }
    return $n . ':' . md5($r);
});

bench('preg_match_small_500k', function () {
    $n = 0;
    for ($i = 0; $i < 500000; $i++) {
        if (preg_match('/^user-(\d+)-([a-z]+)$/', 'user-' . $i . '-abc', $m)) { $n += (int) $m[1]; }
    }
    return $n;
});

bench('preg_replace_callback_1_3mb_x3', function () use ($text) {
    $n = 0;
    for ($i = 0; $i < 3; $i++) {
        $r = preg_replace_callback('/\d+/', fn($m) => (string) ((int) $m[0] + 1), $text);
        $n += strlen($r);
    }
    return $n . ':' . md5($r);
});

bench('preg_split_1_3mb_x5', function () use ($text) {
    $n = 0;
    for ($i = 0; $i < 5; $i++) { $n += count(preg_split('/[\s.]+/', $text, -1, PREG_SPLIT_NO_EMPTY)); }
    return $n;
});

bench('explode_implode_1_3mb_x20', function () use ($text) {
    $n = 0;
    for ($i = 0; $i < 20; $i++) {
        $parts = explode(' ', $text);
        $n += count($parts);
        $j = implode('_', $parts);
        $n += strlen($j);
    }
    return $n . ':' . md5($j);
});

bench('case_trim_ucwords_500k', function () {
    $n = 0;
    for ($i = 0; $i < 500000; $i++) {
        $t = ucwords(strtolower(trim("  Hello WORLD number $i  ")));
        $n += strlen($t) + ord($t[0]);
    }
    return $n;
});

bench('htmlspecialchars_md5_200k', function () {
    $n = 0;
    for ($i = 0; $i < 200000; $i++) {
        $t = htmlspecialchars("<a href=\"/p?id=$i&x=1\">it's $i</a>");
        $n += strlen($t) + ord(md5($t)[0]);
    }
    return $n;
});

// ~5 MB JSON structure (PLAN: "json_encode/decode of a 5 MB structure").
function build_structure(): array {
    $rows = [];
    for ($i = 0; $i < 30000; $i++) {
        $rows[] = [
            'id' => $i,
            'uuid' => md5((string) $i),
            'name' => 'User number ' . $i,
            'email' => "user$i@example.com",
            'active' => ($i % 3) === 0,
            'score' => $i / 3,
            'tags' => ['alpha', 'beta', 'gamma-' . ($i % 17)],
            'address' => ['street' => $i . ' Main St', 'city' => 'Springfield', 'geo' => ['lat' => 1.5 + $i, 'lng' => -2.25 - $i]],
            'bio' => null,
        ];
    }
    return $rows;
}
$struct = build_structure();
$json = json_encode($struct);
printf("INFO json_bytes %d\n", strlen($json));
$ser = serialize($struct);
printf("INFO serialize_bytes %d\n", strlen($ser));

bench('json_encode_5mb_x5', function () use ($struct) {
    $n = 0;
    for ($i = 0; $i < 5; $i++) { $j = json_encode($struct); $n += strlen($j); }
    return $n . ':' . md5($j);
});

bench('json_decode_assoc_5mb_x5', function () use ($json) {
    $n = 0;
    for ($i = 0; $i < 5; $i++) { $d = json_decode($json, true); $n += count($d) + $d[29999]['id']; }
    return $n;
});

bench('json_decode_object_5mb_x5', function () use ($json) {
    $n = 0;
    for ($i = 0; $i < 5; $i++) { $d = json_decode($json); $n += count($d) + $d[29999]->id; }
    return $n;
});

bench('serialize_x5', function () use ($struct) {
    $n = 0;
    for ($i = 0; $i < 5; $i++) { $s = serialize($struct); $n += strlen($s); }
    return $n . ':' . md5($s);
});

bench('unserialize_x5', function () use ($ser) {
    $n = 0;
    for ($i = 0; $i < 5; $i++) { $d = unserialize($ser); $n += count($d) + $d[29999]['id']; }
    return $n;
});

bench('var_export_x2', function () use ($struct) {
    $n = 0;
    for ($i = 0; $i < 2; $i++) { $s = var_export($struct, true); $n += strlen($s); }
    return $n . ':' . md5($s);
});
