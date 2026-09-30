# REPORT_GAP_183 — gap perf oracle↔phpr al pin s181 (SOLO sessione S-183, 2026-09-29 sera → 2026-09-30 sera)

Pin INVARIATO: phpr 19a2faa83a492745 + server b2802f08c5887e77 (nessuna promozione). Tree = pin + L-RT1 «Ret in place» (9c78dc39, TENUTA, non promossa). Tre misure di record: istruttoria t24+ORM, scomposizione L-CR1 (2 corse), A/B L-RT1.

## Micro — NON rimisurate (pin invariato): riferimento s181 arith 2,1 · prop 2,3 · calls 4,0 · str 4,1 · arr 2,8 · re 2,6
Giudici sul pin s181 in S-183 (same-binary, E1 pulita): calls-dq 89,67 / 90,00 / 90,17 (PREV 90,50; oracle 21,67-22,17: 4,07-4,15×) · prop-dq 31,27 / 31,73 / 31,60 (PREV 31,60) · arith-dq 18,12-18,40.

## Istruttoria t24 (s182-pair.sh, finestra 01:07-04:00, gate loadavg <3 ×6 + E2 98-135 %) + ORM E3/E4 — rc=0 CHIUSA
| | t23 (S-181, macchina in uso) | **t24** | lettura |
|---|---|---|---|
| WP full mediana | 1,799 (bordo) | **1,770** | COMPATIBILE in [1,738;1,799]; 5 gambe pulite (leg1 ictx 151/s vs med 76: esclusa), banda_ON **0,014** (t23 0,079) |
| media user-only | 2,44-2,53 | 2,414-2,420 | come t21 (2,42-2,44) |
| ORM net | [6,721;8,250] contaminata | **[7,010;7,014]** | ≤7,05, sentinella oracle 4,89/4,89 in banda, ictx ok, Δ_norm [+0,08;+0,18] nel rumore; dbal [7,291;7,300]; parità 16/10 |
⇒ t23/ORM-s181 = contaminazione ambientale DICHIARATA; riferimenti nuovi WP **1,770** · ORM **[7,010;7,014]**; regola 4 NON scatta.

## Scomposizione L-CR1 (s183-scomp-verdetto.out; A = pin s181; placebo 0,50 ⇒ soglia 4)
| componente | D = X−A calls-dq | segni | attesa | guardie prop/arith |
|---|---|---|---|---|
| −a frame in pila | **+3,67/+3,67** | 5/5 | [0;2] FUORI | prop **+2,07 VIOLATA 5/5** (trasversale firmato su (a)); arith +0,64 |
| −b ip=1 | +2,83/+2,50 | 5/5 | [3;5] FUORI di poco | rispettate |
| −c guardia Ret | +2,33/+2,67 | 5/5 | [0;1] FUORI | rispettate |
| P placebo | +0,50 | 4/5 | [−2;2] centrata | rispettate |
Somma 8,50-9,17 ≈ cifra S-181 [8,00;8,17]: nessun residuo di layout; disasm −a = pin s180 (bl 6123, sp_refs 11613) ⇒ il −8 % di sp_refs e il +2,47 su prop-dq sono di (a). Meccanismo (a) riscritto: non «copia del Frame evitata» ma forma del handler senza `enter_callee` (regalloc).

## Leva L-RT1 «Ret in place» (s183-rt1-verdetto.out) — rc=3 SOLA DIREZIONE
A 89,67 · Z 90,33 (gemello a contenuto, |A−Z| 0,67) · **B 86,83** ns/iter (3,95×); D +2,83/+2,50, rumore 1,83/1,00, segni 5/5, attesa [1;4] CENTRATA; guardie prop −0,07 · arith +0,08 (|D|<1 rispettate). Sotto il pavimento 4 ⇒ tenuta nel tree, si compone con la prossima leva sullo stesso giudice. Disasm B bl 6141 / istr 72457 / sp_refs 10706 (pin 6132/72321/10685: atteso bl −2, misurato +9: il fast path porta inline gc_note ×2 + put + truncate).

## Lettura del gap (non cifra)
calls: 90 ns/chiamata contro 22; i corpi valgono pezzi da 2-4 ns ciascuno (tre di L-CR1 + uno di L-RT1 ≈ 11 ns misurati): nessun colpo singolo ≥4 resta in vista, la strada è la COMPOSIZIONE (pin → tree con più pezzi in un solo A/B). Prossimi pezzi con cifra attesa: (a) per i METODI (methodcall_fast passa ancora il Frame a `enter_callee`: ≈3,7 su un giudice di metodi), il pop/push del valore di ritorno, la lettura `ret_shape`/flags. CheckArity a compile-time SCARTATO per tempo (12,9M op ORM × ~2,7 ns ≈ 0,1 %). str 4,1× resta la seconda categoria.
