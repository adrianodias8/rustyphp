//! Property-access shortcuts that skip name lookups whose outcome is already
//! known (HWCOUNTERS_DRUPAL.md: ~16 name lookups per property op).
//!
//! `magic_applies` answers "would `__get`/`__set`/`__isset`/`__unset` be
//! dispatched for this access?". It used to resolve the property (one or two
//! `prop_info` hash lookups), test presence by name (two `slot_of` scans) and
//! only then look for the magic method, although a class without that method
//! answers "no" whatever the property state. Zend tests `ce->__get` first
//! (zend_std_read_property); this is the same test, with the per-class answer
//! cached for the run.
//!
//! [`Vm::resolve_prop_memo`] caches `resolve_prop_access` for names that live
//! in the ops (`&'m [u8]`): such a name's address is unique for the whole run
//! (the borrow outlives the cache, so it cannot be freed and reused), which
//! makes it an interned key — a hit is a pointer and two-integer compare, no
//! hashing of the bytes, no `bcmp`. Zend gets the same from interned strings
//! plus the runtime cache.

use super::*;

/// Bit of each property magic method in [`Vm::class_magic`]'s mask.
const MAGIC_GET: u8 = 1;
const MAGIC_SET: u8 = 2;
const MAGIC_ISSET: u8 = 4;
const MAGIC_UNSET: u8 = 8;
/// The entry has been computed (an all-zero entry is "not yet").
const MAGIC_KNOWN: u8 = 0x80;

impl<'m> super::Vm<'m> {
    /// Whether class `cid` (inherited methods included) has the magic method
    /// `magic_name`; any other name answers `true` (no shortcut).
    ///
    /// Resolved once per class per run: the class table only grows and an id
    /// is never rebound within a `Vm`, and a class's methods are fixed once
    /// it is in the table, so an entry never goes stale.
    #[inline]
    pub(super) fn class_has_magic(&self, cid: ClassId, magic_name: &[u8]) -> bool {
        let bit = match magic_name {
            b"__get" => MAGIC_GET,
            b"__set" => MAGIC_SET,
            b"__isset" => MAGIC_ISSET,
            b"__unset" => MAGIC_UNSET,
            _ => return true,
        };
        self.class_magic(cid) & bit != 0
    }

    fn class_magic(&self, cid: ClassId) -> u8 {
        if let Some(&m) = self.magic_bits.borrow().get(cid) {
            if m != 0 {
                return m;
            }
        }
        let mut m = MAGIC_KNOWN;
        for (bit, name) in [
            (MAGIC_GET, &b"__get"[..]),
            (MAGIC_SET, b"__set"),
            (MAGIC_ISSET, b"__isset"),
            (MAGIC_UNSET, b"__unset"),
        ] {
            if oop::resolve_method_runtime(&self.classes, cid, name).is_some() {
                m |= bit;
            }
        }
        let mut t = self.magic_bits.borrow_mut();
        if t.len() <= cid {
            t.resize(cid + 1, 0);
        }
        t[cid] = m;
        m
    }
}

/// One [`Vm::resolve_prop_memo`] entry; `name == 0` is empty.
#[derive(Clone, Copy)]
pub(super) struct PropMemo<'m> {
    name: usize,
    len: u32,
    cid: u32,
    scope: u32,
    res: PropAccess<'m>,
}

/// Entries of the direct-mapped memo (a power of two).
const MEMO_SIZE: usize = 1024;

impl<'m> super::Vm<'m> {
    /// `resolve_prop_access(classes, cid, name, scope)` through a per-run
    /// cache. The resolution is a pure function of the class table, which
    /// only grows (and whose entries never change); entries are only stored
    /// when every id involved is already in the table, so a later
    /// declaration cannot change a cached answer.
    pub(super) fn resolve_prop_memo(&self, cid: ClassId, name: &'m [u8], scope: Option<ClassId>) -> PropAccess<'m> {
        let p = name.as_ptr() as usize;
        let sk = crate::bytecode::PropIc::scope_key(scope);
        let h = (p >> 3) ^ (cid.wrapping_mul(0x9E37_79B9)) ^ (sk as usize).wrapping_mul(0x85EB_CA6B);
        let i = (h ^ (h >> 10)) & (MEMO_SIZE - 1);
        if let Some(e) = self.prop_memo.borrow().get(i) {
            if e.name == p && e.len as usize == name.len() && e.cid as usize == cid && e.scope == sk {
                return e.res;
            }
        }
        let res = oop::resolve_prop_access(&self.classes, cid, name, scope);
        let n = self.classes.len();
        if cid < n && scope.is_none_or(|s| s < n) {
            let mut t = self.prop_memo.borrow_mut();
            if t.is_empty() {
                t.resize(MEMO_SIZE, PropMemo { name: 0, len: 0, cid: 0, scope: 0, res: PropAccess::Dynamic });
            }
            t[i] = PropMemo { name: p, len: name.len() as u32, cid: cid as u32, scope: sk, res };
        }
        res
    }
}
