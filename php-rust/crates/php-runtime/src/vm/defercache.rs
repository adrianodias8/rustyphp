//! Cross-request cache of deferred (late-bound) class declarations.
//!
//! A defer-always unit declares each class through `Vm::run_deferred`, which
//! re-lowers the declaration's snippet against the live class image. The
//! include cache keeps whole units; this keeps each deferred snippet's
//! relocated module in the same `UNIT_CACHE` (under a synthetic key), valid
//! for the VM fingerprint it was lowered under, plus the negative result (a
//! supertype still missing) so the next request goes straight to autoload.

use super::*;

impl<'m> super::Vm<'m> {
    /// Replay a cached deferred declaration: the same double-check as an
    /// include hit (append bases, class remap), then the seed delta and the
    /// linked run. `None` = no usable entry (the caller lowers afresh).
    pub(super) fn defer_cache_hit(
        &mut self,
        dk: &UnitKey,
        fp: u64,
        expr: bool,
        caller: usize,
    ) -> Option<Result<Zval, PhpError>> {
        let cu = unit_cache_get(dk, fp).filter(|cu| cu.main_program.is_none())?;
        let remap_base = (self.classes.len(), self.statics.len());
        let (remap, locals) = if cu.module.elided.is_some() {
            self.unit_class_remap_retained(&cu.module)
        } else {
            self.unit_class_remap(&cu.module)
        };
        if cu.static_off != remap_base.1
            || cu.reserved_base != remap_base.0
            || remap[..] != cu.class_remap[..]
            || locals[..] != cu.new_locals[..]
        {
            return None;
        }
        self.apply_seed_delta(&cu.seed_delta);
        uc_log_flush();
        let parked = self.park_module(cu.module);
        Some(self.run_linked(parked, &locals, None, if expr { Some(caller) } else { None }, remap_base, None))
    }

    /// The miss half of the deferred-declaration cache: relocate the fresh
    /// module (as `drive_unit` does), publish it under `fp`, then run it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn defer_link_publish(
        &mut self,
        dk: UnitKey,
        fp: u64,
        mut module: Module,
        program: &Program,
        elide: Option<usize>,
        seed_delta: Rc<SeedDelta>,
        bridge: Option<usize>,
    ) -> Result<Zval, PhpError> {
        let static_off = self.statics.len();
        let reserved_base = self.classes.len();
        let (class_remap, new_locals) = match (module.elided, elide) {
            (Some(_), Some(seed_len)) => {
                let (prog_remap, retained_remap, locals) = self.unit_remap_elided(program, seed_len);
                debug_assert_eq!(retained_remap.len(), module.classes.len());
                assert_fold_equals_mint(&seed_delta, &locals, &module, &dk.path);
                relocate_module_class_ids(&mut module, &prog_remap, static_off);
                (retained_remap, locals)
            }
            _ => {
                let (remap, locals) = self.unit_class_remap(&module);
                assert_fold_equals_mint(&seed_delta, &locals, &module, &dk.path);
                relocate_module_class_ids(&mut module, &remap, static_off);
                (remap, locals)
            }
        };
        uc_stat(|s| s.parked_modules += 1);
        let rc = Rc::new(module);
        unit_cache_put(
            dk,
            CachedUnit {
                fp,
                static_off,
                reserved_base,
                class_remap: class_remap.into(),
                new_locals: new_locals.as_slice().into(),
                seed_delta,
                module: Rc::clone(&rc),
                owner_epoch: VM_EPOCH.with(|e| e.get()),
                main_program: None,
                main_program_net: 0,
                main_program_net_clamped: false,
                main_put_ordinal: 0,
            },
        );
        let parked: &'m Module = self.park_module(rc);
        uc_log_flush();
        self.run_linked(parked, &new_locals, None, bridge, (reserved_base, static_off), None)
    }

}

thread_local! {
    /// Lowerings (deferred declarations and include units) that stopped on a
    /// missing name under a fingerprint: (key, fp) -> the UndefinedClass it
    /// raised. Deterministic, so the next load autoloads it up front.
    static LOWER_NEG: RefCell<HashMap<UnitKey, HashMap<u64, NegEntry>>> =
        RefCell::new(HashMap::default());
}

type NegEntry = (Box<[u8]>, crate::MissingSym, Line);

/// Probed on every include and deferred declaration: by reference, no key
/// copy (the `(UnitKey, fp)` tuple key cloned the whole path per probe).
pub(super) fn lower_neg_get(dk: &UnitKey, fp: u64) -> Option<NegEntry> {
    LOWER_NEG.with(|m| m.borrow().get(dk)?.get(&fp).cloned())
}

pub(super) fn lower_neg_put(dk: &UnitKey, fp: u64, e: NegEntry) {
    LOWER_NEG.with(|m| {
        let mut m = m.borrow_mut();
        // Bounded like the unit cache's ways: a run that keeps minting new
        // keys or fingerprints starts over rather than growing without limit.
        if m.len() >= 1 << 14 {
            m.clear();
        }
        let per = m.entry(dk.clone()).or_default();
        if per.len() >= 16 {
            per.clear();
        }
        per.insert(fp, e);
    })
}

/// Unit-cache key of a deferred declaration: the enclosing file, the line,
/// and the snippet's length and 64-bit digest (no mtime: an edited file
/// yields a different snippet).
pub(super) fn defer_unit_key(file: &[u8], line: Line, snippet: &[u8], digest: u64) -> UnitKey {
    let mut path = Vec::with_capacity(file.len() + 24);
    path.extend_from_slice(b"\0defer\0");
    path.extend_from_slice(file);
    path.push(0);
    path.extend_from_slice(&line.to_le_bytes());
    path.extend_from_slice(&digest.to_le_bytes());
    UnitKey { path, mtime: (0, 0), size: snippet.len() as u64, reg_mode: crate::compile::reg_lower::enabled() }
}


