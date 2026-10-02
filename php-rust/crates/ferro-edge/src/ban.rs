//! BAN/PURGE requests, in the forms Drupal's Purge purgers send.
//!
//! - `BAN` with `Purge-Cache-Tags: <regex>` or `Cache-Tags: <regex>`: the
//!   Varnish VCL convention (`ban("obj.http.Cache-Tags ~ " + req.http.Cache-Tags)`),
//!   so `node:1` also bans `node:10` exactly as Varnish does; anchor the tags
//!   (`(^|\s)node:1(\s|$)`) for exact matches.
//! - `BAN` with `Surrogate-Key: a b c`: exact tags, space-separated.
//! - `BAN` with `X-Url: <regex>` (or `Purge-Url`), optionally `X-Host: <regex>`.
//! - `PURGE /path`: every variant of that one URL.

use regex::Regex;

/// True when a tag regex cannot match across the space between two tags, so
/// testing it against each tag alone equals testing the joined line: an
/// alternation of tag-like literals, each optionally wrapped in
/// `(^|\s)` / `(\s|$)`. Within a literal, `.` is the regex wildcard; a literal
/// whose `.` would have to match the separating space is the one case where
/// the per-tag path differs from Varnish (it bans less).
pub fn per_tag(pattern: &str) -> bool {
    use std::sync::OnceLock;
    static SIMPLE: OnceLock<Regex> = OnceLock::new();
    let re = SIMPLE.get_or_init(|| {
        let branch = r"(?:\(\^\|\\s\))?[A-Za-z0-9_:./\-]+(?:\(\\s\|\$\))?";
        Regex::new(&format!(r"^{branch}(?:\|{branch})*$")).unwrap()
    });
    re.is_match(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifier() {
        assert!(per_tag("node:1"));
        assert!(per_tag("node:1|node_list|config:system.site"));
        assert!(per_tag(r"(^|\s)node:1(\s|$)|(^|\s)node_list(\s|$)"));
        assert!(!per_tag(".*"));
        assert!(!per_tag("node:1 node:2"));
        assert!(!per_tag(r"node:\d+"));
    }
}
