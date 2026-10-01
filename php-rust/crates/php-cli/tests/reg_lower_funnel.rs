//! The register pass battery must exercise the PRODUCTION funnel — the env
//! flag read at process start and the pass applied at its real pipeline
//! point (BEFORE the slot_names cession) — and positively assert fused
//! forms in the `{main}` top-level, the leg the in-process battery could
//! not see.
//!
//! An in-process test cannot toggle `reg_lower::enabled()` (process-wide
//! `OnceLock`), so this spawns the real CLI binary: what production runs is
//! what the test judges. Doubles as the flag-ON smoke with a positive dump
//! control: a pass that silently stops rewriting fails HERE, not much
//! later in a measurement.

use std::process::Command;

fn run_phpr(envs: &[(&str, &str)], file: &std::path::Path) -> (String, String, bool) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ferro"));
    // Start from a known flag state regardless of the caller's shell.
    c.env_remove("PHPR_REG_LOWER");
    c.env_remove("PHPR_DUMP_OPS");
    for (k, v) in envs {
        c.env(k, v);
    }
    let out = c.arg(file).output().expect("spawn phpr");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.success(),
    )
}

#[test]
fn flag_on_folds_the_main_toplevel_and_matches_flag_off_output() {
    // The arith_small shape twice: a top-level tight loop in `{main}` AND
    // the same shape inside a FUNCTION — the probe outside `{main}` (the
    // dump's blind spot was precisely the non-main bodies). The THREE-add
    // chain `$t+$j+$j+$j` leaves the MIDDLE site lhs-stack with its result
    // on the stack (the first fuses into BinarySS, the last into
    // BinaryDst): flag-on it must become `BinaryAdd`, never remain a
    // generic `Binary(Add)`.
    let src = br#"<?php
function probe($n){ $t=0; for($j=0;$j<$n;$j++){ $t = $t + $j + $j + $j; $t -= 2*$j; $t += $j*3 - ($j>>2); } return $t; }
$s=0; for($i=0;$i<1000;$i++){ $s += $i*3 - ($i>>2); } echo $s,"\n"; echo probe(1000),"\n";"#;
    let dir = std::env::temp_dir();
    let file = dir.join(format!("reg-funnel-{}.php", std::process::id()));
    std::fs::write(&file, src.as_slice()).expect("write test php");

    // Arms with an EXPLICIT value from the closed list of the mode
    // contract: an arm written on absence alone would run in the NEW mode
    // after the default flips — a same-mode false green. BOTH arms with
    // the dump: bit-identity/difference of emission is judged ONLY by the
    // dump diff, never by the stopwatch.
    let (off_out, off_err, off_ok) =
        run_phpr(&[("PHPR_REG_LOWER", "0"), ("PHPR_DUMP_OPS", "1")], &file);
    let (on_out, on_err, on_ok) =
        run_phpr(&[("PHPR_REG_LOWER", "1"), ("PHPR_DUMP_OPS", "1")], &file);
    let _ = std::fs::remove_file(&file);

    // EXPECTED stdout pinned (derived, not copied from a run) and exit
    // status asserted — a binary that is wrong the SAME way in both modes
    // no longer passes on parity.
    assert!(off_ok, "flag-off exit non-zero");
    assert!(on_ok, "flag-on exit non-zero");
    let main_sum: i64 = (0..1000i64).map(|i| i * 3 - (i >> 2)).sum();
    let probe_sum: i64 = (0..1000i64).map(|j| j * 4 - (j >> 2)).sum();
    // probe: t += 3j; t -= 2j; t += 3j - (j>>2)  =>  Σ(4j - (j>>2)) — as above.
    let expected = format!("{main_sum}\n{probe_sum}\n");
    assert_eq!(off_out, expected, "flag-off stdout != atteso");
    assert_eq!(on_out, expected, "flag-on stdout != atteso");

    // The two arms must prove DIFFERENT emission on the probe — the hashes
    // of the two dumps are logged by the test, then the diff.
    let h = |s: &str| {
        use std::hash::{Hash, Hasher};
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut hs);
        hs.finish()
    };
    eprintln!("dump-hash off={:016x} on={:016x}", h(&off_err), h(&on_err));
    assert_ne!(
        off_err, on_err,
        "i dump dei due bracci sono IDENTICI: falso verde stesso-modo"
    );

    let fname = file.file_name().unwrap().to_string_lossy().into_owned();
    let chunk_of = |err: &str, label: &str| -> String {
        let unit = err
            .split("== unit ")
            .find(|u| u.split_whitespace().next().is_some_and(|p| p.ends_with(&fname)))
            .unwrap_or_else(|| panic!("no dump chunk for {fname} — PHPR_DUMP_OPS dead?\n{err}"));
        unit.split(&format!("-- {label} "))
            .nth(1)
            .and_then(|r| r.split("\n-- ").next())
            .unwrap_or_else(|| panic!("no body `{label}` in unit dump\n{unit}"))
            .to_owned()
    };

    // Positive control flag-on: `{main}` AND the probe function both show the
    // fused register forms (the loop compare is slot-const in main →
    // CmpJmpSC, slot-slot in the probe `$j<$n` → CmpJmpSS). Later
    // amendments: `+=` fuses into the RMW-on-slot `BinarySTDst`; BinaryDst
    // RELOCATED to the probe (lhs-stack chain). The superinstruction batch:
    // the tree `$i*3 - ($i>>2)` fuses the TWO BinarySC and the Sub into
    // `BinarySCSC` (the standalone BinarySC expectation is REPLACED — a
    // contains("BinarySC") would stay green as a SUBSTRING of BinarySCSC,
    // so the check names the WHOLE form), and the back-edge trigram
    // IncDecSlot;Pop;Jump fuses into `IncDecSlotJmp` in both bodies.
    // Second batch: in `{main}` the whole RMW statement fuses into
    // `BinarySCSCDst` (SCSC tree + STDst tail in a single op) — the check
    // is RELOCATED onto the surviving form; the old contains("BinarySCSC")
    // stays green by substring, and the probe keeps the BinaryDst/CmpJmpSS
    // site untouched by the second batch.
    for (label, cmp_form, dst_form) in [
        ("{main}", "CmpJmpSC", "BinarySCSCDst"),
        ("fn probe", "CmpJmpSS", "BinaryDst"),
    ] {
        let chunk = chunk_of(&on_err, label);
        for form in ["BinarySCSC", "IncDecSlotJmp", cmp_form, dst_form] {
            assert!(
                chunk.contains(form),
                "no {form} in the `{label}` dump: the pass did not rewrite it\n{chunk}"
            );
        }
        // Flag-on every add is either a FUSED form or `BinaryAdd` — a
        // residual generic `Binary(Add)` is the tripwire (dead windows or
        // dead extension).
        assert!(
            !chunk.contains("Binary(Add)"),
            "Binary(Add) generico nel dump flag-on di `{label}`\n{chunk}"
        );
    }
    // Positive control of the extension: the lhs-stack site of the chain
    // `$t+$j+$j` in the probe must show the specialised form flag-on.
    assert!(
        chunk_of(&on_err, "fn probe").contains("BinaryAdd"),
        "nessun BinaryAdd nel dump flag-on della probe: l'estensione \
         A-MA-101-3 non morde\n{}",
        chunk_of(&on_err, "fn probe")
    );
    // OFF arm, mirror positive control: stack-based emission with the
    // `BinaryAdd` specialisation and ZERO register forms.
    for label in ["{main}", "fn probe"] {
        let chunk = chunk_of(&off_err, label);
        assert!(
            chunk.contains("BinaryAdd"),
            "flag-off senza BinaryAdd in `{label}`: la specializzazione H-B2 è sparita\n{chunk}"
        );
        for form in [
            "BinarySC", "CmpJmpSC", "BinaryDst", "BinarySS", "CmpJmpSS", "BinarySTDst",
            // The superinstruction forms do not exist flag-off.
            "BinarySCSC", "BinaryTC", "IncDecSlotPop", "IncDecSlotJmp",
            "PropGetSlot", "PropSetPop", "StringifySlot",
        ] {
            assert!(
                !chunk.contains(form),
                "{form} nel dump flag-off di `{label}`: il modo OFF non è off\n{chunk}"
            );
        }
    }
}
