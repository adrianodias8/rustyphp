//! Line-count caps on the Rust sources: an anti-regrowth tooth (adopted from
//! upstream, which counted `lines()` exactly like `wc -l` and refused any
//! textual pattern matching for the measure).
//!
//! Rules:
//! - a file outside the allowlist may not exceed `CAP_NEW` lines;
//! - an allowlisted file may not exceed its cap, which is the exact count
//!   the day it was set: growing it is declared by raising the number, in
//!   the same commit, with the reason updated;
//! - anti-slack: a cap more than `SLACK_MAX` lines above the real count must
//!   be lowered in the same commit that shrank the file.

use std::path::{Path, PathBuf};

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn no_source_file_over_cap() {
    const CAP_NEW: usize = 2000;
    const SLACK_MAX: usize = 200;
    const CAP_C_MOD: usize = 2062; // +5 prelude_shared count
    const CAP_B_MBSTRING: usize = 2032;
    const CAP_VM_MOD: usize = 26895; // +11 worker/zygote boot superglobals seeded at Vm construction; +6 unserialize E: (Ser::Enum arm, O: of an enum rejected); +73 classic-mode caches (deferred decls, preg, realpath); +75 incremental unit_fp digests, prelude-prefix skip in run_linked, trait-include negative probe; +26 include index/memo plumbing in run_include; +66 drop-driven destructor hooks (vm/gcdrop.rs: fields, note/sweep/collect branches, Zend teardown order); +1 D-24/D-25 sentinel ignores; +47 engine program step 2: magic/memo/link-cache fields, lazy_prop_access inline split, single-lookup unserialize fields; +26 step 3: include bridging from the includer's names, debug-only class-name check; +9 step 4: operand-stack pre-reserve, scope-free unit skip; +3 step 5: literal-name method memo field
    const CAP_VM_HOST: usize = 7909; // +5: unserialize validate/direct dispatch (vm/unser.rs); +6: positioned unserialize failures (E:) routed through unser_offset_fail
    const CAP_VM_RUN: usize = 7652; // +2 borrowed user_wrapper_url scan; includes +29 upstream L-RT1 (S-183, Ret in place), rebased 2026-10-02
    const CAP_T_EVAL: usize = 4773;
    const CAP_T_BUILTINS: usize = 4772;
    const CAP_LOWER_MOD: usize = 4013; // +7: DeferredDecl::digest
    const CAP_VM_DOM: usize = 3675;
    const CAP_BIG5: usize = 3372;
    const CAP_B_STRING: usize = 2870; // +5 memmem search, null-only param-name lookup
    const CAP_B_FILE: usize = 2761;
    const CAP_C_EXPR: usize = 2779;
    const CAP_B_DATE: usize = 2458;
    const CAP_BYTECODE: usize = 2474; // +8: MethodIc keyed on (receiver, calling scope); +9 CompiledClass::declares_private_props, Module::prelude_shared
    const CAP_PREG: usize = 2295;
    const CAP_MEMCENSUS: usize = 2262;
    const CAP_LOWER_CLASS: usize = 2195;
    const CAP_VM_ARRAYS: usize = 2228; // +4 slot-indexed read in field_get
    const CAP_LSP: usize = 2094;
    const CAP_B_FILEINFO: usize = 2083;
    const CAP_WORKER_POOL: usize = 2074;
    const CAP_LOWER_EXPR: usize = 2063;
    // (path, cap, why it is allowed to be this big). Caps re-declared on
    // 2026-10-01 after the comments were translated from Italian.
    let allow: &[(&str, usize, &str)] = &[
        ("php-runtime/src/vm/mod.rs", CAP_VM_MOD, "the VM monolith — the split is the standing target"),
        ("php-runtime/src/vm/host.rs", CAP_VM_HOST, "host builtins (call-a-callable, introspection, streams)"),
        ("php-runtime/src/vm/run.rs", CAP_VM_RUN, "the dispatch loop — layout-sensitive, split last or never"),
        ("php-runtime/tests/eval.rs", CAP_T_EVAL, "the eval test battery"),
        ("php-builtins/tests/builtins.rs", CAP_T_BUILTINS, "the builtins test battery"),
        ("php-runtime/src/lower/mod.rs", CAP_LOWER_MOD, "lowering"),
        ("php-runtime/src/vm/dom.rs", CAP_VM_DOM, "ext/dom"),
        ("php-types/src/big5.rs", CAP_BIG5, "generated table, fixed cap"),
        ("php-builtins/src/string.rs", CAP_B_STRING, "string builtins"),
        ("php-builtins/src/file.rs", CAP_B_FILE, "file builtins"),
        ("php-runtime/src/compile/expr.rs", CAP_C_EXPR, "expression compiler"),
        ("php-builtins/src/date.rs", CAP_B_DATE, "date builtins"),
        ("php-runtime/src/bytecode.rs", CAP_BYTECODE, "the op set and its inline caches"),
        ("php-runtime/src/preg.rs", CAP_PREG, "preg"),
        ("php-types/src/memcensus.rs", CAP_MEMCENSUS, "census instrumentation"),
        ("php-runtime/src/lower/class.rs", CAP_LOWER_CLASS, "class lowering"),
        ("php-runtime/src/vm/arrays.rs", CAP_VM_ARRAYS, "array paths"),
        ("php-runtime/src/lsp_check.rs", CAP_LSP, "LSP checks"),
        ("php-builtins/src/fileinfo.rs", CAP_B_FILEINFO, "fileinfo"),
        ("php-server/src/worker_pool.rs", CAP_WORKER_POOL, "the axum worker pool"),
        ("php-runtime/src/lower/expr.rs", CAP_LOWER_EXPR, "expression lowering"),
        ("php-runtime/src/compile/mod.rs", CAP_C_MOD, "the compiler core (statements, const folding)"),
        ("php-builtins/src/mbstring.rs", CAP_B_MBSTRING, "mbstring builtins"),
    ];
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    rs_files(crates, &mut files);
    assert!(files.len() > 100, "suspicious scan: {} .rs files", files.len());
    let mut bad = Vec::new();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        let n = src.lines().count();
        let rel = f.strip_prefix(crates).unwrap().to_string_lossy().replace('\\', "/");
        match allow.iter().find(|(p, _, _)| *p == rel) {
            Some((_, cap, why)) => {
                if n > *cap {
                    bad.push(format!(
                        "{rel}: {n} lines > cap {cap} ({why}) — regrowth: shrink it, or raise the cap in this commit with the reason"
                    ));
                } else if cap - n > SLACK_MAX {
                    bad.push(format!(
                        "{rel}: {n} lines, cap {cap} is slack (> {SLACK_MAX}) — lower the cap in the same commit"
                    ));
                }
            }
            None if n > CAP_NEW => bad.push(format!(
                "{rel}: {n} lines > {CAP_NEW} (not allowlisted) — split it, or allowlist it with a reason"
            )),
            None => {}
        }
    }
    assert!(bad.is_empty(), "line caps:\n{}", bad.join("\n"));
}
