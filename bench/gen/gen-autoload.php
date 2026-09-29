<?php
// Generates a PSR-4 package of N classes for bench/autoload.php.
//   php gen-autoload.php <out-dir> [n=2000]
// Layout: <out>/composer.json, <out>/src/Pkg<k>/C<i>.php. Run
// `composer dump-autoload` (NOT -o: we want the PSR-4 lookup path Composer
// uses by default, as a fresh Drupal/Laravel checkout would) in <out> afterwards.
$out = $argv[1] ?? null;
$n = (int) ($argv[2] ?? 2000);
if (!$out) { fwrite(STDERR, "usage: gen-autoload.php <out-dir> [n]\n"); exit(2); }
@mkdir("$out/src", 0777, true);
file_put_contents("$out/composer.json", json_encode([
    'name' => 'bench/autoload',
    'autoload' => ['psr-4' => ['Bench\\Gen\\' => 'src/']],
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES) . "\n");
$iface = "<?php\ndeclare(strict_types=1);\nnamespace Bench\\Gen;\n\ninterface Node\n{\n    public function id(): int;\n    public function weight(): int;\n}\n";
file_put_contents("$out/src/Node.php", $iface);
$trait = "<?php\ndeclare(strict_types=1);\nnamespace Bench\\Gen;\n\ntrait Weighted\n{\n    protected int \$w = 1;\n    public function weight(): int { return \$this->w + static::BASE; }\n}\n";
file_put_contents("$out/src/Weighted.php", $trait);
for ($i = 0; $i < $n; $i++) {
    $pkg = 'Pkg' . intdiv($i, 100);
    @mkdir("$out/src/$pkg", 0777, true);
    $code = <<<PHP
<?php
declare(strict_types=1);

namespace Bench\\Gen\\$pkg;

use Bench\\Gen\\Node;
use Bench\\Gen\\Weighted;

/**
 * Generated class $i. Shaped like a small framework service: constants,
 * typed properties, constructor promotion, a few methods, one static factory.
 */
final class C$i implements Node
{
    use Weighted;

    public const BASE = $i;
    public const NAME = 'c$i';

    private array \$options = ['enabled' => true, 'level' => $i, 'label' => 'class-$i'];

    public function __construct(
        private readonly int \$id = $i,
        protected ?string \$label = null,
    ) {
        \$this->w = \$id % 7;
    }

    public static function create(): static
    {
        return new static();
    }

    public function id(): int
    {
        return \$this->id;
    }

    public function label(): string
    {
        return \$this->label ?? self::NAME;
    }

    public function option(string \$key, mixed \$default = null): mixed
    {
        return \$this->options[\$key] ?? \$default;
    }

    public function describe(): string
    {
        return sprintf('%s#%d(%d)', \$this->label(), \$this->id, \$this->weight());
    }
}

PHP;
    file_put_contents("$out/src/$pkg/C$i.php", $code);
}
echo "generated $n classes in $out\n";
