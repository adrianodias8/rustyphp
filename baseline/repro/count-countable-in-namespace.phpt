--TEST--
count() on a Countable object, called unqualified inside a namespace
--DESCRIPTION--
Found 2026-09-30 by the PLAN.md §1.2 smoke test: `composer require monolog/monolog`
(Composer 2.10.1, src/Composer/Installer.php:556, `count($pool)` where
`Pool implements \Countable`) dies under phpr at upstream 9d4ef5ba with
  TypeError: count(): Argument #1 ($value) must be of type Countable|array, App\Pool given
The same call at global scope, or written `\count($pool)`, works.
Expected output below is the oracle's (PHP 8.5.7).
--FILE--
<?php
namespace App;

class Pool implements \Countable
{
    public function count(): int { return 3; }
}

function f(): void
{
    $pool = new Pool();
    echo "A: " . count($pool) . " packages\n";
    $n = count($pool);
    echo "B: $n\n";
    echo "C: ", \count($pool), "\n";
}

f();
$p = new Pool();
echo "D: " . count($p) . "\n";
?>
--EXPECT--
A: 3 packages
B: 3
C: 3
D: 3
