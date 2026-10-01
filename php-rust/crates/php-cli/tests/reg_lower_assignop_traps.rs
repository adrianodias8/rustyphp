//! The SEVEN AssignOp trap fixtures (a–g) as the GATE for the flag-on
//! promotion — flipping the default is FORBIDDEN until they exist and pass
//! BYTE-IDENTICAL in BOTH modes. Fixtures live IN-TREE.
//!
//! The judge HERE is the per-mode comparison (off↔on, EXPLICIT values from
//! the closed list of the mode contract). The TWO-ENGINE leg (against the
//! PHP oracle) lives outside this test, with a NAMED list of pre-existing
//! oracle divergences (undef-lhs without warning; typed-ref zeroed by Zend
//! after a failed AssignOp).
use std::process::Command;

/// (fixture, positive-control marker: the fixture MUST print it — a mute
/// fixture that passes on parity is a silent forgery).
const TRAPS: &[(&str, &str)] = &[
    ("a-rhs-first.php", "byref:"),
    ("b-typed-ref.php", "coerced:"),
    ("c-concat-nonoverlap.php", "a01234"),
    ("d-overflow.php", "float(9.223372036854776E+18)"),
    ("e-undef-warning-order.php", "eff-ran"),
    ("f-destruct-timing.php", "destruct:displaced"),
    ("g-never-commute.php", "g9:"),
];

fn traps_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/assignop-traps")
        .canonicalize()
        .expect("tests/fixtures/assignop-traps exists in-tree")
}

fn run_mode(reg: &str, file: &std::path::Path) -> (String, String, bool) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ferro"));
    c.env_remove("PHPR_REG_LOWER");
    c.env_remove("PHPR_DUMP_OPS");
    c.env("PHPR_REG_LOWER", reg);
    let out = c.arg(file).output().expect("spawn phpr");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.success(),
    )
}

#[test]
fn the_seven_assignop_traps_are_byte_identical_across_modes() {
    let dir = traps_dir();
    // The test counts the traps: a removed/renamed fixture does not
    // silently vanish from the gate (all seven are required).
    let mut found: Vec<String> = std::fs::read_dir(&dir)
        .expect("read traps dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        // The volume's AppleDouble resource forks (`._*.php`) are not fixtures.
        .filter(|n| n.ends_with(".php") && !n.starts_with("._"))
        .collect();
    found.sort();
    assert_eq!(
        found,
        TRAPS.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
        "le fixture su disco non sono le sette trappole nominate"
    );
    for (name, marker) in TRAPS {
        let f = dir.join(name);
        let (off_out, off_err, off_ok) = run_mode("0", &f);
        let (on_out, on_err, on_ok) = run_mode("1", &f);
        assert!(
            off_out.contains(marker),
            "{name}: fixture muta — manca il marker `{marker}`\n{off_out}\n{off_err}"
        );
        assert_eq!(off_ok, on_ok, "{name}: exit status diverge tra i modi");
        assert_eq!(off_out, on_out, "{name}: stdout diverge tra flag-off e flag-on");
        assert_eq!(off_err, on_err, "{name}: stderr diverge tra flag-off e flag-on");
    }
}
