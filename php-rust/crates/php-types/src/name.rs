//! Interned identifiers and a name-keyed table with a pointer fast path
//! (PARITY_PLAN.md stage 1, item 1).
//!
//! Zend interns every identifier at compile time (`zend_insert_literal`,
//! Zend/zend_compile.c:574-582), so a lookup with a compile-time name usually
//! finds its key by pointer. Here the compiler interns names into one
//! per-thread allocation each ([`intern_name`]), and [`NameTable`] probes a
//! small table keyed by the name's ADDRESS before hashing bytes.
//!
//! Correctness never depends on interning: a pointer probe hits only when the
//! lookup slice has the same start address and length as a key the table
//! owns (so the same live, immutable bytes), and every miss falls back to the
//! byte-keyed map. An un-interned name is only slower.

use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    /// Every identifier interned on this thread. Never cleared: it holds names
    /// of compiled code only (bounded by the code the thread compiles), and
    /// compiled units are `Rc`-owned, so they never leave the thread.
    static NAMES: RefCell<FxHashSet<Rc<[u8]>>> = RefCell::new(FxHashSet::default());
}

/// The canonical allocation of identifier `name` on this thread.
pub fn intern_name(name: &[u8]) -> Rc<[u8]> {
    NAMES.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(r) = s.get(name) {
            return r.clone();
        }
        let r: Rc<[u8]> = Rc::from(name);
        s.insert(r.clone());
        r
    })
}

/// [`intern_name`] for a name already in an `Rc`: reuses its allocation when
/// the name is new.
pub fn intern_rc(name: &Rc<[u8]>) -> Rc<[u8]> {
    NAMES.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(r) = s.get(&**name) {
            return r.clone();
        }
        s.insert(name.clone());
        name.clone()
    })
}

const EMPTY: u32 = u32::MAX;

#[inline]
fn ptr_hash(p: *const u8) -> usize {
    ((p as usize as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) as usize
}

/// A map from names to `V`: values in insertion order, a byte-keyed index
/// (the authority, and the iteration order of [`NameTable::iter`] /
/// [`NameTable::values`], identical to the `FxHashMap<Box<[u8]>, V>` it
/// replaces), and an open-addressing table of key addresses for lookups made
/// with the very allocation the key was built from.
#[derive(Clone)]
pub struct NameTable<V> {
    vals: Vec<(Rc<[u8]>, V)>,
    index: FxHashMap<Rc<[u8]>, u32>,
    ptrs: Box<[u32]>,
}

impl<V> Default for NameTable<V> {
    fn default() -> Self {
        NameTable { vals: Vec::new(), index: FxHashMap::default(), ptrs: Box::new([]) }
    }
}

impl<V> NameTable<V> {
    pub fn len(&self) -> usize {
        self.vals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vals.is_empty()
    }

    /// Insert `key` (interned here), replacing and returning a previous value.
    /// A replaced entry keeps its position, like a map overwrite.
    pub fn insert(&mut self, key: &[u8], v: V) -> Option<V> {
        if let Some(&i) = self.index.get(key) {
            return Some(std::mem::replace(&mut self.vals[i as usize].1, v));
        }
        let k = intern_name(key);
        let i = self.vals.len() as u32;
        self.index.insert(k.clone(), i);
        self.vals.push((k, v));
        if self.vals.len() * 2 > self.ptrs.len() {
            let cap = (self.vals.len() * 2).next_power_of_two().max(8);
            self.ptrs = vec![EMPTY; cap].into_boxed_slice();
            for j in 0..self.vals.len() {
                self.ptr_insert(j as u32);
            }
        } else {
            self.ptr_insert(i);
        }
        None
    }

    fn ptr_insert(&mut self, i: u32) {
        let mask = self.ptrs.len() - 1;
        let mut s = ptr_hash(self.vals[i as usize].0.as_ptr()) & mask;
        while self.ptrs[s] != EMPTY {
            s = (s + 1) & mask;
        }
        self.ptrs[s] = i;
    }

    #[inline]
    fn find(&self, name: &[u8]) -> Option<usize> {
        self.find_ptr(name).or_else(|| self.index.get(name).map(|&i| i as usize))
    }

    #[inline]
    fn find_ptr(&self, name: &[u8]) -> Option<usize> {
        if !self.ptrs.is_empty() {
            let mask = self.ptrs.len() - 1;
            let mut s = ptr_hash(name.as_ptr()) & mask;
            loop {
                let i = self.ptrs[s];
                if i == EMPTY {
                    break;
                }
                let k = &self.vals[i as usize].0;
                if k.as_ptr() == name.as_ptr() && k.len() == name.len() {
                    return Some(i as usize);
                }
                s = (s + 1) & mask;
            }
        }
        None
    }

    /// The value of `name` only when `name` is the very allocation a key was
    /// built from (no byte hashing): for callers with a cheaper fallback.
    #[inline]
    pub fn get_ptr(&self, name: &[u8]) -> Option<&V> {
        self.find_ptr(name).map(|i| &self.vals[i].1)
    }

    #[inline]
    pub fn get(&self, name: &[u8]) -> Option<&V> {
        self.find(name).map(|i| &self.vals[i].1)
    }

    pub fn get_mut(&mut self, name: &[u8]) -> Option<&mut V> {
        self.find(name).map(|i| &mut self.vals[i].1)
    }

    pub fn contains_key(&self, name: &[u8]) -> bool {
        self.find(name).is_some()
    }

    /// Entries in the byte index's order (the replaced map's order).
    pub fn iter(&self) -> Iter<'_, V> {
        Iter { order: self.index.values(), vals: &self.vals }
    }

    pub fn values(&self) -> Values<'_, V> {
        Values(self.iter())
    }

    /// Values in insertion order (callers mutate each value independently).
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.vals.iter_mut().map(|(_, v)| v)
    }
}

