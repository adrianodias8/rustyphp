--TEST--
unserialize() builds in one pass like PHP: error offsets, Extra data, delayed __wakeup/__unserialize (also on failure), destructor suppression, non-instantiable classes
--FILE--
<?php
spl_autoload_register(function ($c) { echo "autoload $c\n"; });
class W { public $n = 0; public $o; function __wakeup() { echo "wakeup {$this->n}\n"; } function __destruct() { echo "destruct {$this->n}\n"; } }
class U { public $d; function __unserialize(array $d) { echo "unser ", json_encode($d), "\n"; $this->d = $d; } function __destruct() { echo "destructU\n"; } }
class Th { function __wakeup() { throw new Exception('no'); } function __destruct() { echo "destructTh\n"; } }
abstract class A {} interface I {} trait Tr {} enum E { case X; }
class Ser implements Serializable { function serialize() { return ''; } function unserialize($s) {} }
function t(string $s) {
    echo '-- ', $s, "\n";
    try { var_dump(unserialize($s)); } catch (Throwable $e) { echo get_class($e), ': ', $e->getMessage(), "\n"; }
}
t('');
t('i:1;x');
t('s:2:"abc";');
t('s:5:"abc";');
t('a:2:{i:0;i:1;}');
t('a:1:{i:0;i:1;');
t('a:1:{d:1.5;i:1;}');
t('a:1:{a:0:{}i:1;}');
t('i:99999999999999999999;');
t('d:.5;');
t('d:1.e5;');
t('S:3:"a\62c";');
t('E:6:":uit:X";');
t('O:1:"Q":0:{}');
t('a:3:{i:0;O:1:"W":1:{s:1:"n";i:1;}i:1;O:1:"Q":0:{}i:2;X}');
t('a:2:{i:0;O:1:"U":1:{i:0;i:5;}i:1;O:1:"W":1:{s:1:"n";i:2;s:1:"z";Y}}');
t('a:2:{i:0;O:1:"U":1:{i:0;i:7;}i:1;O:1:"W":2:{s:1:"n";i:7;s:1:"o";O:1:"W":1:{s:1:"n";i:4;}}}x');
t('a:2:{i:0;O:2:"Th":0:{}i:1;O:1:"W":1:{s:1:"n";i:3;}}');
t('O:1:"W":1:{s:1:"n";i:9;N;}');
t('O:1:"A":0:{}');
t('O:1:"I":0:{}');
t('O:2:"Tr":0:{}');
t('O:1:"E":0:{}');
t('O:3:"Ser":0:{}');
t('a:1:{i:0;r:1;}');
t('a:2:{i:0;O:8:"stdClass":0:{}i:1;r:2;}');
foreach ([4096, 4097] as $n) {
    $r = @unserialize(str_repeat('a:1:{i:0;', $n) . 'N;' . str_repeat('}', $n));
    echo "depth $n: ", $r === false ? 'false' : 'array', ' ', error_get_last()['message'] ?? '', "\n";
    error_clear_last();
}
echo "end\n";
?>
--EXPECTF--

Deprecated: Ser implements the Serializable interface, which is deprecated. Implement __serialize() and __unserialize() instead (or in addition, if support for old PHP versions is necessary) in %s on line %d
-- 
bool(false)
-- i:1;x

Warning: unserialize(): Extra data starting at offset 4 of 5 bytes in %s on line 10
int(1)
-- s:2:"abc";

Warning: unserialize(): Error at offset 7 of 10 bytes in %s on line 10
bool(false)
-- s:5:"abc";

Warning: unserialize(): Error at offset 10 of 10 bytes in %s on line 10
bool(false)
-- a:2:{i:0;i:1;}

Warning: unserialize(): Unexpected end of serialized data in %s on line 10

