//! Single-pass `unserialize()`: builds the `Zval` straight from the bytes.
//!
//! The two-phase path ([`crate::unserialize::parse`] to a `Ser` tree, then
//! `Vm::vm_ser_to_zval`) allocates every string twice (a `Vec<u8>` in the
//! tree, then the `PhpStr`) plus a node per value; a Drupal front page
//! unserializes ~940 KB in 270 calls, 214 of them with objects, and that path
//! was 11 % of the request. Input is validated first
//! ([`crate::unserialize::validate`]), so nothing runs (no autoload, no
//! `__unserialize`/`__wakeup`) for malformed input, as before. Payloads with
//! `R:` aliases or `C:` records (the alias pre-pass and the opaque payload)
//! keep the two-phase path. Slot numbering, `r:` resolution and the object
//! protocol are `vm_ser_build`'s, in the same order.

use super::*;
use crate::unserialize::Parser;

/// An unserialize failure PHP reports as "Error at offset N" (with `false`):
/// carried as an error to unwind the builders, turned into the warning by
/// `ho_unserialize`.
pub(super) const UNSER_FAIL: &str = "\0unserialize-fail:";

fn unser_fail(offset: usize) -> PhpError {
    PhpError::Error(format!("{UNSER_FAIL}{offset}"))
}

impl<'m> super::Vm<'m> {
    /// Build an already-validated simple payload (see module docs).
    pub(super) fn unserialize_direct(&mut self, bytes: &[u8]) -> Result<Zval, PhpError> {
        let mut p = Parser::new(bytes);
        let mut ctx = UnserCtx::default();
        self.ud_value(&mut p, &mut ctx)
    }

    fn ud_value(&mut self, p: &mut Parser<'_>, ctx: &mut UnserCtx) -> Result<Zval, PhpError> {
        // Every value slot takes a pre-order number (`r:` included).
        ctx.count += 1;
        let slot = ctx.count;
        let bad = || PhpError::Error("unserialize(): invalid input after validation".to_string());
        let c = p.peek().ok_or_else(bad)?;
        p.i += 1;
        if c == b'N' {
            p.i += 1;
            return Ok(Zval::Null);
        }
        p.i += 1; // ':'
        Ok(match c {
            b'b' => {
                let v = p.peek() == Some(b'1');
                p.i += 2;
                Zval::Bool(v)
            }
            b'i' => Zval::Long(p.int_until(b';').ok_or_else(bad)?),
            b'd' => Zval::Double(
                p.take_until(b';').and_then(crate::unserialize::parse_double).ok_or_else(bad)?,
            ),
            b's' => {
                let s = p.quoted_slice().ok_or_else(bad)?;
                p.i += 1;
                Zval::Str(PhpStr::new(s))
            }
            b'r' => {
                let t = p.int_until(b';').ok_or_else(bad)?;
                ctx.objs.get(&t).cloned().unwrap_or(Zval::Null)
            }
            b'a' => {
                let n = p.usize_until(b':').ok_or_else(bad)?;
                p.i += 1; // '{'
                let mut arr = PhpArray::new();
                for _ in 0..n {
                    let key = self.ud_key(p).ok_or_else(bad)?;
                    let val = self.ud_value(p, ctx)?;
                    arr.insert(key, val);
                }
                p.i += 1; // '}'
                Zval::Array(Rc::new(arr))
            }
            b'O' => self.ud_object(p, ctx, slot)?,
            b'E' => {
                let start = p.i - 2;
                let raw = p.quoted_slice().ok_or_else(bad)?;
                p.i += 1; // ';'
                let v = self.unser_enum(raw, start, p.i)?;
                ctx.objs.insert(slot, v.clone());
                v
            }
            _ => return Err(bad()),
        })
    }

