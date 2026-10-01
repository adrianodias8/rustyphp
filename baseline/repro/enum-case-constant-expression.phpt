--TEST--
Backed enum cases with constant-expression values (negative, shifts, concatenation)
--FILE--
<?php
declare(strict_types=1);
namespace X;
enum S: int { case Info = -1; case OK = 0; case Warning = 1; case Bit = 1 << 3; case Mix = (2 | 4) - 1; }
enum T: string { const P = "pre"; case A = self::P . "-a"; case B = "b"; }
var_dump(S::from(1), S::tryFrom(-1), S::from(max(S::OK->value, S::Info->value)), S::Bit->value, S::Mix->value);
var_dump(T::from("pre-a"), T::A->value, array_map(fn($c) => $c->name, S::cases()));
?>
--EXPECT--
enum(X\S::Warning)
enum(X\S::Info)
enum(X\S::OK)
int(8)
int(5)
enum(X\T::A)
string(5) "pre-a"
array(5) {
  [0]=>
  string(4) "Info"
  [1]=>
  string(2) "OK"
  [2]=>
  string(7) "Warning"
  [3]=>
  string(3) "Bit"
  [4]=>
  string(3) "Mix"
}
