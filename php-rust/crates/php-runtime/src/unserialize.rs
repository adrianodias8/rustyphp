//! Parser for PHP's serialization format (step 50b), the inverse of the
//! `serialize()` builtin. Pure: it produces an intermediate [`Ser`] tree from
//! bytes; the evaluator turns that into a `Zval` (objects need the class table).
//!
//! Grammar handled (the subset `serialize()` emits):
//!   N;  b:[01];  i:<int>;  d:<float>;  s:<len>:"<bytes>";
//!   a:<n>:{<k><v>...}      O:<len>:"<class>":<n>:{<propname><v>...}
//!
//! Shared-reference markers: `r:<n>;` (repeat of the object numbered `n`) and
//! `R:<n>;` (alias of the value slot numbered `n`) parse to [`Ser::ObjRef`] /
//! [`Ser::AliasRef`]; the evaluator resolves them against its slot registry
//! (Zend numbers every serialized value pre-order; `R:` emissions consume no
//! number, everything else — including `r:` — does).

/// An intermediate node decoded from a serialized string.
#[derive(Debug, Clone, PartialEq)]
pub enum Ser {
    Null,
    Bool(bool),
    Long(i64),
    Double(f64),
    Str(Vec<u8>),
    /// Ordered (key, value) pairs; a key is only ever `Long` or `Str`.
    Array(Vec<(Ser, Ser)>),
    /// Class name and ordered (property-name, value) pairs.
    Object(Vec<u8>, Vec<(Vec<u8>, Ser)>),
    /// `C:<len>:"<class>":<len>:{<payload>}` — a legacy `Serializable` record:
    /// class name and the raw opaque payload its `unserialize()` receives.
    CObject(Vec<u8>, Vec<u8>),
    /// `r:<n>;` — the same object as the value numbered `n` (handle copy).
    ObjRef(i64),
    /// `R:<n>;` — a reference aliasing the value slot numbered `n`.
    AliasRef(i64),
    /// `E:<len>:"<Enum>:<Case>";` — the raw name, the offset of the `E` and
    /// the offset past the `";` (the two offsets PHP's errors report).
    Enum(Vec<u8>, usize, usize),
}

/// Parse a complete serialized value. Returns `None` on any malformed input or
/// trailing garbage (PHP's `unserialize()` then yields `false` + a notice).
pub fn parse(bytes: &[u8]) -> Option<Ser> {
    let mut p = Parser { b: bytes, i: 0 };
    let v = p.value()?;
    // PHP tolerates nothing after the top-level value.
    if p.i == p.b.len() {
        Some(v)
    } else {
        None
    }
}

/// Parse one serialized value at the START of `bytes`, returning it together
/// with the byte count consumed. The session `php`/`php_binary` decoders read
/// concatenated `key|<value>` records, so trailing data belongs to the caller
/// (unlike [`parse`], which rejects it).
pub fn parse_prefix(bytes: &[u8]) -> Option<(Ser, usize)> {
    let mut p = Parser { b: bytes, i: 0 };
    let v = p.value()?;
    Some((v, p.i))
}

/// Collect every slot number some `R:<n>;` in the tree aliases, so the
/// evaluator can cell-wrap exactly those slots while building.
pub fn collect_alias_targets(s: &Ser, targets: &mut std::collections::HashSet<i64>) {
    match s {
        Ser::AliasRef(n) => {
            targets.insert(*n);
        }
        Ser::Array(items) => {
            for (_, v) in items {
                collect_alias_targets(v, targets);
            }
        }
        Ser::Object(_, props) => {
            for (_, v) in props {
                collect_alias_targets(v, targets);
            }
        }
        _ => {}
    }
}

/// Check that `bytes` is ONE complete serialized value, without building
/// anything. `Some(true)`: valid and free of `R:` aliases and `C:` records,
/// so the VM may build it in a single pass (`Vm::unserialize_direct`);
/// `Some(false)`: valid but needs the two-phase [`parse`] path; `None`:
/// malformed (same acceptance as [`parse`]).
pub fn validate(bytes: &[u8]) -> Option<bool> {
    let mut p = Parser::new(bytes);
    let mut simple = true;
    p.skip_value(&mut simple)?;
    (p.i == p.b.len()).then_some(simple)
}

