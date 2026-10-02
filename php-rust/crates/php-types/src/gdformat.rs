//! libgd's own `.gd` and `.gd2` image formats, in safe Rust.
//!
//! PHP builds its bundled libgd with them (`imagegd`, `imagegd2`,
//! `imagecreatefromgd*`); distribution libgd 2.3 builds usually omit them
//! ("GD2 image support has been disabled"), so the linked library cannot be
//! used. This is a port of php-src `ext/gd/libgd/gd_gd.c` and `gd_gd2.c`,
//! byte for byte: the same header, colour table, chunk layout, chunk index
//! and zlib `compress()` stream (level 6, window 15, memLevel 8).

use crate::gdio::{self, GdImg, MAX_COLORS};
use crate::zlibio;

const GD2_ID: &[u8; 4] = b"gd2\0";
const GD2_VERS: i32 = 2;
const GD2_CHUNKSIZE: i32 = 128;
const GD2_CHUNKSIZE_MIN: i32 = 64;
const GD2_CHUNKSIZE_MAX: i32 = 4096;
const GD2_FMT_RAW: i32 = 1;
const GD2_FMT_COMPRESSED: i32 = 2;
const GD2_FMT_TRUECOLOR_RAW: i32 = 3;
const GD2_FMT_TRUECOLOR_COMPRESSED: i32 = 4;
const Z_FINISH: i32 = 4;

fn compressed(fmt: i32) -> bool {
    fmt == GD2_FMT_COMPRESSED || fmt == GD2_FMT_TRUECOLOR_COMPRESSED
}
fn truecolor(fmt: i32) -> bool {
    fmt == GD2_FMT_TRUECOLOR_RAW || fmt == GD2_FMT_TRUECOLOR_COMPRESSED
}

// ---- writing ------------------------------------------------------------

fn put_word(out: &mut Vec<u8>, w: i32) {
    out.extend_from_slice(&[(w >> 8) as u8, w as u8]);
}
fn put_int(out: &mut Vec<u8>, i: i32) {
    out.extend_from_slice(&i.to_be_bytes());
}

/// `_gdPutColors`: truecolor flag, colour count (palette), transparent, and
/// the whole 256-entry table (palette).
fn put_colors(im: &GdImg, out: &mut Vec<u8>) {
    let tc = im.true_color();
    out.push(tc as u8);
    if !tc {
        put_word(out, im.colors_total());
    }
    put_int(out, im.transparent());
    if !tc {
        for i in 0..MAX_COLORS {
            let (r, g, b, a) = im.palette_entry(i);
            out.extend_from_slice(&[r as u8, g as u8, b as u8, a as u8]);
        }
    }
}

/// `gdImageGdPtr`.
pub fn encode_gd(im: &GdImg) -> Vec<u8> {
    let mut out = Vec::new();
    put_word(&mut out, if im.true_color() { 65534 } else { 65535 });
    put_word(&mut out, im.sx());
    put_word(&mut out, im.sy());
    put_colors(im, &mut out);
    for y in 0..im.sy() {
        for x in 0..im.sx() {
            let p = im.get_pixel(x, y);
            if im.true_color() {
                put_int(&mut out, p);
            } else {
                out.push(p as u8);
            }
        }
    }
    out
}

/// zlib's `compress()`: deflateInit(Z_DEFAULT_COMPRESSION) = level 6,
/// window 15, memLevel 8, default strategy.
fn zlib_compress(data: &[u8]) -> Option<Vec<u8>> {
    zlibio::ZCtx::new_deflate(-1, 15, 8, 0, None)?.add(data, Z_FINISH).ok()
}

