<?php
// Generates a ~5,000-line PHP file that DOES NOTHING when run (declarations
// only, nothing called) for the PLAN §2.2 compile-cost measurement: whatever
// time an engine spends on it is lex + parse + compile (+ startup floor).
//   php gen-compile.php <out-file> [lines=5000]
$out = $argv[1] ?? null;
$target = (int) ($argv[2] ?? 5000);
if (!$out) { fwrite(STDERR, "usage: gen-compile.php <out-file> [lines]\n"); exit(2); }
$src = "<?php\ndeclare(strict_types=1);\n\nnamespace Bench\\Compile;\n\n";
$lines = 5;
$i = 0;
while ($lines < $target) {
    if ($i % 4 === 3) {
        $chunk = <<<PHP
final class K$i
{
    public const A = $i;
    private array \$items = [];

    public function __construct(private int \$seed = $i, protected string \$name = 'k$i') {}

    public function push(mixed \$v): static
    {
        \$this->items[] = \$v;
        return \$this;
    }

    public function total(): int
    {
        \$t = \$this->seed;
        foreach (\$this->items as \$k => \$v) {
            if (is_int(\$v)) {
                \$t += \$v * \$k;
            } elseif (is_string(\$v)) {
                \$t += strlen(\$v);
            } else {
                \$t ^= self::A;
            }
        }
        return \$t;
    }
}


PHP;
    } else {
        $chunk = <<<PHP
function f$i(array \$rows, int \$limit = $i, ?string \$prefix = null): array
{
    \$out = [];
    \$prefix ??= 'p$i';
    foreach (\$rows as \$idx => \$row) {
        if (\$idx >= \$limit) {
            break;
        }
        \$key = \$prefix . '_' . (\$row['id'] ?? \$idx);
        \$out[\$key] = match (true) {
            isset(\$row['a']) => \$row['a'] + $i,
            isset(\$row['b']) => strtoupper((string) \$row['b']),
            default => sprintf('%s:%d', \$prefix, \$idx * $i),
        };
    }
    try {
        \$n = count(\$out) > 0 ? array_sum(array_filter(\$out, 'is_int')) : 0;
    } catch (\\Throwable \$e) {
        \$n = -1;
    }
    return ['n' => \$n, 'out' => \$out, 'fn' => static fn(int \$x): int => \$x + \$n];
}


PHP;
    }
    $src .= $chunk;
    $lines += substr_count($chunk, "\n");
    $i++;
}
file_put_contents($out, $src);
printf("generated %s: %d lines, %d bytes, %d declarations\n", $out, substr_count($src, "\n"), strlen($src), $i);