/// Cursor over serialized bytes, shared by the tree parser, the validator
/// and the VM's single-pass builder.
pub(crate) struct Parser<'a> {
    pub(crate) b: &'a [u8],
    pub(crate) i: usize,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(b: &'a [u8]) -> Self {
        Parser { b, i: 0 }
    }

    pub(crate) fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    pub(crate) fn eat(&mut self, c: u8) -> Option<()> {
        if self.peek() == Some(c) {
            self.i += 1;
            Some(())
        } else {
            None
        }
    }

    /// Read up to (not including) `delim`, advancing past it.
    pub(crate) fn take_until(&mut self, delim: u8) -> Option<&'a [u8]> {
        let start = self.i;
        let off = self.b.get(start..)?.iter().position(|&c| c == delim)?;
        self.i = start + off + 1;
        Some(&self.b[start..start + off])
    }

    pub(crate) fn int_until(&mut self, delim: u8) -> Option<i64> {
        parse_i64(self.take_until(delim)?)
    }

    /// The `<len>:"<bytes>"` chunk as a slice of the input (no copy).
    pub(crate) fn quoted_slice(&mut self) -> Option<&'a [u8]> {
        let len = self.usize_until(b':')?;
        self.eat(b'"')?;
        let bytes = self.b.get(self.i..self.i.checked_add(len)?)?;
        self.i += len;
        self.eat(b'"')?;
        Some(bytes)
    }

    /// [`Self::value`]'s grammar without building: advances past one value,
    /// clearing `simple` on an `R:` or `C:`.
    fn skip_value(&mut self, simple: &mut bool) -> Option<()> {
        let c = self.peek()?;
        self.i += 1;
        if c == b'N' {
            return self.eat(b';');
        }
        self.eat(b':')?;
        match c {
            b'b' => {
                if !matches!(self.peek()?, b'0' | b'1') {
                    return None;
                }
                self.i += 1;
                self.eat(b';')
            }
            b'i' | b'r' | b'R' => {
                if c == b'R' {
                    *simple = false;
                }
                self.int_until(b';').map(|_| ())
            }
            b'd' => parse_double(self.take_until(b';')?).map(|_| ()),
            b's' => {
                self.quoted_slice()?;
                self.eat(b';')
            }
            b'a' => {
                let n = self.usize_until(b':')?;
                self.eat(b'{')?;
                for _ in 0..n {
                    if !matches!(self.peek()?, b'i' | b's') {
                        return None;
                    }
                    self.skip_value(simple)?;
                    self.skip_value(simple)?;
                }
                self.eat(b'}')
            }
            b'O' => {
                self.quoted_slice()?;
                self.eat(b':')?;
                let n = self.usize_until(b':')?;
                self.eat(b'{')?;
                for _ in 0..n {
                    if !matches!(self.peek()?, b'i' | b's') {
                        return None;
                    }
                    self.skip_value(simple)?;
                    self.skip_value(simple)?;
                }
                self.eat(b'}')
            }
            b'E' => {
                self.quoted_slice()?;
                self.eat(b';')
            }
            b'C' => {
                *simple = false;
                self.quoted_slice()?;
                self.eat(b':')?;
                let len = self.usize_until(b':')?;
                self.eat(b'{')?;
                self.b.get(self.i..self.i.checked_add(len)?)?;
                self.i += len;
                self.eat(b'}')
            }
            _ => None,
        }
    }

    pub(crate) fn value(&mut self) -> Option<Ser> {
        match self.peek()? {
            b'N' => {
                self.i += 1;
                self.eat(b';')?;
                Some(Ser::Null)
            }
            b'b' => {
                self.i += 1;
                self.eat(b':')?;
                let v = match self.peek()? {
                    b'0' => false,
                    b'1' => true,
                    _ => return None,
                };
                self.i += 1;
                self.eat(b';')?;
                Some(Ser::Bool(v))
            }
            b'i' => {
                self.i += 1;
                self.eat(b':')?;
                Some(Ser::Long(self.int_until(b';')?))
            }
            b'r' => {
                self.i += 1;
                self.eat(b':')?;
                Some(Ser::ObjRef(self.int_until(b';')?))
            }
            b'E' => {
                let start = self.i;
                self.i += 1;
                self.eat(b':')?;
                let raw = self.quoted_slice()?.to_vec();
                self.eat(b';')?;
                Some(Ser::Enum(raw, start, self.i))
            }
            b'R' => {
                self.i += 1;
                self.eat(b':')?;
                Some(Ser::AliasRef(self.int_until(b';')?))
            }
            b'd' => {
                self.i += 1;
                self.eat(b':')?;
                let s = self.take_until(b';')?;
                Some(Ser::Double(parse_double(s)?))
            }
            b's' => {
                self.i += 1;
                self.eat(b':')?;
                Some(Ser::Str(self.string_body()?))
            }
            b'a' => {
                self.i += 1;
                self.eat(b':')?;
                let n = self.usize_until(b':')?;
                self.eat(b'{')?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    let k = self.value()?;
                    // A key is only valid as int or string.
                    if !matches!(k, Ser::Long(_) | Ser::Str(_)) {
                        return None;
                    }
                    let v = self.value()?;
                    items.push((k, v));
                }
                self.eat(b'}')?;
                Some(Ser::Array(items))
            }
            b'O' => {
                self.i += 1;
                self.eat(b':')?;
                // Class name: `<len>:"<class>"` followed by `:` then the count.
                let class = self.quoted_bytes()?;
                self.eat(b':')?;
                let n = self.usize_until(b':')?;
                self.eat(b'{')?;
                let mut props = Vec::with_capacity(n);
                for _ in 0..n {
                    // Property names are serialized strings; an `__serialize()`
                    // record may carry *int* keys (`i:0;`) — kept as their
                    // decimal form (the array builder re-canonicalizes them).
                    let name = match self.value()? {
                        Ser::Str(s) => s,
                        Ser::Long(i) => i.to_string().into_bytes(),
                        _ => return None,
                    };
                    let v = self.value()?;
                    props.push((name, v));
                }
                self.eat(b'}')?;
                Some(Ser::Object(class, props))
            }
            b'C' => {
                // Legacy Serializable record: the braces wrap `<len>` raw
                // payload bytes, NOT nested serialized values.
                self.i += 1;
                self.eat(b':')?;
                let class = self.quoted_bytes()?;
                self.eat(b':')?;
                let len = self.usize_until(b':')?;
                self.eat(b'{')?;
                let payload = self.b.get(self.i..self.i.checked_add(len)?)?.to_vec();
                self.i += len;
                self.eat(b'}')?;
                Some(Ser::CObject(class, payload))
            }
            _ => None,
        }
    }

    /// Read a `<len>:"<bytes>"` chunk (byte count, then the verbatim bytes),
    /// stopping right after the closing quote. The terminator differs by context:
    /// a string value ends with `;`, but an object's class name is followed by
    /// `:` (the property count), so callers consume the terminator themselves.
    fn quoted_bytes(&mut self) -> Option<Vec<u8>> {
        self.quoted_slice().map(<[u8]>::to_vec)
    }

    /// A string value / array key / property name: `<len>:"<bytes>";`.
    fn string_body(&mut self) -> Option<Vec<u8>> {
        let bytes = self.quoted_bytes()?;
        self.eat(b';')?;
        Some(bytes)
    }

    pub(crate) fn usize_until(&mut self, delim: u8) -> Option<usize> {
        let s = self.take_until(delim)?;
        let digits = s.strip_prefix(b"+").unwrap_or(s);
        if digits.is_empty() {
            return None;
        }
        digits.iter().try_fold(0usize, |acc, &c| {
            c.is_ascii_digit().then_some(())?;
            acc.checked_mul(10)?.checked_add((c - b'0') as usize)
        })
    }
}

