//! The object store: entries by key (with Vary variants), a tag index for
//! bans, FIFO eviction under a byte budget, hit-for-pass markers.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::StatusCode;
use regex::Regex;

use crate::policy::Lifetime;

/// One stored response.
#[derive(Debug)]
pub struct Entry {
    pub status: StatusCode,
    /// Client-ready headers (no hop-by-hop, no Content-Length, tags stripped
    /// unless exposed).
    pub headers: HeaderMap,
    pub body: Bytes,
    pub tags: Vec<String>,
    /// The tags joined by spaces: what a Varnish-style regex ban matches.
    pub tag_line: String,
    pub host: String,
    /// Path and query: what an `X-Url` ban matches.
    pub url: String,
    /// The request header values this variant was stored for.
    pub vary: Vec<(HeaderName, Option<HeaderValue>)>,
    pub stored: Instant,
    pub life: Lifetime,
}

impl Entry {
    pub fn age(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.stored)
    }
    pub fn fresh(&self, now: Instant) -> bool {
        self.age(now) < self.life.ttl
    }
    pub fn in_grace(&self, now: Instant) -> bool {
        self.age(now) < self.life.ttl + self.life.grace
    }
    /// Kept this long: grace or stale-if-error, whichever reaches further.
    pub fn retained(&self, now: Instant) -> bool {
        self.age(now) < self.life.ttl + self.life.grace.max(self.life.stale_if_error)
    }
    pub fn usable_on_error(&self, now: Instant) -> bool {
        self.age(now) < self.life.ttl + self.life.stale_if_error
    }
    pub fn matches(&self, req: &HeaderMap) -> bool {
        self.vary.iter().all(|(n, v)| req.get(n) == v.as_ref())
    }
    fn size(&self) -> usize {
        let headers: usize = self.headers.iter().map(|(n, v)| n.as_str().len() + v.len() + 32).sum();
        self.body.len() + headers + 2 * self.tag_line.len() + 48 * self.tags.len() + self.url.len() + 256
    }
}

pub enum Lookup {
    Fresh(Arc<Entry>),
    /// Past TTL, within grace: serve it and revalidate in the background.
    Stale(Arc<Entry>),
    /// Fetch; the entry (past grace, within stale-if-error) is the fallback.
    Miss(Option<Arc<Entry>>),
}

struct Slot {
    key: Arc<str>,
    entry: Arc<Entry>,
    size: usize,
}

pub struct Store {
    next_id: u64,
    slots: HashMap<u64, Slot>,
    by_key: HashMap<Arc<str>, Vec<u64>>,
    by_tag: HashMap<String, HashSet<u64>>,
    fifo: VecDeque<u64>,
    bytes: usize,
    max_bytes: usize,
    pass: HashMap<Arc<str>, Instant>,
    /// Bumped by every ban: a fetch that started before a ban is not stored.
    pub ban_gen: u64,
}

