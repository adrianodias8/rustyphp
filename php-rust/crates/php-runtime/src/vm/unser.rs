//! Single-pass `unserialize()`: a port of ext/standard/var_unserializer.re
//! that builds the `Zval` straight from the bytes, the way PHP does.
//!
//! No validation pass first (it read every byte twice and was ~28 % of
//! `unserialize()` on Drupal's cache rows), so the failure behaviour is PHP's
//! own: classes met before the error were autoloaded, the error offset is
//! where re2c's cursor stopped, the objects completed before it still get
//! their delayed `__wakeup` / `__unserialize` (var_destroy), and an object
//! whose fields failed with one of those pending never runs `__destruct`.
//! Payloads holding an `R:` alias or a `C:` record keep the two-phase path
//! ([`crate::unserialize::parse`] + `Vm::vm_ser_to_zval`): an alias turns an
//! already built slot into a reference, which a value moved into its
//! container cannot become.

use super::*;
use crate::unserialize::Parser;

/// An unserialize failure PHP reports as "Error at offset N" (with `false`):
/// carried as an error to unwind the builders, turned into the warning by
/// `ho_unserialize`.
pub(super) const UNSER_FAIL: &str = "\0unserialize-fail:";

fn unser_fail(offset: usize) -> PhpError {
    PhpError::Error(format!("{UNSER_FAIL}{offset}"))
}

/// HT_MAX_SIZE on 64-bit: the most elements an array may declare.
const HT_MAX_SIZE: i64 = 0x4000_0000;
/// `unserialize_max_depth`'s default.
const MAX_DEPTH: usize = 4096;

/// A `__wakeup` / `__unserialize` call PHP delays to the end of the call
/// (var_destroy), queued in the order the objects completed.
enum Delayed {
    Wakeup(Zval),
    Unserialize(Zval, PhpArray),
}

/// One call's state: the pre-order slot counter `r:` resolves against, the
/// objects by slot, the delayed calls and the nesting depth.
#[derive(Default)]
struct Pass {
    count: i64,
    objs: HashMap<i64, Zval>,
    delayed: Vec<Delayed>,
    depth: usize,
}

/// One value: `Some` when built, `None` on a parse failure (the cursor then
/// holds PHP's error offset); `Err` is a thrown exception.
type Step = Result<Option<Zval>, PhpError>;

/// The byte at `i`, or the NUL terminator PHP's buffer carries past the end.
fn at(b: &[u8], i: usize) -> u8 {
    b.get(i).copied().unwrap_or(0)
}

/// End of `[0-9]+` at `i` (`i` itself when there is no digit).
fn digits_end(b: &[u8], mut i: usize) -> usize {
    while at(b, i).is_ascii_digit() {
        i += 1;
    }
    i
}

/// parse_uiv: the digits at `i`, wrapping like `size_t` (re2c already
/// matched at least one).
fn parse_uiv(b: &[u8], i: usize) -> usize {
    b[i..digits_end(b, i)].iter().fold(0usize, |a, &c| a.wrapping_mul(10).wrapping_add((c - b'0') as usize))
}

/// The end of `"X:" uiv <term>` at `start`, or `None` when re2c's pattern
/// does not match (the `any` rule: failure at `start`).
fn uiv_rule(b: &[u8], start: usize, term: &[u8]) -> Option<usize> {
    let e = digits_end(b, start + 2);
    (at(b, start + 1) == b':' && e > start + 2 && b.get(e..e + term.len()) == Some(term)).then_some(e + term.len())
}

/// `d:` (iv | nv | nvexp) `;` — the end of the number (the `;`), if it matches.
fn double_rule(b: &[u8], i: usize) -> Option<usize> {
    let i = i + usize::from(matches!(at(b, i), b'+' | b'-'));
    let int_end = digits_end(b, i);
    let (mut e, ok) = if at(b, int_end) == b'.' {
        let frac_end = digits_end(b, int_end + 1);
        (frac_end, int_end > i || frac_end > int_end + 1)
    } else {
        (int_end, int_end > i)
    };
    if !ok {
        return None;
    }
    if matches!(at(b, e), b'e' | b'E') {
        let x = e + 1 + usize::from(matches!(at(b, e + 1), b'+' | b'-'));
        let x_end = digits_end(b, x);
        if x_end == x {
            return None;
        }
        e = x_end;
    }
    (at(b, e) == b';').then_some(e)
}