/// opcache's `revalidate_freq`: a file's unit key (mtime, size) is re-read
/// from the filesystem at most this often. `PHPR_REVALIDATE_FREQ` (seconds)
/// sets it; the default is 2 s, opcache's, under the server, and 0 (stat on
/// every include, like PHP without opcache) for the CLI.
fn revalidate_freq() -> std::time::Duration {
    static F: std::sync::OnceLock<std::time::Duration> = std::sync::OnceLock::new();
    *F.get_or_init(|| {
        let secs = std::env::var("PHPR_REVALIDATE_FREQ")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(if php_types::sapi::sapi_name() == "cli-server" { 2 } else { 0 });
        std::time::Duration::from_secs(secs)
    })
}

/// The include cache key of `real` (canonical path `key`), from a stat no
/// older than [`revalidate_freq`].
pub(super) fn revalidated_unit_key(real: &std::path::Path, key: &[u8]) -> Option<UnitKey> {
    thread_local! {
        static STATS: RefCell<HashMap<Vec<u8>, (UnitKey, std::time::Instant)>> = RefCell::new(HashMap::default());
    }
    let freq = revalidate_freq();
    let now = std::time::Instant::now();
    if !freq.is_zero() {
        let hit = STATS.with(|m| {
            m.borrow().get(key).filter(|(_, t)| now.duration_since(*t) < freq).map(|(k, _)| k.clone())
        });
        if hit.is_some() {
            return hit;
        }
    }
    let uk = std::fs::metadata(real).ok().and_then(|m| unit_key_for(key, &m))?;
    if !freq.is_zero() {
        STATS.with(|m| {
            let mut m = m.borrow_mut();
            if m.len() >= 1 << 16 {
                m.clear();
            }
            m.insert(key.to_vec(), (uk.clone(), now));
        });
    }
    Some(uk)
}

/// Per-thread include index (server mode, absolute include paths): one
/// lookup by the bytes `include` was given replaces the realpath cache, the
/// stat cache and the unit-cache probe of a warm include — each a separate
/// cold hash-map walk (op-time census: ~1.3 µs of map probes per include,
/// 747 includes per Drupal request). Valid for `revalidate_freq`, like the
/// stat it holds; the memoized hit is the unit cache's own entry for that
/// fingerprint (an entry stays valid for its fingerprint even if the cache
/// later supersedes or evicts it).
struct IncEntry {
    real: std::path::PathBuf,
    key: Rc<[u8]>,
    uk: UnitKey,
    t: std::time::Instant,
    memo: Option<(u64, CachedUnit)>,
}

thread_local! {
    static INC_INDEX: RefCell<HashMap<Box<[u8]>, IncEntry>> = RefCell::new(HashMap::default());
}

/// `(real path, canonical key, unit key)` of `path` if indexed and fresh.
pub(super) fn inc_index_get(path: &[u8]) -> Option<(std::path::PathBuf, Vec<u8>, UnitKey)> {
    let freq = revalidate_freq();
    if freq.is_zero() || path.first() != Some(&b'/') {
        return None;
    }
    INC_INDEX.with(|m| {
        let m = m.borrow();
        let e = m.get(path)?;
        (e.t.elapsed() < freq).then(|| (e.real.clone(), e.key.to_vec(), e.uk.clone()))
    })
}

pub(super) fn inc_index_put(path: &[u8], real: &std::path::Path, key: &[u8], uk: &UnitKey) {
    if revalidate_freq().is_zero() || path.first() != Some(&b'/') {
        return;
    }
    INC_INDEX.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= 1 << 16 {
            m.clear();
        }
        m.insert(
            path.into(),
            IncEntry { real: real.to_path_buf(), key: key.into(), uk: uk.clone(), t: std::time::Instant::now(), memo: None },
        );
    })
}

/// The memoized unit-cache hit for `path` at fingerprint `fp`.
pub(super) fn inc_memo_get(path: &[u8], uk: &UnitKey, fp: u64) -> Option<CachedUnit> {
    INC_INDEX.with(|m| {
        let m = m.borrow();
        let e = m.get(path)?;
        match &e.memo {
            Some((f, cu)) if *f == fp && e.uk == *uk => Some(cu.clone()),
            _ => None,
        }
    })
}

pub(super) fn inc_memo_put(path: &[u8], fp: u64, cu: &CachedUnit) {
    INC_INDEX.with(|m| {
        if let Some(e) = m.borrow_mut().get_mut(path) {
            e.memo = Some((fp, cu.clone()));
        }
    })
}

/// `include` resolution of an absolute path through a per-thread cache keyed
/// by its bytes (`realpath_cache_ttl`, 120 s): no candidate `PathBuf`, no
/// component-wise path hashing (~330 ns per include before). `None`: no
/// readable file.
pub(super) fn resolve_absolute_include(path: &[u8]) -> Option<std::path::PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    thread_local! {
        static ABS: RefCell<HashMap<Box<[u8]>, (std::path::PathBuf, std::time::Instant)>> =
            RefCell::new(HashMap::default());
    }
    const ABS_TTL: std::time::Duration = std::time::Duration::from_secs(120);
    let now = std::time::Instant::now();
    let hit = ABS.with(|m| {
        m.borrow().get(path).filter(|(_, t)| now.duration_since(*t) < ABS_TTL).map(|(r, _)| r.clone())
    });
    if hit.is_some() {
        return hit;
    }
    let p = std::path::Path::new(std::ffi::OsStr::from_bytes(path));
    if !p.is_file() {
        return None;
    }
    let r = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    ABS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= 1 << 16 {
            m.clear();
        }
        m.insert(path.into(), (r.clone(), now));
    });
    Some(r)
}