Warning: unserialize(): Error at offset 13 of 14 bytes in %s on line 10
bool(false)
-- a:1:{i:0;i:1;

Warning: unserialize(): Error at offset 13 of 13 bytes in %s on line 10
bool(false)
-- a:1:{d:1.5;i:1;}

Warning: unserialize(): Error at offset 11 of 16 bytes in %s on line 10
bool(false)
-- a:1:{a:0:{}i:1;}

Warning: unserialize(): Error at offset 10 of 16 bytes in %s on line 10
bool(false)
-- i:99999999999999999999;

Warning: unserialize(): Numerical result out of range in %s on line 10
int(9223372036854775807)
-- d:.5;
float(0.5)
-- d:1.e5;
float(100000)
-- S:3:"a\62c";

Deprecated: unserialize(): Unserializing the 'S' format is deprecated in %s on line 10
string(3) "abc"
-- E:6:":uit:X";

Warning: unserialize(): Class '' not found in %s on line 10

Warning: unserialize(): Error at offset 0 of 13 bytes in %s on line 10
bool(false)
-- O:1:"Q":0:{}
autoload Q
object(__PHP_Incomplete_Class)#2 (1) {
  ["__PHP_Incomplete_Class_Name"]=>
  string(1) "Q"
}
-- a:3:{i:0;O:1:"W":1:{s:1:"n";i:1;}i:1;O:1:"Q":0:{}i:2;X}
autoload Q

Warning: unserialize(): Error at offset 53 of 55 bytes in %s on line 10
wakeup 1
destruct 1
bool(false)
-- a:2:{i:0;O:1:"U":1:{i:0;i:5;}i:1;O:1:"W":1:{s:1:"n";i:2;s:1:"z";Y}}

Warning: unserialize(): Error at offset 56 of 67 bytes in %s on line 10
unser [5]
destructU
wakeup 2
destruct 2
bool(false)
-- a:2:{i:0;O:1:"U":1:{i:0;i:7;}i:1;O:1:"W":2:{s:1:"n";i:7;s:1:"o";O:1:"W":1:{s:1:"n";i:4;}}}x

Warning: unserialize(): Extra data starting at offset 90 of 91 bytes in %s on line 10
unser [7]
wakeup 4
wakeup 7
array(2) {
  [0]=>
  object(U)#3 (1) {
    ["d"]=>
    array(1) {
      [0]=>
      int(7)
    }
  }
  [1]=>
  object(W)#2 (2) {
    ["n"]=>
    int(7)
    ["o"]=>
    object(W)#4 (2) {
      ["n"]=>
      int(4)
      ["o"]=>
      NULL
    }
  }
}
destructU
destruct 7
destruct 4
-- a:2:{i:0;O:2:"Th":0:{}i:1;O:1:"W":1:{s:1:"n";i:3;}}
Exception: no
-- O:1:"W":1:{s:1:"n";i:9;N;}

Warning: unserialize(): Error at offset 23 of 26 bytes in %s on line 10
wakeup 9
destruct 9
bool(false)
-- O:1:"A":0:{}
Error: Cannot instantiate abstract class A
-- O:1:"I":0:{}
Error: Cannot instantiate interface I
-- O:2:"Tr":0:{}
Error: Cannot instantiate trait Tr
-- O:1:"E":0:{}
Error: Cannot instantiate enum E
-- O:3:"Ser":0:{}

Warning: Erroneous data format for unserializing 'Ser' in %s on line 10

Warning: unserialize(): Error at offset 13 of 14 bytes in %s on line 10
bool(false)
-- a:1:{i:0;r:1;}

Warning: unserialize(): Error at offset 13 of 14 bytes in %s on line 10
bool(false)
-- a:2:{i:0;O:8:"stdClass":0:{}i:1;r:2;}
array(2) {
  [0]=>
  object(stdClass)#3 (0) {
  }
  [1]=>
  object(stdClass)#3 (0) {
  }
}
depth 4096: array unserialize(): Error at offset 13 of 14 bytes
depth 4097: false unserialize(): Error at offset 36869 of 40972 bytes
end
