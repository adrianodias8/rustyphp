//! The real regression test for "absent ≡ `=1`" — run in a SUBPROCESS and
//! judged by a dump diff.
//!
//! The old in-process half was effectively `f(x)==f(x)` (fabricated
//! coverage): it compiled twice with the same bool in the same env. Here
//! the two arms are TWO PROCESSES of the real binary with DIFFERENT
//! environments (env built from scratch, never inherited): one with
//! `PHPR_REG_LOWER` ABSENT, one with `=1`. Judge: BYTE-identical dump
//! (`PHPR_DUMP_OPS=1`, whole module over BODY_ZOO) + identical stdout +
//! a POSITIVE CONTROL (the dump contains register forms: an empty dump or
//! an env that never propagated cannot produce a green).
//! Coverage note: the "both modes" corpus exercises default(absent) and
//! `=0`; the absent↔`=1` pair end-to-end is exercised ONLY by this test.
//! Amended later: a DISCRIMINATING `=0` arm (proves the env travels —
//! different dump, same stdout), an out-of-funnel positive control
//! (`Z::{prop-init}` in the dump), and the residual `Binary(Add)` pinned
//! to EXACTLY `==1`.

use std::process::Command;

const BODY_ZOO: &str = r#"<?php
class Z {
    const K = 5;
    public $p = self::K + 1;
    public function m($a, $b) { return $a + $b * 2; }
}
function f($x) { $s = 0; for ($i = 0; $i < $x; $i++) { $s = $s + $i; } return $s; }
$z = new Z();
echo f(9), ":", $z->m(3, 4), ":", $z->p, "\n";
"#;

fn run_arm(src_path: &std::path::Path, reg_lower: Option<&str>) -> (Vec<u8>, Vec<u8>) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ferro"));
    // Environment BUILT from a closed list: nothing is inherited from the
    // test process — the absence of PHPR_REG_LOWER holds BY CONSTRUCTION.
    cmd.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("PHPR_DUMP_OPS", "1")
        .arg(src_path);
    if let Some(v) = reg_lower {
        cmd.env("PHPR_REG_LOWER", v);
    }
    let out = cmd.output().expect("spawn phpr");
    assert!(
        out.status.success(),
        "phpr rc!=0 (reg_lower={reg_lower:?}): stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    (out.stdout, out.stderr)
}

/// PER-BODY count of an op in the PHPR_DUMP_OPS dump. Bodies open with the
/// header `-- NAME n_slots=… --`; the op is recognised ANCHORED as the
/// second token of an instruction line (`0003 Binary(Add)`), never as a
/// bare substring (a hypothetical `XBinary(Add)` does not count).
fn per_body_op_counts(dump: &str, op_token: &str) -> std::collections::BTreeMap<String, usize> {
    let mut cur = String::from("<fuori-corpo>");
    let mut map = std::collections::BTreeMap::new();
    for line in dump.lines() {
        if let Some(rest) = line.strip_prefix("-- ") {
            cur = rest.split(" n_slots").next().unwrap_or(rest).trim().to_string();
        } else {
            let mut it = line.split_whitespace();
            let is_instr = matches!(it.next(), Some(ix) if !ix.is_empty() && ix.chars().all(|c| c.is_ascii_digit()));
            if is_instr && it.next() == Some(op_token) {
                *map.entry(cur.clone()).or_insert(0) += 1;
            }
        }
    }
    map
}

