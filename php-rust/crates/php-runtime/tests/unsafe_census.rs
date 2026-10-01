//! `unsafe` census (DECISION_KERNEL.md §8): every Rust source line that uses
//! the `unsafe` keyword is counted per file, and each file's count is pinned.
//!
//! Rules:
//! - a file outside the allowlist may not contain `unsafe` at all;
//! - an allowlisted file's count must match exactly: adding `unsafe` is
//!   declared by raising the number in the same commit, with a `SAFETY:`
//!   comment at the new site; removing it lowers the number.
//!
//! A line counts when the word `unsafe` appears on it and the line is not a
//! comment (`//`, `///`, `//!`, or a `*` continuation).

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

fn has_word(line: &str, word: &str) -> bool {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    line.match_indices(word).any(|(i, _)| {
        let before = line[..i].chars().next_back();
        let after = line[i + word.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
    })
}

fn unsafe_lines(src: &str) -> usize {
    src.lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("//") && !t.starts_with('*') && has_word(l, "unsafe")
        })
        .count()
}

#[test]
fn unsafe_only_where_declared() {
    // (path, lines using `unsafe`, what it is for).
    let allow: &[(&str, usize, &str)] = &[
        // The value model: hand-rolled refcounted string, array teardown.
        ("php-types/src/zstr.rs", 15, "single-allocation refcounted string (Miri-checked in CI)"),
        ("php-types/src/array.rs", 2, "inline element teardown in PhpArray::drop"),
        // C libraries through FFI.
        ("php-types/src/gdio.rs", 45, "libgd FFI"),
        ("php-types/src/tidyio.rs", 58, "libtidy FFI"),
        ("php-types/src/xsltio.rs", 51, "libxslt FFI"),
        ("php-types/src/zlibio.rs", 7, "zlib FFI"),
        ("php-types/src/netio.rs", 3, "socket syscalls"),
        ("php-types/src/fsown.rs", 5, "chown/chgrp syscalls"),
        ("php-types/src/stream.rs", 1, "fd syscall"),
        ("php-builtins/src/env.rs", 4, "geteuid/getloadavg/uname"),
        ("php-builtins/src/file.rs", 5, "statvfs/access/utimes/flock"),
        ("php-runtime/src/vm/host.rs", 13, "flock/poll/kill/signal/sigprocmask"),
        ("php-cli/src/server.rs", 2, "localtime_r for the log timestamp"),
        // Sound by local reasoning, documented at the site.
        ("php-builtins/src/mbstring.rs", 1, "from_utf8_unchecked on a prefix from_utf8 validated"),
        ("php-runtime/src/vm/mysqli.rs", 1, "non-UTF-8 SQL bytes passed through the mysql crate (relies on it only calling as_bytes)"),
        // C callbacks that carry the Vm as a raw pointer.
        ("php-runtime/src/vm/mod.rs", 1, "Vm pointer recovered in a C callback"),
        ("php-runtime/src/vm/pdo.rs", 2, "SQLite UDF callback: Vm pointer, Send for the callable"),
        ("php-runtime/src/vm/xslt.rs", 1, "XSLT callback: Vm pointer"),
        // Native fiber stacks (owner decision 2026-10-01, DECISION_KERNEL.md §8).
        ("php-runtime/src/vm/coroutines.rs", 2, "fiber body: Vm pointer across the stack switch; the Yielder pointer in Fiber::suspend"),
        // Instrumentation builds only (feature-gated), never the shipped binary's default.
        ("php-types/src/memcensus.rs", 35, "byte census (mem-census)"),
        ("php-runtime/src/vm/zvalcensus.rs", 4, "zval census atexit hooks"),
        ("php-runtime/src/vm/sondaprice.rs", 1, "price probe"),
        ("php-cli/src/main.rs", 5, "counting global allocator (census)"),
        ("php-server/src/main.rs", 18, "counting global allocators (census)"),
    ];
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    rs_files(crates, &mut files);
    assert!(files.len() > 100, "suspicious scan: {} .rs files", files.len());
    let mut bad = Vec::new();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        let n = unsafe_lines(&src);
        let rel = f.strip_prefix(crates).unwrap().to_string_lossy().replace('\\', "/");
        if rel == "php-runtime/tests/unsafe_census.rs" {
            continue; // this file names the keyword in its own strings
        }
        match allow.iter().find(|(p, _, _)| *p == rel) {
            Some((_, pinned, why)) if n != *pinned => bad.push(format!(
                "{rel}: {n} lines use `unsafe`, pinned at {pinned} ({why}) — update the pin in this commit"
            )),
            Some(_) => {}
            None if n > 0 => bad.push(format!(
                "{rel}: {n} lines use `unsafe` and the file is not allowlisted — remove it, or allowlist it with a reason"
            )),
            None => {}
        }
    }
    assert!(bad.is_empty(), "unsafe census:\n{}", bad.join("\n"));
}