impl Store {
    pub fn new(max_bytes: usize) -> Store {
        Store {
            next_id: 0,
            slots: HashMap::new(),
            by_key: HashMap::new(),
            by_tag: HashMap::new(),
            fifo: VecDeque::new(),
            bytes: 0,
            max_bytes,
            pass: HashMap::new(),
            ban_gen: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn lookup(&mut self, key: &str, req: &HeaderMap, now: Instant) -> Lookup {
        let Some(ids) = self.by_key.get(key) else { return Lookup::Miss(None) };
        let Some(id) = ids.iter().copied().find(|id| self.slots[id].entry.matches(req)) else {
            return Lookup::Miss(None);
        };
        let e = self.slots[&id].entry.clone();
        if e.fresh(now) {
            Lookup::Fresh(e)
        } else if e.in_grace(now) {
            Lookup::Stale(e)
        } else if e.retained(now) {
            Lookup::Miss(Some(e))
        } else {
            self.remove(id);
            Lookup::Miss(None)
        }
    }

    /// Stores `entry` under `key`, replacing the variant with the same Vary
    /// values, then evicts oldest-first down to the budget.
    pub fn insert(&mut self, key: &Arc<str>, entry: Arc<Entry>) {
        if let Some(ids) = self.by_key.get(key) {
            let same: Vec<u64> = ids.iter().copied().filter(|id| self.slots[id].entry.vary == entry.vary).collect();
            for id in same {
                self.remove(id);
            }
        }
        self.pass.remove(key);
        let id = self.next_id;
        self.next_id += 1;
        let size = entry.size();
        for t in &entry.tags {
            self.by_tag.entry(t.clone()).or_default().insert(id);
        }
        self.by_key.entry(key.clone()).or_default().push(id);
        self.slots.insert(id, Slot { key: key.clone(), entry, size });
        self.fifo.push_back(id);
        self.bytes += size;
        while self.bytes > self.max_bytes {
            match self.fifo.pop_front() {
                Some(old) => {
                    self.remove(old);
                }
                None => break,
            }
        }
        if self.fifo.len() > 2 * self.slots.len() + 1024 {
            let slots = &self.slots;
            self.fifo.retain(|id| slots.contains_key(id));
        }
    }

    fn remove(&mut self, id: u64) -> bool {
        let Some(slot) = self.slots.remove(&id) else { return false };
        self.bytes -= slot.size;
        for t in &slot.entry.tags {
            if let Some(set) = self.by_tag.get_mut(t) {
                set.remove(&id);
                if set.is_empty() {
                    self.by_tag.remove(t);
                }
            }
        }
        if let Some(ids) = self.by_key.get_mut(&slot.key) {
            ids.retain(|&x| x != id);
            if ids.is_empty() {
                self.by_key.remove(&slot.key);
            }
        }
        true
    }

    fn remove_all(&mut self, mut ids: Vec<u64>) -> usize {
        self.ban_gen += 1;
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter().filter(|&id| self.remove(id)).count()
    }

    /// Every variant of one key (PURGE).
    pub fn purge_key(&mut self, key: &str) -> usize {
        self.pass.remove(key);
        let ids = self.by_key.get(key).cloned().unwrap_or_default();
        self.remove_all(ids)
    }

    /// Exact tags (`Surrogate-Key`).
    pub fn ban_tags(&mut self, tags: &[&str]) -> usize {
        let ids = tags.iter().filter_map(|t| self.by_tag.get(*t)).flat_map(|s| s.iter().copied()).collect();
        self.remove_all(ids)
    }

    /// Varnish's `obj.http.Cache-Tags ~ <regex>`: the regex runs against the
    /// space-joined tag line. When `per_tag` (the pattern cannot match across
    /// a space, see `ban::per_tag`), it runs once per distinct tag through the
    /// index instead of once per object.
    pub fn ban_tag_regex(&mut self, re: &Regex, per_tag: bool) -> usize {
        let ids = if per_tag {
            self.by_tag.iter().filter(|(t, _)| re.is_match(t)).flat_map(|(_, s)| s.iter().copied()).collect()
        } else {
            self.slots.iter().filter(|(_, s)| re.is_match(&s.entry.tag_line)).map(|(id, _)| *id).collect()
        };
        self.remove_all(ids)
    }

    /// Any predicate over entries (URL bans).
    pub fn ban_where(&mut self, f: impl Fn(&Entry) -> bool) -> usize {
        let ids = self.slots.iter().filter(|(_, s)| f(&s.entry)).map(|(id, _)| *id).collect();
        self.remove_all(ids)
    }

    pub fn set_pass(&mut self, key: &Arc<str>, until: Instant) {
        if self.pass.len() > 100_000 {
            let now = Instant::now();
            self.pass.retain(|_, t| *t > now);
        }
        self.pass.insert(key.clone(), until);
    }

    pub fn is_pass(&mut self, key: &str, now: Instant) -> bool {
        match self.pass.get(key) {
            Some(t) if *t > now => true,
            Some(_) => {
                self.pass.remove(key);
                false
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(url: &str, tags: &[&str], body: usize, ttl: u64) -> Arc<Entry> {
        Arc::new(Entry {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Bytes::from(vec![b'x'; body]),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            tag_line: tags.join(" "),
            host: "h".into(),
            url: url.into(),
            vary: Vec::new(),
            stored: Instant::now(),
            life: Lifetime { ttl: Duration::from_secs(ttl), grace: Duration::ZERO, stale_if_error: Duration::ZERO },
        })
    }

    #[test]
    fn tag_index_and_bans() {
        let mut s = Store::new(1 << 20);
        let k = |s: &str| Arc::<str>::from(s);
        s.insert(&k("a"), entry("/a", &["node:1", "node_list"], 10, 60));
        s.insert(&k("b"), entry("/b", &["node:10"], 10, 60));
        s.insert(&k("c"), entry("/c", &["user:1"], 10, 60));
        let anchored = Regex::new(r"(^|\s)node:1(\s|$)").unwrap();
        assert_eq!(s.ban_tag_regex(&anchored, true), 1);
        let loose = Regex::new("node:1").unwrap();
        assert_eq!(s.ban_tag_regex(&loose, false), 1); // Varnish substring semantics: node:10
        assert_eq!(s.ban_tags(&["user:1", "missing"]), 1);
        assert!(s.is_empty() && s.by_tag.is_empty() && s.by_key.is_empty() && s.bytes() == 0);
    }

    #[test]
    fn fifo_eviction_and_replacement() {
        let mut s = Store::new(3000);
        let k = |s: &str| Arc::<str>::from(s);
        s.insert(&k("a"), entry("/a", &[], 1000, 60));
        s.insert(&k("a"), entry("/a", &[], 1000, 60)); // replaces
        assert_eq!(s.len(), 1);
        s.insert(&k("b"), entry("/b", &[], 1000, 60));
        s.insert(&k("c"), entry("/c", &[], 1000, 60)); // evicts a
        assert!(matches!(s.lookup("a", &HeaderMap::new(), Instant::now()), Lookup::Miss(None)));
        assert!(matches!(s.lookup("c", &HeaderMap::new(), Instant::now()), Lookup::Fresh(_)));
        assert!(s.bytes() <= 3000);
    }
}
