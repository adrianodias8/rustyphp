--TEST--
serialize() writes an enum case as E:, unserialize() resolves it to the singleton (autoloaded)
--FILE--
<?php
enum S { case A; case B; const K = 1; }
enum I: int { case One = 1; }
class Plain {}
$s = serialize([S::A, I::One, S::A]);
echo $s, "\n";
$v = unserialize($s);
var_dump($v[0] === S::A, $v[1] === I::One, $v[2] === $v[0]);
var_dump(unserialize(serialize(S::B)) === S::B);
var_dump(unserialize('a:2:{i:0;s:1:"x";i:1;E:3:"S:C";}'));
var_dump(unserialize('E:3:"S:K";'));
var_dump(unserialize('E:4:"Nope";'));
var_dump(unserialize('E:6:"Nope:A";'));
var_dump(unserialize('E:7:"Plain:A";'));
var_dump(unserialize('E:0:"";'));
spl_autoload_register(function ($c) {
    echo "autoload $c\n";
    if ($c === 'Late') { eval('enum Late { case X; }'); }
});
var_dump(unserialize('E:6:"Late:X";') === Late::X);
try {
    unserialize('O:1:"S":0:{}');
} catch (Error $e) {
    echo get_class($e), ': ', $e->getMessage(), "\n";
}
?>
--EXPECTF--
a:3:{i:0;E:3:"S:A";i:1;E:5:"I:One";i:2;r:2;}
bool(true)
bool(true)
bool(true)
bool(true)

Warning: unserialize(): Undefined constant S::C in %s on line %d

Warning: unserialize(): Error at offset 31 of 32 bytes in %s on line %d
bool(false)

Warning: unserialize(): S::K is not an enum case in %s on line %d

Warning: unserialize(): Error at offset 10 of 10 bytes in %s on line %d
bool(false)

Warning: unserialize(): Invalid enum name 'Nope' (missing colon) in %s on line %d

Warning: unserialize(): Error at offset 0 of 11 bytes in %s on line %d
bool(false)

Warning: unserialize(): Class 'Nope' not found in %s on line %d

Warning: unserialize(): Error at offset 0 of 13 bytes in %s on line %d
bool(false)

Warning: unserialize(): Class 'Plain' is not an enum in %s on line %d

Warning: unserialize(): Error at offset 0 of 14 bytes in %s on line %d
bool(false)

Warning: unserialize(): Error at offset 2 of 7 bytes in %s on line %d
bool(false)
autoload Late
bool(true)
Error: Cannot instantiate enum S
