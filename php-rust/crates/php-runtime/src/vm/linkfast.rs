//! Per-run caches for `run_linked`'s function registration
//! (HWCOUNTERS_DRUPAL.md §5, include/linking).
//!
//! Every unit module starts with the main image's ~1000 prelude functions
//! (the very same `Rc`s, WP-20). `run_linked` skipped that shared prefix by
//! walking it with pointer compares on every include (2.7M instructions per
//! Drupal request at ~750 includes), and its redeclaration check fell back to
//! a case-insensitive scan of the includer's whole function list, prelude
//! included. Modules are immutable and borrowed for the whole run (`&'m`), so
//! a module's prefix length is computed once and keyed by its address, and
//! the prelude's names are indexed once.

use super::*;

impl<'m> super::Vm<'m> {
    /// How many leading functions of `m` are `prelude_fns` itself (same `Rc`
    /// at the same index).
    pub(super) fn prelude_prefix(&mut self, m: &'m Module) -> usize {
        let k = m as *const Module as usize;
        if let Some(&n) = self.prelude_prefix_memo.get(&k) {
            return n;
        }
        // A unit compiled against this run's prelude carries the count: the
        // compile shared `prelude[i]` for exactly its first `n` indices. The
        // end checks confirm it was this prelude (the vector is one block of
        // the main image); anything else takes the walk.
        let n = m.prelude_shared;
        if n > 0
            && n <= self.prelude_fns.len()
            && Rc::ptr_eq(&m.functions[0], &self.prelude_fns[0])
            && Rc::ptr_eq(&m.functions[n - 1], &self.prelude_fns[n - 1])
            && m.functions.get(n).zip(self.prelude_fns.get(n)).is_none_or(|(f, p)| !Rc::ptr_eq(f, p))
        {
            return n;
        }
        let n = m
            .functions
            .iter()
            .zip(self.prelude_fns.iter())
            .take_while(|(f, p)| Rc::ptr_eq(f, p))
            .count();
        self.prelude_prefix_memo.insert(k, n);
        n
    }

    /// The first function of `m` named `name` (ASCII case-insensitive), as
    /// `m.functions.iter().find(..)` would return it; `prefix` is
    /// [`Self::prelude_prefix`] of `m` and `lower` the lowercased `name`.
    pub(super) fn find_fn_in_module(&mut self, m: &'m Module, prefix: usize, name: &[u8], lower: &[u8]) -> Option<&'m Rc<Func>> {
        if self.prelude_names.is_none() {
            let mut t: HashMap<Vec<u8>, usize> = HashMap::default();
            for (i, f) in self.prelude_fns.iter().enumerate() {
                t.entry(f.name.to_ascii_lowercase()).or_insert(i);
            }
            self.prelude_names = Some(t);
        }
        // The prefix is prelude_fns[..prefix]: its first match is the
        // prelude's first function of that name, if it lies in the prefix.
        let hit = self.prelude_names.as_ref().and_then(|t| t.get(lower)).copied().filter(|&i| i < prefix);
        match hit {
            Some(i) => Some(&m.functions[i]),
            None => m.functions[prefix..].iter().find(|cf| name_eq_ignore_case(&cf.name, name)),
        }
    }
}

/// Whether a unit main can never read or write its own variable slots: every
/// op is a declaration, a constant push, a statement sweep or the return —
/// the body of a class file (`DeclareDeferred; Sweep; PushConst; Ret`). A
/// fresh bridge cell for such a unit stays `Undef` and is never published,
/// so `run_linked` skips allocating it (16k cells per Drupal request).
pub(super) fn unit_main_scope_free(main: &Func) -> bool {
    main.ops.iter().all(|op| {
        matches!(
            op,
            Op::DeclareDeferred { .. }
                | Op::DeclareClass { .. }
                | Op::DeclareFn { .. }
                | Op::DeclareTrait { .. }
                | Op::Sweep { .. }
                | Op::PushConst(_)
                | Op::Pop
                | Op::Ret
                | Op::Nop
        )
    })
}