#[test]
fn absent_env_subprocess_dump_identical_to_explicit_one() {
    let dir = std::env::temp_dir().join(format!("phpr-absent-eq-one-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let src = dir.join("body_zoo.php");
    std::fs::write(&src, BODY_ZOO).expect("write fixture");

    let (out_absent, dump_absent) = run_arm(&src, None);
    let (out_one, dump_one) = run_arm(&src, Some("1"));
    let (out_zero, dump_zero) = run_arm(&src, Some("0"));
    let _ = std::fs::remove_dir_all(&dir);

    // Positive control BEFORE the verdict: the absent arm really is in the
    // default-ON mode — the dump exists and shows register forms in
    // {main}/f (the scalar loop produces them). Without this proof, two
    // empty dumps or two dead envs would give an indistinguishable green
    // (the mode is PROVEN, not assumed).
    let d = String::from_utf8_lossy(&dump_absent);
    assert!(
        d.contains("BinaryDst") || d.contains("CmpJmpSC") || d.contains("BinarySS"),
        "controllo positivo: il dump del braccio ASSENTE non mostra forme \
         registro — dump morto o modo non-default?\n{d}"
    );
    // Identical program output (functional parity of the two arms).
    assert_eq!(
        out_absent, out_one,
        "stdout diverso tra assente e `=1` (parita' funzionale rotta)"
    );
    // The "whole module" claim is PROVEN — the dump must contain the
    // OUT-OF-FUNNEL body of the property initializer (`Z::{prop-init}`),
    // not just {main}/fn.
    assert!(
        d.contains("Z::{prop-init}"),
        "controllo positivo fuori-funnel: il dump non contiene il corpo \
         `Z::{{prop-init}}` — il «modulo intero» non e' provato\n{d}"
    );
    // Replaces the older GLOBAL `==1` check: the residual `Binary(Add)` is
    // judged PER BODY, anchored on the op token — a new residual shows up
    // with the body NAME instead of conflating three causes (changed
    // prelude / widened funnel / new residual) into one global count.
    let adds = per_body_op_counts(&d, "Binary(Add)");
    let residui: Vec<String> = adds.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    assert_eq!(
        adds.get("Z::{prop-init}").copied().unwrap_or(0),
        1,
        "Binary(Add) atteso ESATTAMENTE 1 dentro Z::{{prop-init}}; residui per corpo: {residui:?}"
    );
    assert_eq!(
        adds.len(),
        1,
        "Binary(Add) residuo FUORI da Z::{{prop-init}} — corpi con residuo: {residui:?}"
    );
    // Enumeration tripwire: ALL the zoo bodies appear BY NAME in the dump
    // of the absent arm.
    for body in ["{main}", "fn f", "Z::m", "Z::{prop-init}"] {
        assert!(
            d.lines().any(|l| l.strip_prefix("-- ").is_some_and(|r| r.trim_start().starts_with(body))),
            "corpo '{body}' assente dal dump: lo zoo non e' piu' enumerato"
        );
    }
    // DISCRIMINATING `=0` arm — two arms equal-by-construction cannot fail
    // on mode; the third arm proves the env travels: identical stdout but
    // a DIFFERENT dump (stack emission, zero register forms).
    assert_eq!(out_zero, out_absent, "stdout `=0` diverso (parita' funzionale rotta)");
    let dz = String::from_utf8_lossy(&dump_zero);
    assert_ne!(
        dump_zero, dump_absent,
        "il dump `=0` e' IDENTICO all'assente: l'env non viaggia — il \
         dente non discrimina il modo"
    );
    assert!(
        !dz.contains("BinaryDst") && !dz.contains("CmpJmpSC") && !dz.contains("BinarySS"),
        "forme registro nel dump `=0`: il modo OFF non e' off\n{dz}"
    );
    // POSITIVE control of the `=0` arm — an env that killed dumping
    // entirely under `=0` would pass the negative checks above. The OFF
    // dump must contain Z::{prop-init} and live STACK forms: anchored
    // `BinaryAdd` > 1. (The first expectation counted the generic
    // `Binary(Add)`, but the stack loop emits the DEDICATED `BinaryAdd` op
    // — the generic one only lives outside the funnel.)
    assert!(
        dz.contains("Z::{prop-init}"),
        "controllo positivo `=0`: manca Z::{{prop-init}} nel dump OFF"
    );
    let n_off: usize = per_body_op_counts(&dz, "BinaryAdd").values().sum();
    assert!(
        n_off > 1,
        "controllo positivo `=0`: attesi BinaryAdd di pila > 1, trovati {n_off}"
    );
    // The VERDICT: BYTE-identical dump over the whole module.
    assert_eq!(
        dump_absent, dump_one,
        "dump-diff: assente e `=1` NON emettono lo stesso modulo\n--- assente ---\n{}\n--- =1 ---\n{}",
        String::from_utf8_lossy(&dump_absent),
        String::from_utf8_lossy(&dump_one)
    );
}
