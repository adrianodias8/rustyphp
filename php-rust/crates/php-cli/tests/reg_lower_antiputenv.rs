//! The register-lowering mode is decided by the ENVIRONMENT AT SPAWN and
//! sealed eagerly at bootstrap (`seal_reg_lower_mode`, the first act of
//! both mains) — `putenv()` from PHP code cannot flip it in ANY direction.
//!
//! Arms run on the REAL funnel (spawned binary, like reg_lower_funnel),
//! derived from the mode contract (value-parsed spelling, closed list:
//! absent=>DEFAULT_ON, `=1`=>on, `=0`=>off, other=>default+warning). Every
//! MODE arm uses an EXPLICIT value from the closed list at spawn (never
//! absence alone: an arm written on absence would silently test the NEW
//! mode after the default flips; the `absent` arm derives from the
//! contract). putenv() must be impotent in ALL directions: set, unset, and
//! the `=0` opt-out introduced by the contract.
//!
//! Honest note: in the CLI the first read of the flag happened anyway at
//! the compile of `{main}`, before any putenv — the CLI arm pins the
//! end-to-end INVARIANT as a regression; the window that was really open
//! was the server, closed BY CONSTRUCTION by the seal in its main (same
//! `seal_reg_lower_mode`, statically auditable).

use std::process::Command;

/// Foldable body: the same shape as reg_lower_funnel (BinarySC/CmpJmpSC/
/// BinaryDst expected flag-on in the `{main}` of the included unit).
const INCLUDED: &[u8] =
    br#"<?php $s=0; for($i=0;$i<100;$i++){ $s += $i*3 - ($i>>2); } echo $s,"\n";"#;

const REG_FORMS: [&str; 4] = ["BinarySC", "CmpJmpSC", "BinaryDst", "BinarySS"];

fn run_case(spawn: Option<&str>, tag: &str, putenv_stmt: &str) -> (String, String, String) {
    let dir = std::env::temp_dir();
    let tag = format!("{}-{tag}", std::process::id());
    let inc = dir.join(format!("antiputenv-inc-{tag}.php"));
    let main = dir.join(format!("antiputenv-main-{tag}.php"));
    std::fs::write(&inc, INCLUDED).expect("write included php");
    std::fs::write(
        &main,
        format!("<?php {putenv_stmt}; include '{}';", inc.display()).into_bytes(),
    )
    .expect("write main php");

    let mut c = Command::new(env!("CARGO_BIN_EXE_ferro"));
    c.env_remove("PHPR_REG_LOWER");
    c.env_remove("PHPR_DUMP_OPS");
    c.env("PHPR_DUMP_OPS", "1");
    if let Some(v) = spawn {
        c.env("PHPR_REG_LOWER", v);
    }
    let out = c.arg(&main).output().expect("spawn phpr");
    let inc_name = inc.file_name().unwrap().to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&inc);
    let _ = std::fs::remove_file(&main);
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        inc_name,
    )
}

/// The dump chunk of the INCLUDED unit (compiled AFTER the putenv).
///
/// Confirmed pitfall: the path of the included unit appears as a CONSTANT
/// (`cst… Str("…/inc.php")`) inside the `{main}` chunk, so a substring
/// match hooks the WRONG chunk (first match on main). The right chunk is
/// the one whose HEADER is the path: after `split("== unit ")` every chunk
/// STARTS with the path of its own unit.
fn included_chunk<'a>(stderr: &'a str, inc_name: &str) -> &'a str {
    stderr
        .split("== unit ")
        .find(|u| {
            u.split_whitespace()
                .next()
                .is_some_and(|p| p.ends_with(inc_name))
        })
        .unwrap_or_else(|| {
            panic!("no dump chunk HEADED by {inc_name} — PHPR_DUMP_OPS dead?\n{stderr}")
        })
}

/// `=0` is the contract's opt-out — BEFORE the contract it TURNED ON the
/// pass (`is_some()`). Flip-proof arm: it does not depend on the default.
#[test]
fn explicit_zero_is_off_and_putenv_set_cannot_turn_the_pass_on() {
    // Baseline of the same mode without putenv: the program output must
    // not depend on the putenv (parity), and no hand-computed literal is
    // pinned.
    let (base, _, _) = run_case(Some("0"), "zero", "");
    let (out, err, inc_name) = run_case(Some("0"), "zero", "putenv('PHPR_REG_LOWER=1')");
    assert_eq!(out, base, "putenv(set) cambia l'output del programma");
    let chunk = included_chunk(&err, &inc_name);
    // Positive control: the chunk contains opcodes (the dump really bites).
    assert!(
        chunk.contains("CmpJmp") || chunk.contains("Binary"),
        "dump chunk vuoto/insensato per {inc_name}:\n{chunk}"
    );
    for form in REG_FORMS {
        assert!(
            !chunk.contains(form),
            "{form} nell'unità inclusa con `=0` allo spawn e putenv(set): \
             contratto value-parsed o sigillo rotti\n{chunk}"
        );
    }
}