/// `str::parse::<i64>` on bytes (optional `+`/`-`, one or more digits,
/// overflow rejected) without the UTF-8 check and `str` round trip: integer
/// parsing was ~30 % of `unserialize()`.
fn parse_i64(s: &[u8]) -> Option<i64> {
    let (neg, digits) = match s.first()? {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty() {
        return None;
    }
    // Accumulate negatively so i64::MIN parses.
    let v = digits.iter().try_fold(0i64, |acc, &c| {
        c.is_ascii_digit().then_some(())?;
        acc.checked_mul(10)?.checked_sub((c - b'0') as i64)
    })?;
    if neg {
        Some(v)
    } else {
        v.checked_neg()
    }
}

/// Parse a serialized float body. PHP emits `INF` / `-INF` / `NAN` for the
/// non-finite cases and a shortest-round-trip decimal otherwise.
pub(crate) fn parse_double(s: &[u8]) -> Option<f64> {
    match s {
        b"INF" => return Some(f64::INFINITY),
        b"-INF" => return Some(f64::NEG_INFINITY),
        b"NAN" => return Some(f64::NAN),
        _ => {}
    }
    std::str::from_utf8(s).ok()?.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars() {
        assert_eq!(parse(b"N;"), Some(Ser::Null));
        assert_eq!(parse(b"b:1;"), Some(Ser::Bool(true)));
        assert_eq!(parse(b"i:42;"), Some(Ser::Long(42)));
        assert_eq!(parse(b"i:-7;"), Some(Ser::Long(-7)));
        assert_eq!(parse(b"d:2.5;"), Some(Ser::Double(2.5)));
        assert_eq!(parse(b"s:5:\"hello\";"), Some(Ser::Str(b"hello".to_vec())));
        // A string length is a byte count: an embedded ';' is data, not a delim.
        assert_eq!(parse(b"s:3:\"a;b\";"), Some(Ser::Str(b"a;b".to_vec())));
        // Embedded quote inside the counted bytes is fine too.
        assert_eq!(parse(b"s:4:\"a\";b\";"), Some(Ser::Str(b"a\";b".to_vec())));
        // A wrong byte count must not parse (closing quote lands mid-data).
        assert_eq!(parse(b"s:2:\"abc\";"), None);
    }

    #[test]
    fn non_finite_floats() {
        assert_eq!(parse(b"d:INF;"), Some(Ser::Double(f64::INFINITY)));
        assert_eq!(parse(b"d:-INF;"), Some(Ser::Double(f64::NEG_INFINITY)));
        assert!(matches!(parse(b"d:NAN;"), Some(Ser::Double(d)) if d.is_nan()));
    }

    #[test]
    fn arrays_and_objects() {
        assert_eq!(
            parse(b"a:2:{i:0;i:9;i:1;N;}"),
            Some(Ser::Array(vec![
                (Ser::Long(0), Ser::Long(9)),
                (Ser::Long(1), Ser::Null),
            ]))
        );
        assert_eq!(
            parse(b"O:8:\"stdClass\":1:{s:1:\"x\";i:5;}"),
            Some(Ser::Object(
                b"stdClass".to_vec(),
                vec![(b"x".to_vec(), Ser::Long(5))]
            ))
        );
    }

    #[test]
    fn malformed_and_trailing_garbage() {
        assert_eq!(parse(b"z"), None);
        assert_eq!(parse(b""), None);
        assert_eq!(parse(b"i:1;XX"), None); // trailing garbage
        assert_eq!(parse(b"b:2;"), None); // bad bool
        assert_eq!(parse(b"a:2:{i:0;i:9;}"), None); // count mismatch
    }

    #[test]
    fn int_parsing_matches_str_parse() {
        for c in [
            "0", "-0", "+0", "42", "-42", "+42", "", "-", "+", "1a", " 1", "1 ", "--1", "+-1",
            "9223372036854775807", "9223372036854775808", "-9223372036854775808",
            "-9223372036854775809", "00012", "\u{663}",
        ] {
            assert_eq!(parse_i64(c.as_bytes()), c.parse::<i64>().ok(), "{c:?}");
            let mut p = Parser::new(format!("{c}:").into_bytes().leak());
            assert_eq!(p.usize_until(b':'), c.parse::<usize>().ok(), "{c:?}");
        }
    }

    #[test]
    fn validate_accepts_exactly_what_parse_accepts() {
        let cases: &[&[u8]] = &[
            b"N;", b"b:0;", b"b:2;", b"i:-3;", b"i:;", b"d:1.5;", b"d:INF;", b"d:x;",
            b"s:3:\"a;b\";", b"s:2:\"abc\";", b"s:9:\"ab\";",
            b"a:0:{}", b"a:1:{i:0;s:1:\"x\";}", b"a:1:{s:1:\"k\";a:1:{i:0;N;}}",
            b"a:1:{d:1.0;i:1;}", b"a:2:{i:0;i:1;}", b"a:1:{i:0;i:1;}X",
            b"O:8:\"stdClass\":1:{s:1:\"x\";i:5;}", b"O:8:\"stdClass\":1:{i:0;r:1;}",
            b"O:8:\"stdClass\":1:{N;i:5;}", b"O:3:\"Foo\":0:{}",
            b"C:3:\"Foo\":3:{abc}", b"C:3:\"Foo\":4:{abc}", b"R:1;", b"r:1;",
            b"a:2:{i:0;a:0:{}i:1;R:2;}", b"", b"z", b"N", b"i:1;;",
        ];
        for c in cases {
            assert_eq!(validate(c).is_some(), parse(c).is_some(), "{}", String::from_utf8_lossy(c));
        }
        assert_eq!(validate(b"a:1:{i:0;O:8:\"stdClass\":0:{}}"), Some(true));
        assert_eq!(validate(b"a:2:{i:0;a:0:{}i:1;R:2;}"), Some(false));
        assert_eq!(validate(b"C:3:\"Foo\":3:{abc}"), Some(false));
    }

    #[test]
    fn reference_markers_parse() {
        assert_eq!(parse(b"r:1;"), Some(Ser::ObjRef(1)));
        assert_eq!(parse(b"R:2;"), Some(Ser::AliasRef(2)));
        // The canonical self-cycle: object slot 1 referenced from its own prop.
        assert_eq!(
            parse(b"O:8:\"stdClass\":1:{s:4:\"self\";r:1;}"),
            Some(Ser::Object(
                b"stdClass".to_vec(),
                vec![(b"self".to_vec(), Ser::ObjRef(1))]
            ))
        );
        let mut targets = std::collections::HashSet::new();
        collect_alias_targets(&parse(b"a:2:{i:0;a:1:{i:0;i:1;}i:1;R:2;}").unwrap(), &mut targets);
        assert_eq!(targets, std::collections::HashSet::from([2]));
    }
}
