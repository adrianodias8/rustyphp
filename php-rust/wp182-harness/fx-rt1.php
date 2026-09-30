<?php
// FX-RT1 (S-183): presidio bilaterale della leva L-RT1 «Ret in place» — oracle == phpr byte-id.
// Il rischio della leva è la NOTA GC mancante su un locale che è l'ultimo detentore di un CICLO: il ciclo non si libera
// al ritorno (refcount >0) ma solo al successivo gc_collect_cycles(), che trova il candidato SOLO se il ritorno l'ha annotato.
// Forme: (1) locale array auto-referente che tiene un oggetto; (2) locale oggetto auto-referente; (3) locale array di oggetti
// senza ciclo (dtor al ritorno); (4) ciclo in un frame con $this (cammino vecchio, controllo); (5) ciclo lasciato su uno slot
// di funzione ricorsiva; (6) foreach nel frame (cammino vecchio per ammissione: iters); (7) $$dinamica (cammino vecchio).
// Il MUTANTE «note GC saltate» morde sulla forma (1): collect1 trova 0 invece di 1 e «dtor a» slitta a fine script.
class D { public function __construct(public string $n) {} public $p = null; function __destruct() { echo "dtor {$this->n}\n"; } }
function cyc_arr($n) { $a = []; $a['o'] = new D($n); $a['me'] = &$a; return $n; }
function cyc_obj($n) { $o = new D($n); $o->p = $o; return strlen($n); }
function plain($n) { $a = [new D($n . '1'), new D($n . '2')]; return count($a); }
class K { function m($n) { $o = new D($n); $o->p = $o; return $n; } }
function rec($n) { if ($n <= 0) { $o = new D('rec'); $o->p = $o; return 0; } return 1 + rec($n - 1); }
function withforeach($n) { $o = new D($n); $o->p = $o; foreach ([1, 2] as $i) { $n .= $i; } return $n; }
function dynvar($n) { $name = 'x'; $$name = new D($n); $t = $$name; $t->p = $t; unset($t); return $n; }
echo "cyc_arr: ", cyc_arr('a'), "\n"; echo "collect1: ", gc_collect_cycles(), "\n";
echo "cyc_obj: ", cyc_obj('bb'), "\n"; echo "collect2: ", gc_collect_cycles(), "\n";
$pl = plain('p'); echo "plain: $pl\n"; // assegnazione separata: dtor dei locali allo Sweep dopo lo statement = §3.28 (ii) pre-esistente, fuori perimetro
$k = new K; echo "method: ", $k->m('m'), "\n"; echo "collect3: ", gc_collect_cycles(), "\n";
echo "rec: ", rec(3), "\n"; echo "collect4: ", gc_collect_cycles(), "\n";
echo "foreach: ", withforeach('f'), "\n"; echo "collect5: ", gc_collect_cycles(), "\n";
echo "dynvar: ", dynvar('d'), "\n"; echo "collect6: ", gc_collect_cycles(), "\n";
echo "FX-RT1 DONE\n";