/// zend_is_valid_class_name.
fn valid_class_name(name: &[u8]) -> bool {
    name.iter().all(|&c| c.is_ascii_alphanumeric() || c == b'_' || c == b'\\' || c >= 0x80)
}

impl<'m> super::Vm<'m> {
    /// `unserialize($s)` of a payload with no `R:`/`C:`: the value, or
    /// `false` and PHP's warning; the delayed calls run last, as in PHP.
    pub(super) fn unserialize_single(&mut self, b: &[u8]) -> Result<Zval, PhpError> {
        let mut p = Parser::new(b);
        let mut pass = Pass::default();
        let out = match self.us_value(&mut p, Some(&mut pass)) {
            Ok(Some(v)) => {
                if p.i < b.len() {
                    self.us_warn(Diag::Warning(format!(
                        "unserialize(): Extra data starting at offset {} of {} bytes",
                        p.i,
                        b.len()
                    )))?;
                }
                v
            }
            Ok(None) => {
                self.us_warn(Diag::Warning(format!("unserialize(): Error at offset {} of {} bytes", p.i, b.len())))?;
                Zval::Bool(false)
            }
            Err(e) => {
                // With an exception pending none of the delayed calls runs,
                // and none of those objects destructs (var_destroy).
                for d in &pass.delayed {
                    let (Delayed::Wakeup(o) | Delayed::Unserialize(o, _)) = d;
                    self.us_skip_dtor(o);
                }
                return Err(e);
            }
        };
        let failed = matches!(out, Zval::Bool(false));
        // PHP frees a failed result (destructing what nothing else holds)
        // before var_destroy's delayed calls, and each of theirs after it.
        drop(std::mem::take(&mut pass.objs));
        self.us_destruct_now(failed)?;
        self.us_run_delayed(std::mem::take(&mut pass.delayed), failed)?;
        Ok(out)
    }

    /// Raise a diagnostic now, at unserialize()'s line, as PHP does: one
    /// queued behind another would reach a user error handler's own flush
    /// and render bypassing it.
    fn us_warn(&mut self, d: Diag) -> Result<(), PhpError> {
        self.diags.push(d);
        self.us_warn_flush()
    }

    fn us_warn_flush(&mut self) -> Result<(), PhpError> {
        let line = self.cur_line(self.frames.len() - 1);
        self.flush_diags(line)
    }

    /// Run the destructors a failed unserialize released, now, as PHP's
    /// refcounting does (drop mode; the classic engine sweeps at its next op).
    fn us_destruct_now(&mut self, failed: bool) -> Result<(), PhpError> {
        if failed && self.gc_drop {
            self.gc_drop_sweep(None)?;
        }
        Ok(())
    }

    /// var_destroy's delayed calls: in order until one throws; that object
    /// and every later one are then marked destructed and not called.
    fn us_run_delayed(&mut self, delayed: Vec<Delayed>, released: bool) -> Result<(), PhpError> {
        let mut failed = None;
        for d in delayed {
            let (obj, r) = match d {
                Delayed::Wakeup(o) if failed.is_none() => (o.clone(), self.call_method_sync(o, b"__wakeup", Vec::new())),
                Delayed::Unserialize(o, data) if failed.is_none() => {
                    let r = self.call_method_sync(o.clone(), b"__unserialize", vec![Zval::Array(Rc::new(data))]);
                    (o, r)
                }
                Delayed::Wakeup(o) | Delayed::Unserialize(o, _) => {
                    self.us_skip_dtor(&o);
                    continue;
                }
            };
            if let Err(e) = r {
                self.us_skip_dtor(&obj);
                failed = Some(e);
            }
            // Each entry is released after its call: what only it held
            // destructs before the next call.
            drop(obj);
            self.us_destruct_now(released)?;
        }
        failed.map_or(Ok(()), Err)
    }