/// [`NameTable::iter`]: a plain struct (no drop glue), so a borrow of the
/// table ends with the expression that used it.
pub struct Iter<'a, V> {
    order: std::collections::hash_map::Values<'a, Rc<[u8]>, u32>,
    vals: &'a [(Rc<[u8]>, V)],
}

impl<'a, V> Iterator for Iter<'a, V> {
    type Item = (&'a Rc<[u8]>, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        self.order.next().map(|&i| {
            let (k, v) = &self.vals[i as usize];
            (k, v)
        })
    }
}

/// [`NameTable::values`].
pub struct Values<'a, V>(Iter<'a, V>);

impl<'a, V> Iterator for Values<'a, V> {
    type Item = &'a V;
    fn next(&mut self) -> Option<&'a V> {
        self.0.next().map(|(_, v)| v)
    }
}

impl<V: PartialEq> PartialEq for NameTable<V> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self.vals.iter().all(|(k, v)| other.get(k).is_some_and(|o| o == v))
    }
}

impl<V: std::fmt::Debug> std::fmt::Debug for NameTable<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter().map(|(k, v)| (String::from_utf8_lossy(k), v))).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_and_byte_lookups_agree() {
        let mut t = NameTable::default();
        for (i, n) in ["a", "bb", "ccc", "", "a\0b", "x"].iter().enumerate() {
            assert!(t.insert(n.as_bytes(), i).is_none());
        }
        assert_eq!(t.insert(b"bb", 9), Some(1));
        for (i, n) in ["a", "bb", "ccc", "", "a\0b", "x"].iter().enumerate() {
            let want = if *n == "bb" { 9 } else { i };
            assert_eq!(t.get(n.as_bytes()), Some(&want));
            assert_eq!(t.get(&intern_name(n.as_bytes())), Some(&want));
            assert_eq!(t.get(&n.as_bytes().to_vec()), Some(&want));
        }
        let k = intern_name(b"ccc");
        assert_eq!(t.get(&k[..2]), None);
        assert_eq!(t.get_ptr(&k), Some(&2));
        assert_eq!(t.get_ptr(&b"ccc".to_vec()), None);
        assert_eq!(t.get(b"zz"), None);
        assert_eq!(t.len(), 6);
    }
}