/// `gdImageGd2Ptr(im, cs, fmt)`.
pub fn encode_gd2(im: &GdImg, cs: i32, fmt: i32) -> Vec<u8> {
    let tc = im.true_color();
    let bpp = if tc { 4 } else { 1 };
    let mut fmt = if fmt != GD2_FMT_RAW && fmt != GD2_FMT_COMPRESSED { GD2_FMT_COMPRESSED } else { fmt };
    if tc {
        fmt += 2;
    }
    let cs = match cs {
        0 => GD2_CHUNKSIZE,
        c if c < GD2_CHUNKSIZE_MIN => GD2_CHUNKSIZE_MIN,
        c if c > GD2_CHUNKSIZE_MAX => GD2_CHUNKSIZE_MAX,
        c => c,
    };
    let (sx, sy) = (im.sx(), im.sy());
    let ncx = (sx + cs - 1) / cs;
    let ncy = (sy + cs - 1) / cs;
    let mut out = Vec::new();
    out.extend_from_slice(GD2_ID);
    for w in [GD2_VERS, sx, sy, cs, fmt, ncx, ncy] {
        put_word(&mut out, w);
    }
    // compressed: the chunk index (offset, size per chunk) is written last,
    // into space reserved here
    let idx_pos = out.len();
    if compressed(fmt) {
        out.resize(idx_pos + (ncx * ncy) as usize * 8, 0);
    }
    put_colors(im, &mut out);
    let comp_max = (cs as f32 * bpp as f32 * cs as f32 * 1.02f32) as usize + 12;
    let mut index: Vec<(i32, i32)> = Vec::new();
    let mut chunk = Vec::with_capacity((cs * cs * bpp) as usize);
    for cy in 0..ncy {
        for cx in 0..ncx {
            let (ylo, yhi) = (cy * cs, (cy * cs + cs).min(sy));
            let (xlo, xhi) = (cx * cs, (cx * cs + cs).min(sx));
            chunk.clear();
            for y in ylo..yhi {
                for x in xlo..xhi {
                    let p = im.get_pixel(x, y);
                    match (compressed(fmt), tc) {
                        // gdTrueColorGetAlpha/Red/Green/Blue
                        (true, true) => chunk.extend_from_slice(&[((p & 0x7F00_0000) >> 24) as u8, (p >> 16) as u8, (p >> 8) as u8, p as u8]),
                        (true, false) => chunk.push(p as u8),
                        (false, true) => put_int(&mut out, p),
                        (false, false) => out.push(p as u8),
                    }
                }
            }
            if compressed(fmt) {
                // libgd's fixed output buffer: a chunk that does not fit is
                // reported and left out of the index
                match zlib_compress(&chunk).filter(|z| z.len() <= comp_max) {
                    Some(z) => {
                        index.push((out.len() as i32, z.len() as i32));
                        out.extend_from_slice(&z);
                    }
                    None => gdio::push_error("Error from compressing"),
                }
            }
        }
    }
    if compressed(fmt) {
        for (i, (off, size)) in index.iter().enumerate() {
            out[idx_pos + i * 8..idx_pos + i * 8 + 4].copy_from_slice(&off.to_be_bytes());
            out[idx_pos + i * 8 + 4..idx_pos + i * 8 + 8].copy_from_slice(&size.to_be_bytes());
        }
    }
    out
}

// ---- reading ------------------------------------------------------------

/// A read cursor with libgd's `gdGetC`/`gdGetWord`/`gdGetInt` semantics
/// (big-endian; `None` at end of data) and its dynamic-context seek (no
/// seeking past the end of the data).
struct In<'a> {
    d: &'a [u8],
    pos: usize,
}

impl In<'_> {
    fn byte(&mut self) -> Option<i32> {
        let b = *self.d.get(self.pos)?;
        self.pos += 1;
        Some(b as i32)
    }
    fn word(&mut self) -> Option<i32> {
        Some((self.byte()? << 8) | self.byte()?)
    }
    fn int(&mut self) -> Option<i32> {
        let b = self.d.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn seek(&mut self, pos: i64) -> bool {
        if pos < 0 || pos as usize > self.d.len() {
            return false;
        }
        self.pos = pos as usize;
        true
    }
}

