<?php
// Differential fuzz of unserialize() failure behaviour (NOTES session 12):
//   php unser-fuzz.php gen N SEED > cases.txt   (oracle: seeds + mutations)
//   php|ferro unser-fuzz.php run cases.txt      (diff the two outputs)
// Each case prints the result (var_export-like), every warning with its
// message, and the magic-method trace (__wakeup / __unserialize /
// __destruct / autoload), so offsets and side effects are compared.
enum Suit: string { case H = 'h'; case S = 's'; }
class W { public $n = 0; public $o; function __wakeup() { echo "  wakeup {$this->id()}\n"; } function __destruct() { echo "  destruct {$this->id()}\n"; } function id() { return is_scalar($this->n) ? $this->n : gettype($this->n); } }
class U { public $d; function __unserialize(array $d) { echo '  unser ', count($d), "\n"; $this->d = $d; } function __destruct() { echo "  destructU\n"; } }
class P { public $a = 1; protected $b = 2; private $c = 3; public ?P $next = null; }
class T { public int $i = 0; public string $s = ''; }
abstract class Abs {}
class Ser implements Serializable { function serialize() { return ''; } function unserialize($s) {} }
spl_autoload_register(function ($c) { echo "  autoload $c\n"; });
set_error_handler(function ($no, $msg) { echo "  [$no] $msg\n"; return true; });

function seeds(): array {
    $w = new W; $w->n = 7; $w->o = new W;
    $u = new U; $u->d = [1, 'x'];
    $p = new P; $p->next = new P;
    $o = new stdClass; $o->self = $o; $o->list = [1, 2.5, -3, true, null, 'é'];
    return [
        serialize([1, 'a' => 'b', 2 => [3, 4], 'k' => null, 'f' => 1.5, 't' => true]),
        serialize($w), serialize($u), serialize($p), serialize($o), serialize(Suit::H),
        serialize([Suit::S, $w, 'x' => $u]), serialize([['a' => ['b' => ['c' => 1]]]]),
        serialize(-PHP_INT_MAX - 1), serialize(1e100), serialize(-0.0), serialize(NAN), serialize(INF),
        'O:1:"Q":0:{}', 'O:3:"Abs":0:{}', 'O:3:"Ser":0:{}', 'E:6:"Suit:X";', 'E:4:"W:n0";',
        'O:1:"T":1:{s:1:"i";s:1:"x";}', 'a:1:{i:0;r:1;}', 'a:2:{i:0;O:1:"W":0:{}i:1;r:2;}',
        'i:99999999999999999999;', 'a:1:{i:99999999999999999999;i:1;}', 'd:1.;', 'd:.5;', 'd:1e5;', 'd:1.e5;', 'd:+.5E-3;',
        'S:3:"a\62c";', 's:3:"abc";junk', 'b:1;', 'N;',
    ];
}

function mutate(string $s): string {
    $n = strlen($s);
    switch (mt_rand(0, 5)) {
        case 0: return substr($s, 0, mt_rand(0, max(0, $n - 1)));
        case 1: if ($n) { $s[mt_rand(0, $n - 1)] = '0123456789:;{}"aisdObNrE-+.'[mt_rand(0, 26)]; } return $s;
        case 2: $i = mt_rand(0, $n); return substr($s, 0, $i) . '0123456789:;{}"'[mt_rand(0, 14)] . substr($s, $i);
        case 3: $i = mt_rand(0, max(0, $n - 1)); return substr($s, 0, $i) . substr($s, $i + 1);
        case 4: return preg_replace_callback('/\d+/', fn ($m) => mt_rand(0, 3) ? $m[0] : (string) ($m[0] + mt_rand(-1, 1)), $s, 1);
        default: return $s . substr($s, 0, mt_rand(0, 3));
    }
}

function show($v, int $d = 0): string {
    if ($d > 4) return '…';
    if (is_array($v)) { $o = []; foreach ($v as $k => $x) $o[] = var_export($k, true) . '=>' . show($x, $d + 1); return '[' . implode(',', $o) . ']'; }
    if ($v instanceof UnitEnum) return get_class($v) . '::' . $v->name;
    if (is_object($v)) { $o = []; foreach ((array) $v as $k => $x) $o[] = str_replace("\0", '~', $k) . '=' . (is_object($x) ? get_class($x) : show($x, $d + 1)); return get_class($v) . '{' . implode(',', $o) . '}'; }
    if (is_float($v)) return is_nan($v) ? 'NAN' : var_export($v, true);
    return var_export($v, true);
}

if ($argv[1] === 'gen') {
    mt_srand((int) $argv[3]);
    $seeds = seeds();
    foreach ($seeds as $s) echo base64_encode($s), "\n";
    for ($i = 0; $i < (int) $argv[2]; $i++) {
        $s = $seeds[mt_rand(0, count($seeds) - 1)];
        for ($k = mt_rand(1, 2); $k > 0; $k--) $s = mutate($s);
        echo base64_encode($s), "\n";
    }
    exit;
}
foreach (file($argv[2], FILE_IGNORE_NEW_LINES) as $i => $line) {
    $s = base64_decode($line);
    echo "#$i ", json_encode($s, JSON_INVALID_UTF8_SUBSTITUTE), "\n";
    try {
        $v = unserialize($s);
        echo '  = ', show($v), "\n";
        unset($v);
    } catch (Throwable $e) {
        echo '  ! ', get_class($e), ': ', $e->getMessage(), "\n";
    }
    gc_collect_cycles();
}