    /// An array key: `i:<n>;` or `s:<len>:"<bytes>";` (validated).
    fn ud_key(&mut self, p: &mut Parser<'_>) -> Option<Key> {
        let c = p.peek()?;
        p.i += 2;
        if c == b'i' {
            return Some(Key::Int(p.int_until(b';')?));
        }
        let s = p.quoted_slice()?;
        p.i += 1;
        Some(Key::from_bytes(s))
    }

    /// A property name: a string, or an int key kept in its decimal form.
    fn ud_prop_name(&mut self, p: &mut Parser<'_>) -> Option<Vec<u8>> {
        let c = p.peek()?;
        p.i += 2;
        if c == b'i' {
            return Some(p.int_until(b';')?.to_string().into_bytes());
        }
        let s = p.quoted_slice()?.to_vec();
        p.i += 1;
        Some(s)
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
        if !self.class_index.contains_key(lower.as_slice()) {
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

    /// `O:` of an enum class: PHP's object_init_ex refuses it.
    pub(super) fn unser_reject_enum(&self, cid: Option<ClassId>) -> Result<(), PhpError> {
        match cid {
            Some(cid) if matches!(self.classes[cid].instantiable, Instantiable::Enum) => Err(PhpError::Error(
                format!("Cannot instantiate enum {}", String::from_utf8_lossy(&self.classes[cid].name)),
            )),
            _ => Ok(()),
        }
    }

    /// `O:<len>:"<class>":<n>:{...}` — `vm_ser_build`'s `Ser::Object` arm.
    fn ud_object(&mut self, p: &mut Parser<'_>, ctx: &mut UnserCtx, slot: i64) -> Result<Zval, PhpError> {
        let bad = || PhpError::Error("unserialize(): invalid input after validation".to_string());
        let class = p.quoted_slice().ok_or_else(bad)?;
        p.i += 1; // ':'
        let n = p.usize_until(b':').ok_or_else(bad)?;
        p.i += 1; // '{'
        let lower = class.to_ascii_lowercase();
        if matches!(lower.as_slice(), b"pdo" | b"pdostatement" | b"pdorow") {
            let msg = format!("Unserialization of '{}' is not allowed", String::from_utf8_lossy(class));
            if let Some(cid) = self.class_index.get(&b"exception"[..]).copied() {
                let obj = self.synthesize_throwable(cid, &msg)?;
                return Err(PhpError::Thrown(obj));
            }
        }
        if !self.class_index.contains_key(lower.as_slice()) {
            self.try_autoload(class, &lower, None)?;
        }
        let cid = self.class_index.get(lower.as_slice()).copied();
        self.unser_reject_enum(cid)?;
        if let Some(cid) = cid {
            if resolve_method_runtime(&self.classes, cid, b"__unserialize").is_some() {
                let obj = self.vm_make_unserialized_object(class, Vec::new());
                ctx.objs.insert(slot, obj.clone());
                let mut data = PhpArray::new();
                for _ in 0..n {
                    let name = self.ud_prop_name(p).ok_or_else(bad)?;
                    let val = self.ud_value(p, ctx)?;
                    data.insert(Key::from_bytes(&name), val);
                }
                p.i += 1; // '}'
                self.call_method_sync(obj.clone(), b"__unserialize", vec![Zval::Array(Rc::new(data))])?;
                return Ok(obj);
            }
        }
        let obj = self.vm_unserialized_shell(class);
        ctx.objs.insert(slot, obj.clone());
        let mut fields: Vec<(Vec<u8>, Zval)> = Vec::with_capacity(n);
        for _ in 0..n {
            let name = self.ud_prop_name(p).ok_or_else(bad)?;
            let val = self.ud_value(p, ctx)?;
            fields.push((name, val));
        }
        p.i += 1; // '}'
        self.vm_apply_unserialized_fields(&obj, fields);
        if let Some(cid) = cid {
            if resolve_method_runtime(&self.classes, cid, b"__wakeup").is_some() {
                self.call_method_sync(obj.clone(), b"__wakeup", Vec::new())?;
            }
        }
        Ok(obj)
    }
}