    /// GC_ADD_FLAGS(obj, IS_OBJ_DESTRUCTOR_CALLED).
    fn us_skip_dtor(&mut self, obj: &Zval) {
        if let Zval::Object(rc) = obj {
            let b = rc.borrow();
            self.destructed.insert(b.id);
            b.gc.set_destructed(true);
        }
    }

    /// php_var_unserialize_internal: one value at the cursor. `pass` is
    /// `None` for an array key or property name (PHP's NULL var_hash), where
    /// containers, objects and `r:` fail.
    fn us_value<'b>(&mut self, p: &mut Parser<'b>, pass: Option<&mut Pass>) -> Step {
        let b = p.b;
        let start = p.i;
        if start >= b.len() {
            return Ok(None);
        }
        let mut pass = pass;
        let slot = match pass.as_deref_mut() {
            Some(ps) => {
                ps.count += 1;
                ps.count
            }
            None => 0,
        };
        let c = b[start];
        let lit = |s: &[u8]| b.get(start..start + s.len()) == Some(s);
        Ok(Some(match c {
            b'N' if lit(b"N;") => {
                p.i = start + 2;
                Zval::Null
            }
            b'b' if lit(b"b:0;") || lit(b"b:1;") => {
                p.i = start + 4;
                Zval::Bool(b[start + 2] == b'1')
            }
            b'i' if at(b, start + 1) == b':' => {
                let d = start + 2 + usize::from(matches!(at(b, start + 2), b'+' | b'-'));
                let e = digits_end(b, d);
                if e == d || at(b, e) != b';' {
                    return Ok(None);
                }
                p.i = e + 1;
                Zval::Long(self.us_parse_iv(b, start + 2)?.0)
            }
            b'd' if at(b, start + 1) == b':' => {
                let body = &b[start + 2..];
                let special = [&b"NAN;"[..], b"INF;", b"-INF;"].into_iter().find(|s| body.starts_with(s));
                let e = match special {
                    Some(s) => start + 2 + s.len() - 1,
                    None => match double_rule(b, start + 2) {
                        Some(e) => e,
                        None => return Ok(None),
                    },
                };
                p.i = e + 1;
                Zval::Double(crate::unserialize::parse_double(&b[start + 2..e]).unwrap_or(0.0))
            }
            b's' | b'S' => {
                let Some(q) = uiv_rule(b, start, b":\"") else { return Ok(None) };
                let len = parse_uiv(b, start + 2);
                if b.len() - q < len {
                    p.i = start + 2;
                    return Ok(None);
                }
                // `s:` borrows the input (one allocation: the string itself).
                let (bytes, end) = if c == b's' {
                    (std::borrow::Cow::Borrowed(&b[q..q + len]), q + len)
                } else {
                    match unescape_s(b, q, len) {
                        Some((v, e)) => (std::borrow::Cow::Owned(v), e),
                        None => return Ok(None),
                    }
                };
                if at(b, end) != b'"' {
                    p.i = end;
                    return Ok(None);
                }
                if at(b, end + 1) != b';' {
                    p.i = end + 1;
                    return Ok(None);
                }
                p.i = end + 2;
                if c == b'S' {
                    self.us_warn(Diag::Deprecated("unserialize(): Unserializing the 'S' format is deprecated".into()))?;
                }
                Zval::Str(PhpStr::new(&*bytes))
            }
            // An `R:` here is a key (values with one take the two-phase
            // path): its rule consumes it, then refuses without a var_hash.
            b'R' => {
                if let Some(e) = uiv_rule(b, start, b";") {
                    p.i = e;
                }
                return Ok(None);
            }
            b'r' => {
                let Some(e) = uiv_rule(b, start, b";") else { return Ok(None) };
                p.i = e;
                let Some(ps) = pass else { return Ok(None) };
                let n = parse_uiv(b, start + 2) as i64;
                match ps.objs.get(&n) {
                    Some(o) if n != slot => o.clone(),
                    _ => return Ok(None),
                }
            }
            b'a' => {
                let Some(e) = uiv_rule(b, start, b":{") else { return Ok(None) };
                p.i = e;
                let Some(ps) = pass else { return Ok(None) };
                let n = self.us_parse_iv(b, start + 2)?.0;
                if n >= HT_MAX_SIZE || n as usize > (b.len() - e) / 2 {
                    return Ok(None);
                }
                // An empty array skips the nested-data walk (and its depth check).
                let arr = match n {
                    0 => PhpArray::new(),
                    _ => match self.us_items(p, ps, n as usize)? {
                        Some(arr) => arr,
                        None => return Ok(None),
                    },
                };
                if at(b, p.i) != b'}' {
                    return Ok(None);
                }
                p.i += 1;
                Zval::Array(Rc::new(arr))
            }
            b'O' => {
                let Some(q) = uiv_rule(b, start, b":\"") else { return Ok(None) };
                let Some(ps) = pass else { return Ok(None) };
                p.i = q;
                return self.us_object(p, ps, start, slot);
            }
            b'E' => {
                let Some(q) = uiv_rule(b, start, b":\"") else { return Ok(None) };
                let Some(ps) = pass else { return Ok(None) };
                let len = parse_uiv(b, start + 2);
                if b.len() - q < len || len == 0 {
                    p.i = start + 2;
                    return Ok(None);
                }
                let end = q + len;
                if at(b, end) != b'"' {
                    p.i = end;
                    return Ok(None);
                }
                if at(b, end + 1) != b';' {
                    p.i = end + 1;
                    return Ok(None);
                }
                let r = self.unser_enum(&b[q..end], start, end + 2);
                self.us_warn_flush()?;
                match r {
                    Ok(v) => {
                        p.i = end + 2;
                        ps.objs.insert(slot, v.clone());
                        v
                    }
                    Err(PhpError::Error(m)) if m.starts_with(UNSER_FAIL) => {
                        p.i = m[UNSER_FAIL.len()..].parse().unwrap_or(start);
                        return Ok(None);
                    }
                    Err(e) => return Err(e),
                }
            }
            b'}' => {
                self.us_warn(Diag::Warning("unserialize(): Unexpected end of serialized data".into()))?;
                return Ok(None);
            }
            _ => return Ok(None),
        }))
    }

    /// parse_iv2 at `i`: the value (clamped, with PHP's warning, when out of
    /// range) and the end of the digits.
    fn us_parse_iv(&mut self, b: &[u8], mut i: usize) -> Result<(i64, usize), PhpError> {
        let neg = at(b, i) == b'-';
        i += usize::from(matches!(at(b, i), b'+' | b'-'));
        while at(b, i) == b'0' {
            i += 1;
        }
        let s = i;
        let mut v = 0u64;
        while at(b, i).is_ascii_digit() {
            v = v.wrapping_mul(10).wrapping_add((b[i] - b'0') as u64);
            i += 1;
        }
        if i - s > 19 || v > i64::MAX as u64 + u64::from(neg) {
            self.us_warn(Diag::Warning("unserialize(): Numerical result out of range".into()))?;
            return Ok((if neg { i64::MIN } else { i64::MAX }, i));
        }
        Ok(((if neg { (v as i64).wrapping_neg() } else { v as i64 }), i))
    }

    /// A key / property name (PHP parses it with a NULL var_hash): an int or
    /// the string's bytes; anything else fails with the cursor where that
    /// token's rule left it.
    fn us_key<'b>(&mut self, p: &mut Parser<'b>) -> Result<Option<Result<i64, std::borrow::Cow<'b, [u8]>>>, PhpError> {
        let b = p.b;
        let s = p.i;
        // The hot shape inline: `s:<len>:"<bytes>";` borrowed from the input.
        if at(b, s) == b's' {
            if let Some(q) = uiv_rule(b, s, b":\"") {
                let len = parse_uiv(b, s + 2);
                if b.len() - q >= len && at(b, q + len) == b'"' && at(b, q + len + 1) == b';' {
                    p.i = q + len + 2;
                    return Ok(Some(Err(std::borrow::Cow::Borrowed(&b[q..q + len]))));
                }
            }
        }
        Ok(match self.us_value(p, None)? {
            Some(Zval::Long(n)) => Some(Ok(n)),
            Some(Zval::Str(st)) => Some(Err(std::borrow::Cow::Owned(st.as_bytes().to_vec()))),
            _ => None,
        })
    }

    /// process_nested_array_data: `n` key/value pairs into a fresh array.
    fn us_items<'b>(&mut self, p: &mut Parser<'b>, ps: &mut Pass, n: usize) -> Result<Option<PhpArray>, PhpError> {
        if ps.depth >= MAX_DEPTH {
            self.us_depth_warning()?;
            return Ok(None);
        }
        ps.depth += 1;
        let r = self.us_items_at_depth(p, ps, n);
        ps.depth -= 1;
        r
    }

    fn us_items_at_depth<'b>(&mut self, p: &mut Parser<'b>, ps: &mut Pass, n: usize) -> Result<Option<PhpArray>, PhpError> {
        let mut arr = PhpArray::new();
        for _ in 0..n {
            let key = match self.us_key(p)? {
                Some(Ok(i)) => Key::Int(i),
                Some(Err(s)) => Key::from_bytes(&s),
                None => return Ok(None),
            };
            let Some(v) = self.us_value(p, Some(ps))? else { return Ok(None) };
            arr.insert(key, v);
        }
        Ok(Some(arr))
    }

    fn us_depth_warning(&mut self) -> Result<(), PhpError> {
        self.us_warn(Diag::Warning(format!(
            "unserialize(): Maximum depth of {MAX_DEPTH} exceeded. The depth limit can be changed using the max_depth unserialize() option or the unserialize_max_depth ini setting"
        )))
    }

    /// The `object ":" uiv ":" ["]` rule, the cursor past the opening quote.
    fn us_object<'b>(&mut self, p: &mut Parser<'b>, ps: &mut Pass, start: usize, slot: i64) -> Step {
        let b = p.b;
        let q = p.i;
        let len = parse_uiv(b, start + 2);
        if b.len() - q < len || len == 0 {
            p.i = start + 2;
            return Ok(None);
        }
        let end = q + len;
        if at(b, end) != b'"' {
            p.i = end;
            return Ok(None);
        }
        if at(b, end + 1) != b':' {
            p.i = end + 1;
            return Ok(None);
        }
        p.i = start;
        let class = &b[q..end];
        if class[0] == 0 || class[0] == b'\\' {
            return Ok(None);
        }
        let lower = class.to_ascii_lowercase();
        let mut tr = None;
        if !self.class_index.contains_key(lower.as_slice()) {
            tr = self.us_trait_named(&lower);
            if tr.is_none() {
                if !valid_class_name(class) {
                    return Ok(None);
                }
                self.try_autoload(class, &lower, None)?;
            }
        }
        let cid = self.class_index.get(lower.as_slice()).copied();
        p.i = end;
        if matches!(lower.as_slice(), b"pdo" | b"pdostatement" | b"pdorow") {
            let msg = format!("Unserialization of '{}' is not allowed", String::from_utf8_lossy(class));
            if let Some(ecid) = self.class_index.get(&b"exception"[..]).copied() {
                return Err(PhpError::Thrown(self.synthesize_throwable(ecid, &msg)?));
            }
        }
        if end + 2 >= b.len() {
            self.us_warn(Diag::Warning("Bad unserialize data".into()))?;
            return Ok(None);
        }
        let (n, e) = self.us_parse_iv(b, end + 2)?;
        p.i = e;
        if n < 0 || n as usize > (b.len() - end) / 2 {
            return Ok(None);
        }
        if at(b, e) != b':' {
            return Ok(None);
        }
        if at(b, e + 1) != b'{' {
            p.i = e + 1;
            return Ok(None);
        }
        p.i = e + 2;
        let n = n as usize;
        let has = |vm: &Self, m: &'static [u8]| cid.is_some_and(|c| vm.resolve_method_static(c, m).is_some());
        let has_unserialize = has(self, b"__unserialize");
        if let Some(c) = cid.filter(|_| !has_unserialize) {
            if self.class_index.get(&b"serializable"[..]).is_some_and(|&s| self.instance_of(c, s)) {
                let name = String::from_utf8_lossy(&self.classes[c].name).into_owned();
                self.us_warn(Diag::Warning(format!("Erroneous data format for unserializing '{name}'")))?;
                return Ok(None);
            }
        }
        self.unser_reject_enum(cid)?;
        if let Some(t) = tr {
            return Err(PhpError::Error(format!("Cannot instantiate trait {}", String::from_utf8_lossy(&t))));
        }
        if has_unserialize {
            let obj = self.vm_make_unserialized_object(class, Vec::new());
            ps.objs.insert(slot, obj.clone());
            // A failure (or exception) in the data: the object never destructs.
            let data = match self.us_items(p, ps, n) {
                Ok(Some(data)) => data,
                r => {
                    self.us_skip_dtor(&obj);
                    return r.map(|_| None);
                }
            };
            ps.delayed.push(Delayed::Unserialize(obj.clone(), data));
            return Ok(self.us_close(p).then_some(obj));
        }
        let obj = self.vm_unserialized_shell(class);
        ps.objs.insert(slot, obj.clone());
        let has_wakeup = has(self, b"__wakeup");
        let mut fields: Vec<(Vec<u8>, Zval)> = Vec::with_capacity(n);
        let r = if ps.depth >= MAX_DEPTH {
            self.us_depth_warning()?;
            Ok(false)
        } else {
            ps.depth += 1;
            let r = self.us_fields(p, ps, n, &mut fields);
            ps.depth -= 1;
            r
        };
        // The fields read so far stay on the object, as in PHP.
        self.vm_apply_unserialized_fields(&obj, fields);
        if !matches!(r, Ok(true)) {
            if has_wakeup {
                self.us_skip_dtor(&obj);
            }
            return r.map(|_| None);
        }
        if has_wakeup {
            ps.delayed.push(Delayed::Wakeup(obj.clone()));
        }
        Ok(self.us_close(p).then_some(obj))
    }

    /// process_nested_object_data's parse half: `n` name/value pairs (an int
    /// name kept in its decimal form), `false` on a parse failure.
    fn us_fields<'b>(&mut self, p: &mut Parser<'b>, ps: &mut Pass, n: usize, out: &mut Vec<(Vec<u8>, Zval)>) -> Result<bool, PhpError> {
        for _ in 0..n {
            let name = match self.us_key(p)? {
                Some(Ok(i)) => i.to_string().into_bytes(),
                Some(Err(s)) => s.into_owned(),
                None => return Ok(false),
            };
            let Some(v) = self.us_value(p, Some(ps))? else { return Ok(false) };
            out.push((name, v));
        }
        Ok(true)
    }

    /// finish_nested_data: the closing `}`.
    fn us_close(&mut self, p: &mut Parser<'_>) -> bool {
        let ok = at(p.b, p.i) == b'}';
        p.i += usize::from(ok);
        ok
    }

    /// A builder's positioned failure (`unser::UNSER_FAIL`, e.g. an unknown
    /// enum) becomes PHP's "Error at offset N of M bytes" warning and `false`.
    pub(super) fn unser_offset_fail(&mut self, built: Result<Zval, PhpError>, nbytes: usize) -> Result<Zval, PhpError> {
        match built {
            Err(PhpError::Error(m)) if m.starts_with(UNSER_FAIL) => {
                let off = &m[UNSER_FAIL.len()..];
                self.diags.push(Diag::Warning(format!("unserialize(): Error at offset {off} of {nbytes} bytes")));
                Ok(Zval::Bool(false))
            }
            r => r,
        }
    }
    /// `E:<len>:"<Enum>:<Case>";` — the case singleton, after var_unserializer.re's
    /// `E:` rule: the enum is looked up with autoload; a failure warns like PHP
    /// and aborts the whole unserialize at the offset PHP reports (`start`, the
    /// `E`, before the class checks; `end`, past `";`, after them).
    pub(super) fn unser_enum(&mut self, raw: &[u8], start: usize, end: usize) -> Result<Zval, PhpError> {
        let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        if raw.is_empty() {
            return Err(unser_fail(start + 2));
        }
        let Some(colon) = raw.iter().position(|&b| b == b':') else {
            let msg = format!("unserialize(): Invalid enum name '{}' (missing colon)", lossy(raw));
            self.diags.push(Diag::Warning(msg));
            return Err(unser_fail(start));
        };
        let (class, case) = (&raw[..colon], &raw[colon + 1..]);
        // zend_is_valid_class_name: label bytes and `\` only, no warning.
        if !class.iter().all(|&c| c.is_ascii_alphanumeric() || c == b'_' || c == b'\\' || c >= 0x80) {
            return Err(unser_fail(start));
        }
        let lower = class.to_ascii_lowercase();
        // zend_lookup_class: an empty name is not autoloaded.
        if !class.is_empty() && !self.class_index.contains_key(lower.as_slice()) {
            self.try_autoload(class, &lower, None)?;
        }
        let Some(cid) = self.class_index.get(lower.as_slice()).copied() else {
            self.diags.push(Diag::Warning(format!("unserialize(): Class '{}' not found", lossy(class))));
            return Err(unser_fail(start));
        };
        if !matches!(self.classes[cid].instantiable, Instantiable::Enum) {
            self.diags.push(Diag::Warning(format!("unserialize(): Class '{}' is not an enum", lossy(class))));
            return Err(unser_fail(start));
        }
        if let Some(i) = self.enum_case_idx(cid, case) {
            return Ok(Zval::Object(self.enum_case(cid, i as u32)));
        }
        let msg = if self.classes[cid].consts.iter().any(|k| k.name.as_ref() == case) {
            format!("unserialize(): {}::{} is not an enum case", lossy(class), lossy(case))
        } else {
            format!("unserialize(): Undefined constant {}::{}", lossy(class), lossy(case))
        };
        self.diags.push(Diag::Warning(msg));
        Err(unser_fail(end))
    }

    /// `O:` of a class object_init_ex refuses: abstract, interface or enum.
    pub(super) fn unser_reject_enum(&self, cid: Option<ClassId>) -> Result<(), PhpError> {
        let Some(cid) = cid else { return Ok(()) };
        let what = match self.classes[cid].instantiable {
            Instantiable::Yes => return Ok(()),
            Instantiable::Abstract => "abstract class",
            Instantiable::Interface => "interface",
            Instantiable::Enum => "enum",
        };
        Err(PhpError::Error(format!("Cannot instantiate {what} {}", String::from_utf8_lossy(&self.classes[cid].name))))
    }

    /// The declared name of the trait `lower`, if one is declared: Zend keeps
    /// traits in the class table, so `O:` of one is found (no autoload) and
    /// refused like an abstract class.
    fn us_trait_named(&self, lower: &[u8]) -> Option<Vec<u8>> {
        self.seed_traits.iter().find(|(_, t)| t.name.eq_ignore_ascii_case(lower)).map(|(_, t)| t.name.to_vec())
    }
}

/// unserialize_str (the deprecated `S:` format): `len` characters from `q`,
/// `\xx` a hex-escaped byte; the bytes and the cursor after them, `None`
/// past the end or on a bad escape.
fn unescape_s(b: &[u8], mut q: usize, len: usize) -> Option<(Vec<u8>, usize)> {
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        let c = *b.get(q)?;
        if c != b'\\' {
            out.push(c);
        } else {
            let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
            out.push(hex(at(b, q + 1))? << 4 | hex(at(b, q + 2))?);
            q += 2;
        }
        q += 1;
    }
    Some((out, q))
}