/// `_gdGetColors`: `gd2x` = a gd 2.x header (alpha in the table, 32-bit
/// transparent). The image was created with the file's truecolor flag.
fn get_colors(input: &mut In, im: &mut GdImg, gd2x: bool) -> Option<()> {
    let (total, transparent) = if gd2x {
        if (input.byte()? != 0) != im.true_color() {
            return None;
        }
        let total = if im.true_color() { im.colors_total() } else { input.word()? };
        if total > MAX_COLORS as i32 {
            return None;
        }
        (total, input.int()?)
    } else {
        let total = input.byte()?;
        let t = input.word()?;
        (total, if t == 257 { -1 } else { t })
    };
    if im.true_color() {
        im.set_color_header(total, transparent, None);
        return Some(());
    }
    let mut table = Vec::with_capacity(MAX_COLORS);
    for _ in 0..MAX_COLORS {
        let (r, g, b) = (input.byte()?, input.byte()?, input.byte()?);
        let a = if gd2x { Some(input.byte()?) } else { None };
        table.push((r, g, b, a));
    }
    im.set_color_header(total, transparent, Some(&table));
    Some(())
}

/// `gdImageCreateFromGdPtr`.
pub fn decode_gd(data: &[u8]) -> Option<GdImg> {
    let mut input = In { d: data, pos: 0 };
    let mut sx = input.word()?;
    let (mut gd2x, mut tc) = (false, false);
    if sx == 65535 || sx == 65534 {
        gd2x = true;
        tc = sx == 65534;
        sx = input.word()?;
    }
    let sy = input.word()?;
    let mut im = GdImg::create(sx, sy, tc)?;
    get_colors(&mut input, &mut im, gd2x)?;
    for y in 0..sy {
        for x in 0..sx {
            let p = if im.true_color() { input.int()? } else { input.byte()? };
            im.put_raw_pixel(x, y, p);
        }
    }
    Some(im)
}

struct Gd2Header {
    sx: i32,
    sy: i32,
    cs: i32,
    vers: i32,
    fmt: i32,
    ncx: i32,
    ncy: i32,
    index: Vec<(i32, i32)>,
}

/// `_gd2GetHeader`.
fn gd2_header(input: &mut In) -> Option<Gd2Header> {
    if input.d.get(..4)? != GD2_ID {
        return None;
    }
    input.pos = 4;
    let vers = input.word()?;
    if vers != 1 && vers != 2 {
        return None;
    }
    let (sx, sy, cs) = (input.word()?, input.word()?, input.word()?);
    if !(GD2_CHUNKSIZE_MIN..=GD2_CHUNKSIZE_MAX).contains(&cs) {
        return None;
    }
    let fmt = input.word()?;
    if !(GD2_FMT_RAW..=GD2_FMT_TRUECOLOR_COMPRESSED).contains(&fmt) {
        return None;
    }
    let (ncx, ncy) = (input.word()?, input.word()?);
    let mut index = Vec::new();
    if compressed(fmt) {
        let nc = ncx as i64 * ncy as i64;
        if nc <= 0 || nc * 8 > i32::MAX as i64 {
            return None;
        }
        for _ in 0..nc {
            let (off, size) = (input.int()?, input.int()?);
            if off < 0 || size < 0 {
                return None;
            }
            index.push((off, size));
        }
    }
    Some(Gd2Header { sx, sy, cs, vers, fmt, ncx, ncy, index })
}

/// `_gd2ReadChunk`: one zlib stream at `off`, inflated into at most
/// `max` bytes (libgd's fixed chunk buffer).
fn read_chunk(data: &[u8], off: i32, size: i32, max: usize) -> Option<Vec<u8>> {
    let comp = data.get(off as usize..off as usize + size as usize)?;
    zlibio::uncompress(comp, 15).filter(|v| v.len() <= max)
}