/// Flag ABSENT: emission follows the contract's NAMED DEFAULT (derived
/// arm, it re-derives itself on the flip; the STRONG flip tripwire is the
/// unit test `mode_contract_default_is_off_pre_flip`), and putenv towards
/// the opposite mode stays impotent.
#[test]
fn absent_flag_follows_the_named_default_and_putenv_cannot_move_it() {
    let (base, base_err, base_inc) = run_case(None, "absent", "");
    let base_has = {
        let c = included_chunk(&base_err, &base_inc);
        REG_FORMS.iter().any(|f| c.contains(f))
    };
    assert_eq!(
        base_has,
        php_runtime::REG_LOWER_DEFAULT_ON,
        "l'emissione con flag assente non segue il default nominato del contratto"
    );
    let flip_stmt = if php_runtime::REG_LOWER_DEFAULT_ON {
        "putenv('PHPR_REG_LOWER=0')"
    } else {
        "putenv('PHPR_REG_LOWER=1')"
    };
    let (out, err, inc_name) = run_case(None, "absent", flip_stmt);
    assert_eq!(out, base, "putenv cambia l'output del programma");
    let chunk = included_chunk(&err, &inc_name);
    assert_eq!(
        REG_FORMS.iter().any(|f| chunk.contains(f)),
        base_has,
        "putenv ha mosso il modo di DEFAULT dopo il boot\n{chunk}"
    );
}

#[test]
fn putenv_unset_after_boot_cannot_turn_the_pass_off() {
    let (base, _, _) = run_case(Some("1"), "unset", "");
    let (out, err, inc_name) = run_case(Some("1"), "unset", "putenv('PHPR_REG_LOWER')");
    assert_eq!(out, base, "putenv(unset) cambia l'output del programma");
    let chunk = included_chunk(&err, &inc_name);
    assert!(
        REG_FORMS.iter().any(|f| chunk.contains(f)),
        "nessuna forma registro nell'unità inclusa DOPO putenv(unset) con \
         flag allo spawn: il sigillo non tiene nella direzione set→unset\n{chunk}"
    );
}

/// The direction OPENED by the contract: the `=0` opt-out written via
/// putenv after boot must be as impotent as the unset.
#[test]
fn putenv_zero_after_boot_cannot_turn_the_pass_off() {
    let (base, _, _) = run_case(Some("1"), "pzero", "");
    let (out, err, inc_name) = run_case(Some("1"), "pzero", "putenv('PHPR_REG_LOWER=0')");
    assert_eq!(out, base, "putenv(=0) cambia l'output del programma");
    let chunk = included_chunk(&err, &inc_name);
    assert!(
        REG_FORMS.iter().any(|f| chunk.contains(f)),
        "nessuna forma registro nell'unità inclusa DOPO putenv('=0') con \
         `=1` allo spawn: il sigillo non tiene nella direzione set→zero\n{chunk}"
    );
}

/// Out-of-grammar value: fallback to the default is NEVER silent (a stderr
/// warning naming the grammar) and emission matches the absent arm.
#[test]
fn out_of_grammar_value_is_default_plus_loud_warning() {
    let (base, base_err, base_inc) = run_case(None, "junkbase", "");
    let base_has = {
        let c = included_chunk(&base_err, &base_inc);
        REG_FORMS.iter().any(|f| c.contains(f))
    };
    let (out, err, inc_name) = run_case(Some("junk"), "junk", "");
    assert_eq!(out, base, "un valore fuori grammatica cambia l'output del programma");
    assert!(
        err.contains("fuori grammatica"),
        "nessun warning per PHPR_REG_LOWER=junk: fallback silenzioso\n{err}"
    );
    let chunk = included_chunk(&err, &inc_name);
    assert_eq!(
        REG_FORMS.iter().any(|f| chunk.contains(f)),
        base_has,
        "un valore fuori grammatica non cade sul default del contratto\n{chunk}"
    );
}