/// `gdImageCreateFromGd2Ptr`.
pub fn decode_gd2(data: &[u8]) -> Option<GdImg> {
    let mut input = In { d: data, pos: 0 };
    let h = gd2_header(&mut input)?;
    let mut im = GdImg::create(h.sx, h.sy, truecolor(h.fmt))?;
    get_colors(&mut input, &mut im, h.vers == 2)?;
    let tc = im.true_color();
    let chunk_max = (h.cs * h.cs * if tc { 4 } else { 1 }) as usize;
    let mut n = 0;
    for cy in 0..h.ncy {
        for cx in 0..h.ncx {
            let (ylo, yhi) = (cy * h.cs, (cy * h.cs + h.cs).min(im.sy()));
            let chunk = if compressed(h.fmt) {
                let (off, size) = h.index[n];
                Some(read_chunk(data, off, size, chunk_max)?)
            } else {
                None
            };
            let mut pos = 0;
            for y in ylo..yhi {
                let (xlo, xhi) = (cx * h.cs, (cx * h.cs + h.cs).min(im.sx()));
                for x in xlo..xhi {
                    let p = match &chunk {
                        None => {
                            let v = if tc { input.int() } else { input.byte() };
                            let Some(v) = v else {
                                gdio::push_error("gd2: EOF while reading\n");
                                return None;
                            };
                            v
                        }
                        Some(c) if tc => {
                            let b = c.get(pos..pos + 4).map_or([0; 4], |b| [b[0], b[1], b[2], b[3]]);
                            pos += 4;
                            i32::from_be_bytes(b)
                        }
                        Some(c) => {
                            pos += 1;
                            *c.get(pos - 1).unwrap_or(&0) as i32
                        }
                    };
                    im.put_raw_pixel(x, y, p);
                }
            }
            n += 1;
        }
    }
    Some(im)
}

/// `gdImageCreateFromGd2PartPtr`: the `w`x`h` region at (`srcx`, `srcy`).
pub fn decode_gd2_part(data: &[u8], srcx: i32, srcy: i32, w: i32, h: i32) -> Option<GdImg> {
    if w < 1 || h < 1 {
        return None;
    }
    let mut input = In { d: data, pos: 0 };
    let hd = gd2_header(&mut input)?;
    let (fsx, fsy, cs) = (hd.sx, hd.sy, hd.cs);
    let mut im = GdImg::create(w, h, truecolor(hd.fmt))?;
    get_colors(&mut input, &mut im, hd.vers == 2)?;
    let tc = im.true_color();
    let chunk_max = (cs * cs * if tc { 4 } else { 1 }) as usize;
    let (scx, scy) = ((srcx / cs).max(0), (srcy / cs).max(0));
    let (ecx, ecy) = (((srcx + w) / cs).min(hd.ncx - 1), ((srcy + h) / cs).min(hd.ncy - 1));
    let dstart = input.pos as i64;
    for cy in scy..=ecy {
        let (ylo, yhi) = (cy * cs, (cy * cs + cs).min(fsy));
        for cx in scx..=ecx {
            let (xlo, xhi) = (cx * cs, (cx * cs + cs).min(fsx));
            let chunk = if compressed(hd.fmt) {
                let Some(&(off, size)) = hd.index.get((cx + cy * hd.ncx) as usize) else { return None };
                let Some(c) = read_chunk(data, off, size, chunk_max) else {
                    gdio::push_error("Error reading comproessed chunk");
                    return None;
                };
                Some(c)
            } else {
                let bpp = if tc { 4 } else { 1 } as i64;
                let dpos = (cy as i64 * (cs as i64 * fsx as i64) + cx as i64 * cs as i64 * (yhi - ylo) as i64) * bpp + dstart;
                if !input.seek(dpos) {
                    gdio::push_error("Error from seek: 0");
                    return None;
                }
                None
            };
            let mut pos = 0;
            for y in ylo..yhi {
                for x in xlo..xhi {
                    let ch: u32 = match &chunk {
                        None if tc => input.int().unwrap_or(0) as u32,
                        None => input.byte().unwrap_or(0) as u32,
                        Some(c) if tc => {
                            let b = c.get(pos..pos + 4).map_or([0; 4], |b| [b[0], b[1], b[2], b[3]]);
                            pos += 4;
                            u32::from_be_bytes(b)
                        }
                        Some(c) => {
                            pos += 1;
                            *c.get(pos - 1).unwrap_or(&0) as u32
                        }
                    };
                    if x >= srcx && x < srcx + w && x < fsx && x >= 0 && y >= srcy && y < srcy + h && y < fsy && y >= 0 {
                        im.put_raw_pixel(x - srcx, y - srcy, ch as i32);
                    }
                }
            }
        }
    }
    Some(im)
}
