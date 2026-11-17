//! How 2D objects are drawn beyond a solid pen and a solid fill: line types (dashes), hatches,
//! gradients, tiles and image fills (`docs/FORMAT.md` § "Line types, hatches and other fills").
//!
//! The definitions are document resources, each a record with a reference number and its name
//! in the name list. An object names its line type as minus that number in its pen (`i32` at
//! 0x50) and its hatch, gradient, tile or image as minus that number in its fill (`i32` at
//! 0x64); the frame its fill is laid out in is its child of type 121 (a fill space).
//!
//! Hatches and dashes are drawn here as line segments in the coordinates of their object
//! (millimetres); `paper` is how many of those millimetres one millimetre of paper is (the scale
//! of the design layer, of the viewport, 1 on a sheet).

use vwx_core::container::Record;
use vwx_core::le;

use crate::raw::{kind, Raw};
use crate::shapes::P2;

/// Lengths of line types and hatches are stored in inches.
pub const INCH: f64 = 25.4;
/// Most line segments one hatch or tile fill of one object is drawn with; a fill that would
/// need more is left to its colour.
pub const MAX_FILL_SEGMENTS: usize = 20_000;
/// Most dashes one line is drawn with; a line that would need more is drawn solid.
pub const MAX_DASHES: usize = 20_000;

/// Object types of the definitions.
pub mod kinds {
    pub const HATCH_LIST: u8 = 65;
    pub const HATCH: u8 = 66;
    pub const LINE_TYPE: u8 = 96;
    pub const TILE: u8 = 108;
    pub const IMAGE: u8 = 119;
    pub const GRADIENT: u8 = 120;
    pub const FILL_SPACE: u8 = 121;
}

fn real(r: &Record, at: usize) -> Option<f64> {
    le::f64(r.bytes, at).filter(|v| v.is_finite())
}

/// A point stored as (y, x), returned as [x, y].
fn yx(r: &Record, at: usize) -> Option<P2> {
    Some([real(r, at + 8)?, real(r, at)?])
}

/// The scale of a layer (1 : scale): the `f64` at 0xf8 of its record (sample 175: 25 after
/// `SetLScale(layer, 25)`; the design layers of the corpus project 50, its sheets 1).
pub fn layer_scale(layer: &[u8]) -> f64 {
    le::f64(layer, 0xf8).filter(|s| s.is_finite() && *s > 0.0).unwrap_or(1.0)
}

/// A line type (type 96, 354 bytes).
#[derive(Debug, Clone, PartialEq)]
pub struct LineType {
    /// Dash, gap, dash, gap, ... in inches on paper; empty for a line type without dashes.
    pub dashes: Vec<f64>,
    /// The dashes grow with the weight of the line (the byte at 0x60).
    pub scale_with_thickness: bool,
    /// A line type drawn with geometry (its group child) instead of dashes: the byte at 0x57
    /// is 0 or 2 (3 for dashes).
    pub complex: bool,
}

/// A line type (type 96): the byte at 0x57 is 3 for one of dashes; the byte at 0x60 says the
/// dashes scale with the line weight, the byte at 0x61 is the count `n` of values, `f64` from
/// 0x62: first the length of the whole pattern, then where each dash ends and the next begins,
/// counted from its start (samples 173: `LT-b` of a dash of 20 and a gap of 5 has 25, 20; two
/// pairs 0.12, 0.18, 0.03, 0.07 give 0.4, 0.12, 0.3, 0.33). In inches. A few corpus records
/// have no length first but after the last place; the length is then the value after them.
pub fn line_type(r: &Record) -> Option<LineType> {
    if r.kind() != kinds::LINE_TYPE {
        return None;
    }
    let complex = r.bytes.get(0x57) != Some(&3);
    let n = *r.bytes.get(0x61)? as usize;
    let swt = r.bytes.get(0x60) == Some(&1);
    let mut dashes = Vec::new();
    if !complex && n >= 2 {
        let values: Vec<Option<f64>> = (0..=n).map(|k| real(r, 0x62 + 8 * k)).collect();
        let places: Vec<f64> = values[1..n].iter().map(|v| v.unwrap_or(f64::NAN)).collect();
        let last = places.last().copied().unwrap_or(0.0);
        let length = match values[0] {
            Some(v) if v > last && v < 1e3 => v,
            _ => values[n].filter(|v| *v > last)?,
        };
        let mut from = 0.0;
        for p in places.into_iter().chain([length]) {
            if !(p >= from) {
                return None;
            }
            dashes.push(p - from);
            from = p;
        }
    }
    Some(LineType { dashes, scale_with_thickness: swt, complex })
}

/// One level (family of parallel lines) of a hatch, in inches (on paper for a page-based hatch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HatchLevel {
    /// A point a dash starts at.
    pub start: P2,
    /// The direction of the lines, as long as one dash and its gap.
    pub repeat: P2,
    /// From a line to the next: a point a dash starts at on the next line is `start + offset`.
    pub offset: P2,
    /// The part of `repeat` that is drawn (1: solid lines).
    pub dash: f64,
    /// Palette index of its pen.
    pub color: u16,
    /// Line weight in thousandths of an inch.
    pub weight: u16,
}

/// A hatch (type 66).
#[derive(Debug, Clone, PartialEq)]
pub struct Hatch {
    /// Its lengths are on paper (scale with the layer) rather than in the model.
    pub page: bool,
    pub rotate_in_wall: bool,
    pub rotate_in_symbol: bool,
    /// No background is drawn under the lines.
    pub transparent: bool,
    /// Palette index of its background.
    pub background: u16,
    pub levels: Vec<HatchLevel>,
}

/// A hatch (type 66, 88 + 60 n bytes): the byte at 0x51 holds bits 0x01 page based, 0x02 rotate
/// in walls, 0x04 transparent, 0x08 rotate in symbols (job 171/172: the values the scripting
/// API gives with selectors 661 to 664); `u16` palette index of the background at 0x54, `u16`
/// count of levels at 0x56, from 0x58 60 bytes per level: start, repeat and offset as (y, x),
/// the dash factor as `f64`, `u16` palette index of the pen, `u8` line weight in mils.
pub fn hatch(r: &Record) -> Option<Hatch> {
    if r.kind() != kinds::HATCH {
        return None;
    }
    let flags = *r.bytes.get(0x51)?;
    let count = le::u16(r.bytes, 0x56)? as usize;
    let levels: Option<Vec<HatchLevel>> = (0..count)
        .map(|k| {
            let at = 0x58 + 60 * k;
            Some(HatchLevel { start: yx(r, at)?, repeat: yx(r, at + 16)?, offset: yx(r, at + 32)?, dash: real(r, at + 48)?, color: le::u16(r.bytes, at + 56)?, weight: *r.bytes.get(at + 58)? as u16 })
        })
        .collect();
    Some(Hatch {
        page: flags & 0x01 != 0,
        rotate_in_wall: flags & 0x02 != 0,
        transparent: flags & 0x04 != 0,
        rotate_in_symbol: flags & 0x08 != 0,
        background: le::u16(r.bytes, 0x54)?,
        levels: levels?,
    })
}

/// A colour stop of a gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    pub position: f64,
    pub midpoint: f64,
    pub color: [u8; 3],
    pub opacity: u16,
}

/// A gradient (type 120, 84 + 24 n bytes): `u16` count of stops at 0x52; from 0x54 per stop
/// its position and the midpoint to the next as `f64`, red, green, blue as `u16` 0..255 and the
/// opacity in percent (job 174: red at 0, green at 0.3 with midpoint 0.4, blue at 1).
pub fn gradient(r: &Record) -> Option<Vec<Stop>> {
    if r.kind() != kinds::GRADIENT {
        return None;
    }
    let count = le::u16(r.bytes, 0x52)? as usize;
    (0..count)
        .map(|k| {
            let at = 0x54 + 24 * k;
            let channel = |c: usize| le::u16(r.bytes, at + 16 + 2 * c).map(|v| v.min(255) as u8);
            Some(Stop { position: real(r, at)?, midpoint: real(r, at + 8)?, color: [channel(0)?, channel(1)?, channel(2)?], opacity: le::u16(r.bytes, at + 22)? })
        })
        .collect()
}

/// The colour a gradient is drawn in: the mean of its stops.
pub fn gradient_color(stops: &[Stop]) -> Option<[u8; 3]> {
    if stops.is_empty() {
        return None;
    }
    let mean = |c: usize| (stops.iter().map(|s| s.color[c] as u32).sum::<u32>() / stops.len() as u32) as u8;
    Some([mean(0), mean(1), mean(2)])
}

/// A tile fill (type 108).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tile {
    /// The two steps from a tile to its neighbours (millimetres).
    pub repeat: P2,
    pub offset: P2,
    /// Palette index of its background.
    pub background: u16,
}

/// A tile fill (type 108, 122 bytes): `u16` palette index of the background at 0x58, the
/// repetition point at 0x5a and the offset point at 0x6a, each (y, x) (job 174: background
/// green, repetition (40, 25), offset (3, 4)). Its group child holds the geometry of one tile.
/// In the corpus the two points are the two sides of the grid the tiles lie on: (0, 100) and
/// (37.5, 0).
pub fn tile(r: &Record) -> Option<Tile> {
    if r.kind() != kinds::TILE {
        return None;
    }
    Some(Tile { background: le::u16(r.bytes, 0x58)?, repeat: yx(r, 0x5a)?, offset: yx(r, 0x6a)? })
}

/// An image (type 119): a QuickTime-style image description and the compressed image.
#[derive(Debug, Clone, PartialEq)]
pub struct Image<'a> {
    pub width: u32,
    pub height: u32,
    /// Bits per pixel.
    pub depth: u32,
    /// `png`, `jpeg`, ... (the codec of its description, lower case).
    pub format: String,
    /// The image file as stored (a PNG or JPEG file).
    pub data: &'a [u8],
}

/// An image (type 119): width, height and bits per pixel as `u32` at 0x7c, 0x80, 0x84; from
/// 0x94 an image description (its `u32` size first, then its codec stored reversed, `gnp ` for
/// PNG and `gepj` for JPEG; its `u32` data size at 0x2c), then the image itself (corpus: of the
/// 50 976 images 44 162 PNG, 6 660 JPEG; most are the previews resources keep as children).
pub fn image<'a>(r: &Record<'a>) -> Option<Image<'a>> {
    if r.kind() != kinds::IMAGE {
        return None;
    }
    let size = le::u32(r.bytes, 0x94)? as usize;
    let codec: Vec<u8> = r.bytes.get(0x98..0x9c)?.iter().rev().copied().collect();
    let format = String::from_utf8_lossy(&codec).trim().to_lowercase();
    let start = 0x94 + size;
    let length = le::u32(r.bytes, 0x94 + 0x2c)? as usize;
    let data = r.bytes.get(start..start.checked_add(length)?.min(r.bytes.len()))?;
    Some(Image { width: le::u32(r.bytes, 0x7c)?, height: le::u32(r.bytes, 0x80)?, depth: le::u32(r.bytes, 0x84)?, format, data })
}

/// The frame a fill of an object is laid out in: a point and the ends of its two axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FillSpace {
    pub origin: P2,
    pub i: P2,
    pub j: P2,
}

/// A fill space (type 121, 136 bytes, a child of the filled object): its origin, the end of its
/// i axis and of its j axis, each (y, x), from 0x50 (job 171: a rectangle 1000 x 500 with a hatch
/// has its centre and axes of 508 = 20 inches, the scale of the layer times an inch; job 172:
/// `Fill Angle` 0.5 turns both axes, `Fill X Offset` moves the origin from the centre). The
/// children of mesh faces (type 41) that most corpus fill spaces are lay out textures.
pub fn fill_space(r: &Record) -> Option<FillSpace> {
    if r.kind() != kinds::FILL_SPACE {
        return None;
    }
    Some(FillSpace { origin: yx(r, 0x50)?, i: yx(r, 0x60)?, j: yx(r, 0x70)? })
}

/// The pen of a drawn object, its own or its class's (`i32` at 0x50; of the class at 0x5a).
pub fn pen(r: &Record, class: Option<&Record>) -> i32 {
    let by_class = r.bytes.get(0x24).is_some_and(|f| f & crate::shapes::by_class::PEN != 0);
    match class.filter(|_| by_class) {
        Some(class) => le::u32(class.bytes, 0x5a).unwrap_or(0) as i32,
        None => le::u32(r.bytes, 0x50).unwrap_or(0) as i32,
    }
}

/// The fill of a drawn object, its own or its class's (`i32` at 0x64; of the class at 0x60).
pub fn fill(r: &Record, class: Option<&Record>) -> i32 {
    let by_class = r.bytes.get(0x24).is_some_and(|f| f & crate::shapes::by_class::FILL != 0);
    match class.filter(|_| by_class) {
        Some(class) => le::u32(class.bytes, 0x60).unwrap_or(0) as i32,
        None => le::u32(r.bytes, 0x64).unwrap_or(0) as i32,
    }
}

/// The definition a negative pen or fill names.
pub fn definition<'r, 'a>(raw: &'r Raw<'a>, value: i32) -> Option<&'r Record<'a>> {
    let reference = value.checked_neg().filter(|v| *v > 0)? as u32;
    raw.by_ref.get(&reference).map(|&k| &raw.records[k])
}

/// The dashes a drawn object's lines are drawn with, in the millimetres of the object: `None`
/// for a solid pen (or a line type of geometry, drawn solid).
pub fn dashes(raw: &Raw, r: &Record, class: Option<&Record>, paper: f64) -> Option<Vec<f64>> {
    let t = line_type(definition(raw, pen(r, class))?)?;
    let pattern: Vec<f64> = t.dashes.iter().map(|d| d * INCH * paper).collect();
    (pattern.len() >= 2 && pattern.iter().sum::<f64>() > 1e-9).then_some(pattern)
}

/// The dashes of a line through `points` (and back to the first with `closed`) drawn with
/// `pattern` (dash, gap, ...): the pattern runs on along the whole line. `None` when it would
/// take more than [`MAX_DASHES`].
pub fn dash(points: &[P2], closed: bool, pattern: &[f64]) -> Option<Vec<[P2; 2]>> {
    let period: f64 = pattern.iter().sum();
    let mut path: Vec<P2> = points.to_vec();
    if closed && points.len() > 2 {
        path.push(points[0]);
    }
    let length: f64 = path.windows(2).map(|w| dist(w[0], w[1])).sum();
    if !(period > 0.0) || length / period * pattern.len() as f64 > 2.0 * MAX_DASHES as f64 {
        return None;
    }
    let mut out = Vec::new();
    // where along the line each piece of the pattern begins and ends
    let (mut at, mut k) = (0.0, 0usize);
    let mut pieces = Vec::new();
    while at < length {
        let end = at + pattern[k % pattern.len()];
        if k % 2 == 0 && end > at {
            pieces.push((at, end.min(length)));
        }
        at = end;
        k += 1;
    }
    // each piece along the path, split at its corners
    let mut start = 0.0;
    let mut piece = 0;
    for w in path.windows(2) {
        let len = dist(w[0], w[1]);
        let end = start + len;
        while piece < pieces.len() && pieces[piece].0 < end {
            let (a, b) = pieces[piece];
            let (from, to) = (a.max(start), b.min(end));
            if to > from && len > 0.0 {
                out.push([lerp(w[0], w[1], (from - start) / len), lerp(w[0], w[1], (to - start) / len)]);
            }
            if b <= end {
                piece += 1;
            } else {
                break;
            }
        }
        start = end;
    }
    Some(out)
}

fn dist(a: P2, b: P2) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn lerp(a: P2, b: P2, t: f64) -> P2 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Joins segments that follow each other into lines (the sides of a polyline, so that its
/// dashes run on around its curves).
pub fn chains(segments: &[[P2; 2]]) -> Vec<Vec<P2>> {
    let mut out: Vec<Vec<P2>> = Vec::new();
    for [a, b] in segments {
        match out.last_mut() {
            Some(line) if line.last().is_some_and(|p| dist(*p, *a) < 1e-9) => line.push(*b),
            _ => out.push(vec![*a, *b]),
        }
    }
    out
}

/// The parts of the line from `a` to `b` inside `outline` (even-odd: what a polyline runs
/// around twice to reach its hole is not inside), as parameters along it, 0 at `a` and 1 at `b`.
pub fn clip(outline: &[P2], a: P2, b: P2) -> Vec<(f64, f64)> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let side = |p: P2| d[0] * (p[1] - a[1]) - d[1] * (p[0] - a[0]);
    let along = |p: P2| d[0] * (p[0] - a[0]) + d[1] * (p[1] - a[1]);
    let dd = d[0] * d[0] + d[1] * d[1];
    if dd <= 0.0 {
        return Vec::new();
    }
    let mut cuts = Vec::new();
    for k in 0..outline.len() {
        let (p, q) = (outline[k], outline[(k + 1) % outline.len()]);
        let (sp, sq) = (side(p), side(q));
        // (an end on the line counts as on one side, so a line through a corner crosses once)
        if (sp > 0.0) != (sq > 0.0) {
            let t = sp / (sp - sq);
            cuts.push(along(lerp(p, q, t)) / dd);
        }
    }
    cuts.sort_by(f64::total_cmp);
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (s, e) in cuts.chunks_exact(2).map(|c| (c[0].max(0.0), c[1].min(1.0))).filter(|(s, e)| e > s) {
        // (parts that touch, as on either side of the way to a hole, are one)
        match out.last_mut() {
            Some(last) if s <= last.1 + 1e-12 => last.1 = last.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

/// How a fill's own lengths lie in the coordinates of its object: a point, and where one unit
/// along x and along y of the fill goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub origin: P2,
    pub x: P2,
    pub y: P2,
}

impl Frame {
    pub fn point(&self, p: P2) -> P2 {
        [self.origin[0] + p[0] * self.x[0] + p[1] * self.y[0], self.origin[1] + p[0] * self.x[1] + p[1] * self.y[1]]
    }

    pub fn vector(&self, v: P2) -> P2 {
        [v[0] * self.x[0] + v[1] * self.y[0], v[0] * self.x[1] + v[1] * self.y[1]]
    }

    /// The frame of an inch of a hatch: its fill space has axes of an inch on paper (the scale
    /// times 25.4 mm), so a page-based hatch takes them as they are and one in the model divides
    /// by the scale. Without a fill space the hatch lies at the origin, unturned.
    pub fn hatch(space: Option<&FillSpace>, page: bool, paper: f64) -> Frame {
        let (origin, x, y) = match space {
            Some(s) => (s.origin, [s.i[0] - s.origin[0], s.i[1] - s.origin[1]], [s.j[0] - s.origin[0], s.j[1] - s.origin[1]]),
            None => ([0.0; 2], [INCH * paper, 0.0], [0.0, INCH * paper]),
        };
        let k = if page { 1.0 } else { 1.0 / paper };
        Frame { origin, x: [x[0] * k, x[1] * k], y: [y[0] * k, y[1] * k] }
    }
}

/// The dashes of the lines of a hatch inside `outline` (coordinates of the object), with the
/// palette index of each: `None` when there would be more than `budget`.
pub fn hatch_lines(outline: &[P2], hatch: &Hatch, frame: &Frame, budget: usize) -> Option<Vec<([P2; 2], u16)>> {
    let (lo, hi) = bounds(outline)?;
    let corners = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]];
    let mut out = Vec::new();
    for level in &hatch.levels {
        let (s, r, o) = (frame.point(level.start), frame.vector(level.repeat), frame.vector(level.offset));
        let period = r[0].hypot(r[1]);
        if !(period > 1e-9) {
            continue;
        }
        let d = [r[0] / period, r[1] / period];
        let n = [-d[1], d[0]];
        let dot = |p: P2, v: P2| p[0] * v[0] + p[1] * v[1];
        // the lines are s + j o: their distance across is s.n + j o.n
        let step = dot(o, n);
        if step.abs() < 1e-9 {
            continue;
        }
        let across: Vec<f64> = corners.iter().map(|&c| dot(c, n) - dot(s, n)).collect();
        let along: Vec<f64> = corners.iter().map(|&c| dot(c, d)).collect();
        let (vmin, vmax) = (across.iter().copied().fold(f64::MAX, f64::min), across.iter().copied().fold(f64::MIN, f64::max));
        let (umin, umax) = (along.iter().copied().fold(f64::MAX, f64::min) - 1.0, along.iter().copied().fold(f64::MIN, f64::max) + 1.0);
        let (j0, j1) = {
            let (a, b) = ((vmin / step).ceil(), (vmax / step).floor());
            if a <= b {
                (a, b)
            } else {
                ((vmax / step).ceil(), (vmin / step).floor())
            }
        };
        if j1 - j0 > budget as f64 {
            return None;
        }
        let solid = !(level.dash > 0.0 && level.dash < 1.0);
        for j in j0 as i64..=j1 as i64 {
            let p = [s[0] + j as f64 * o[0], s[1] + j as f64 * o[1]];
            // the line from umin to umax along d (p lies at dot(p, d) along it)
            let at = dot(p, d);
            let (a, b) = ([p[0] + (umin - at) * d[0], p[1] + (umin - at) * d[1]], [p[0] + (umax - at) * d[0], p[1] + (umax - at) * d[1]]);
            let length = umax - umin;
            for (t0, t1) in clip(outline, a, b) {
                let (u0, u1) = (umin + t0 * length - at, umin + t1 * length - at);
                let mut piece = |from: f64, to: f64| {
                    out.push(([[p[0] + from * d[0], p[1] + from * d[1]], [p[0] + to * d[0], p[1] + to * d[1]]], level.color));
                };
                if solid {
                    piece(u0, u1);
                } else {
                    // dashes start at k period from p
                    let (k0, k1) = ((u0 / period).floor() as i64, (u1 / period).ceil() as i64);
                    if (k1 - k0) as usize > budget {
                        return None;
                    }
                    for k in k0..k1 {
                        let (from, to) = ((k as f64 * period).max(u0), (k as f64 * period + level.dash * period).min(u1));
                        if to > from {
                            piece(from, to);
                        }
                    }
                }
                if out.len() > budget {
                    return None;
                }
            }
        }
    }
    Some(out)
}

/// The tiles of a tile fill inside `outline`: the segments of one tile (in the units of the
/// tile) repeated on the grid of its two steps, placed by `frame`. `None` when there would be
/// more than `budget` segments.
pub fn tile_lines(outline: &[P2], tile: &Tile, geometry: &[[P2; 2]], frame: &Frame, budget: usize) -> Option<Vec<[P2; 2]>> {
    let (lo, hi) = bounds(outline)?;
    let (a, b) = (frame.vector(tile.repeat), frame.vector(tile.offset));
    let det = a[0] * b[1] - a[1] * b[0];
    if geometry.is_empty() || det.abs() < 1e-9 {
        return Some(Vec::new());
    }
    // the cells of the grid over the box of the outline (and one more around: a tile may reach
    // beyond its cell)
    let cell = |p: P2| {
        let q = [p[0] - frame.origin[0], p[1] - frame.origin[1]];
        [(q[0] * b[1] - q[1] * b[0]) / det, (a[0] * q[1] - a[1] * q[0]) / det]
    };
    let cs: Vec<P2> = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]].iter().map(|&c| cell(c)).collect();
    let (i0, i1) = (cs.iter().map(|c| c[0]).fold(f64::MAX, f64::min).floor() - 1.0, cs.iter().map(|c| c[0]).fold(f64::MIN, f64::max).ceil() + 1.0);
    let (j0, j1) = (cs.iter().map(|c| c[1]).fold(f64::MAX, f64::min).floor() - 1.0, cs.iter().map(|c| c[1]).fold(f64::MIN, f64::max).ceil() + 1.0);
    if (i1 - i0) * (j1 - j0) * geometry.len() as f64 > budget as f64 {
        return None;
    }
    let mut out = Vec::new();
    for i in i0 as i64..=i1 as i64 {
        for j in j0 as i64..=j1 as i64 {
            let shift = [i as f64 * a[0] + j as f64 * b[0], i as f64 * a[1] + j as f64 * b[1]];
            for [p, q] in geometry {
                let (p, q) = (frame.point(*p), frame.point(*q));
                let (p, q) = ([p[0] + shift[0], p[1] + shift[1]], [q[0] + shift[0], q[1] + shift[1]]);
                out.extend(clip(outline, p, q).into_iter().map(|(t0, t1)| [lerp(p, q, t0), lerp(p, q, t1)]));
            }
            if out.len() > budget {
                return None;
            }
        }
    }
    Some(out)
}

fn bounds(points: &[P2]) -> Option<(P2, P2)> {
    if points.len() < 3 || points.iter().flatten().any(|c| !c.is_finite()) {
        return None;
    }
    let mut lo = [f64::MAX; 2];
    let mut hi = [f64::MIN; 2];
    for p in points {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    Some((lo, hi))
}

/// How the fill of a 2D object is drawn.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filling {
    /// The colour of its face, `None` for none (a transparent hatch).
    pub color: Option<[u8; 3]>,
    /// Lines on it (the lines of a hatch, the geometry of tiles), each with its colour.
    pub lines: Vec<([P2; 2], [u8; 3])>,
}

/// The line segments of the geometry of a tile: the lines and outlines among the children of
/// its group (in the units of the tile).
pub fn tile_geometry(raw: &Raw, tile: usize) -> Vec<[P2; 2]> {
    let mut out = Vec::new();
    let mut todo: Vec<(usize, usize)> = raw.children[tile].iter().map(|&c| (c, 0)).collect();
    while let Some((c, depth)) = todo.pop() {
        let r = &raw.records[c];
        match r.kind() {
            kind::GROUP if depth < 8 => todo.extend(raw.children[c].iter().map(|&k| (k, depth + 1))),
            kind::LINE => out.extend(crate::shapes::line(r)),
            kind::POLYLINE => out.extend(crate::shapes::polyline(r).map(|v| crate::shapes::polyline_segments(&v, crate::shapes::closed(r))).unwrap_or_default()),
            kind::ARC => {
                let points = crate::shapes::outline(r).map(|mut o| {
                    o.push(o[0]);
                    o
                });
                out.extend(points.or_else(|| crate::shapes::arc(r)).unwrap_or_default().windows(2).map(|w| [w[0], w[1]]));
            }
            _ => {
                if let Some(o) = crate::shapes::outline(r) {
                    let closed = crate::shapes::closed(r);
                    out.extend(o.windows(2).map(|w| [w[0], w[1]]));
                    if closed {
                        out.push([o[o.len() - 1], o[0]]);
                    }
                }
            }
        }
    }
    out.retain(|s| s.iter().flatten().all(|c| c.is_finite()));
    out
}

/// How the fill of drawn object `i` (with its outline in its own coordinates) is drawn:
/// a solid fill or pattern in the fill colour `color`; a hatch as its lines over its background
/// (none when transparent); a gradient in the mean colour of its stops; a tile fill as its tiles
/// over its background; an image fill in the fill colour.
pub fn filling(raw: &Raw, i: usize, class: Option<&Record>, outline: &[P2], paper: f64, color: [u8; 3], pen: [u8; 3]) -> Filling {
    let r = &raw.records[i];
    let solid = Filling { color: Some(color), lines: Vec::new() };
    let Some(def) = definition(raw, fill(r, class)) else { return solid };
    let space = raw.children[i].iter().map(|&c| &raw.records[c]).find_map(fill_space);
    let palette = |k: u16| raw.palette.color(k);
    match def.kind() {
        kinds::HATCH => {
            let Some(h) = hatch(def) else { return solid };
            let background = (!h.transparent).then(|| palette(h.background).unwrap_or(color));
            let frame = Frame::hatch(space.as_ref(), h.page, paper);
            match hatch_lines(outline, &h, &frame, MAX_FILL_SEGMENTS) {
                Some(lines) => Filling { color: background, lines: lines.into_iter().map(|(s, k)| (s, palette(k).unwrap_or(pen))).collect() },
                // too many lines: the colour of the background, or of the fill
                None => Filling { color: background.or(Some(color)), lines: Vec::new() },
            }
        }
        kinds::GRADIENT => Filling { color: gradient(def).as_deref().and_then(gradient_color).or(Some(color)), lines: Vec::new() },
        kinds::TILE => {
            let Some(t) = tile(def) else { return solid };
            let background = palette(t.background).unwrap_or(color);
            let k = raw.by_ref.get(&def.ref_id()).copied();
            let geometry = k.map(|k| tile_geometry(raw, k)).unwrap_or_default();
            // the tile in millimetres, turned as its fill space is
            let frame = Frame::hatch(space.as_ref(), false, paper);
            let frame = Frame { origin: frame.origin, x: frame.x.map(|c| c / INCH), y: frame.y.map(|c| c / INCH) };
            match tile_lines(outline, &t, &geometry, &frame, MAX_FILL_SEGMENTS) {
                Some(lines) => Filling { color: Some(background), lines: lines.into_iter().map(|s| (s, pen)).collect() },
                None => Filling { color: Some(background), lines: Vec::new() },
            }
        }
        _ => solid,
    }
}

/// Adds the line types, hatches, gradients, tiles and image fills of a document (the named
/// ones: the images resources keep as their previews are left out) to its model, in metres.
pub fn add_to(raw: &Raw, doc: &mut vwx_model::Document) {
    let m = 0.001;
    let palette = |k: u16| raw.palette.color(k).unwrap_or([0, 0, 0]);
    for (i, r) in raw.records.iter().enumerate() {
        let id = r.ref_id() as u64;
        let Some(name) = raw.names.get(&r.ref_id()).filter(|_| id != 0).cloned() else { continue };
        match r.kind() {
            kinds::LINE_TYPE => {
                let Some(t) = line_type(r) else { continue };
                doc.line_types.push(vwx_model::LineType { id, name, dashes: t.dashes.iter().map(|d| d * INCH * m).collect(), scale_with_thickness: t.scale_with_thickness, complex: t.complex });
            }
            kinds::HATCH => {
                let Some(h) = hatch(r) else { continue };
                let v = |p: P2| p.map(|c| c * INCH * m);
                doc.hatches.push(vwx_model::Hatch {
                    id,
                    name,
                    page: h.page,
                    rotate_in_wall: h.rotate_in_wall,
                    rotate_in_symbol: h.rotate_in_symbol,
                    transparent: h.transparent,
                    background: palette(h.background),
                    levels: h
                        .levels
                        .iter()
                        .map(|l| vwx_model::HatchLevel { start: v(l.start), repeat: v(l.repeat), offset: v(l.offset), dash: l.dash, color: palette(l.color), weight: l.weight })
                        .collect(),
                });
            }
            kinds::GRADIENT => {
                let Some(stops) = gradient(r) else { continue };
                let stops = stops.iter().map(|s| vwx_model::GradientStop { position: s.position, midpoint: s.midpoint, color: s.color, opacity: s.opacity }).collect();
                doc.gradients.push(vwx_model::Gradient { id, name, stops });
            }
            kinds::TILE => {
                let Some(t) = tile(r) else { continue };
                let lines = tile_geometry(raw, i).iter().map(|s| s.map(|p| p.map(|c| c * m))).collect();
                doc.tiles.push(vwx_model::Tile { id, name, repeat: t.repeat.map(|c| c * m), offset: t.offset.map(|c| c * m), background: palette(t.background), lines });
            }
            kinds::IMAGE => {
                let Some(image) = image(r) else { continue };
                doc.image_fills.push(vwx_model::ImageFill { id, name, format: image.format, width: image.width, height: image.height, data: image.data.to_vec() });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::shapes::tests::record;

    fn put_yx(r: &mut [u8], at: usize, p: P2) {
        r[at..at + 8].copy_from_slice(&p[1].to_le_bytes());
        r[at + 8..at + 16].copy_from_slice(&p[0].to_le_bytes());
    }

    /// A hatch record of one level per entry (start, repeat, offset, dash, colour, weight).
    pub(crate) fn hatch_record(flags: u8, background: u16, levels: &[(P2, P2, P2, f64, u16, u8)]) -> Vec<u8> {
        let mut r = record(kinds::HATCH, 0x58 + 60 * levels.len(), &[]);
        r[0x51] = flags;
        r[0x54..0x56].copy_from_slice(&background.to_le_bytes());
        r[0x56..0x58].copy_from_slice(&(levels.len() as u16).to_le_bytes());
        for (k, (s, rep, o, dash, color, weight)) in levels.iter().enumerate() {
            let at = 0x58 + 60 * k;
            put_yx(&mut r, at, *s);
            put_yx(&mut r, at + 16, *rep);
            put_yx(&mut r, at + 32, *o);
            r[at + 48..at + 56].copy_from_slice(&dash.to_le_bytes());
            r[at + 56..at + 58].copy_from_slice(&color.to_le_bytes());
            r[at + 58] = *weight;
            r[at + 59] = 0x3f;
        }
        r
    }

    /// A line type record of dashes as Vectorworks keeps them: the length of the pattern, then
    /// where each piece ends.
    pub(crate) fn line_type_record(values: &[f64]) -> Vec<u8> {
        let mut r = record(kinds::LINE_TYPE, 354, &[(0x58, -1.0)]);
        r[0x57] = 3;
        r[0x61] = values.len() as u8;
        for k in 0..12 {
            let v = values.get(k).copied().unwrap_or(values[0]);
            r[0x62 + 8 * k..0x6a + 8 * k].copy_from_slice(&v.to_le_bytes());
        }
        r
    }

    fn rec(bytes: &[u8]) -> Record<'_> {
        vwx_core::container::records(bytes).unwrap().remove(0)
    }

    #[test]
    fn line_type_as_sample_173() {
        // LT-b: a dash of 20 and a gap of 5
        let r = line_type_record(&[25.0, 20.0]);
        assert_eq!(line_type(&rec(&r)).unwrap().dashes, [20.0, 5.0]);
        // two pairs 0.12, 0.18, 0.03, 0.07
        let r = line_type_record(&[0.4, 0.12, 0.3, 0.33]);
        let d = line_type(&rec(&r)).unwrap().dashes;
        let want = [0.12, 0.18, 0.03, 0.07];
        assert!(d.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-12), "{d:?}");
        // a record of geometry has no dashes
        let mut r = line_type_record(&[25.0, 20.0]);
        r[0x57] = 0;
        assert!(line_type(&rec(&r)).unwrap().complex);
    }

    #[test]
    fn hatch_as_sample_172() {
        // S-twee: two levels, the second red with weight 5
        let r = hatch_record(0x08, 256, &[([0.5, 0.25], [0.0, 3.0], [2.0, 0.0], 1.0, 257, 1), ([0.0, 0.0], [3.0, 0.0], [0.0, 1.0], 0.6, 1237, 5)]);
        let h = hatch(&rec(&r)).unwrap();
        assert!(!h.page && !h.transparent && h.rotate_in_symbol && !h.rotate_in_wall);
        assert_eq!(h.levels[0].start, [0.5, 0.25]);
        assert_eq!((h.levels[1].repeat, h.levels[1].offset, h.levels[1].dash, h.levels[1].color, h.levels[1].weight), ([3.0, 0.0], [0.0, 1.0], 0.6, 1237, 5));
    }

    #[test]
    fn dashes_run_on_around_corners() {
        // a square of 10 with dashes of 3 and gaps of 1: the pattern goes on past each corner
        let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let d = dash(&square, true, &[3.0, 1.0]).unwrap();
        assert_eq!(&d[..4], &[[[0.0, 0.0], [3.0, 0.0]], [[4.0, 0.0], [7.0, 0.0]], [[8.0, 0.0], [10.0, 0.0]], [[10.0, 0.0], [10.0, 1.0]]]);
        let drawn: f64 = d.iter().map(|[a, b]| dist(*a, *b)).sum();
        assert!((drawn - 30.0).abs() < 1e-9, "{drawn}");
    }

    #[test]
    fn hatch_lines_as_static_hatch_of_job_172() {
        // S-een: start (0, 0), repeat (2, 1), offset (0, 1.5), dash 0.5, at the origin, in a
        // rectangle of 200 x 100; Vectorworks' static hatch of it has 11 dashes, among them
        // (0, 0)-(25.4, 12.7), (0, 38.1)-(25.4, 50.8) and (101.6, 88.9)-(123.8, 100)
        let r = hatch_record(0x08, 256, &[([0.0, 0.0], [2.0, 1.0], [0.0, 1.5], 0.5, 257, 1)]);
        let h = hatch(&rec(&r)).unwrap();
        let rect = [[0.0, 0.0], [200.0, 0.0], [200.0, 100.0], [0.0, 100.0]];
        let lines = hatch_lines(&rect, &h, &Frame::hatch(None, false, 20.0), 1000).unwrap();
        assert_eq!(lines.len(), 11, "{lines:?}");
        let has = |a: P2, b: P2| lines.iter().any(|([p, q], _)| dist(*p, a) < 0.1 && dist(*q, b) < 0.1);
        assert!(has([0.0, 0.0], [25.4, 12.7]) && has([0.0, 38.1], [25.4, 50.8]) && has([101.6, 88.9], [123.8, 100.0]));
        // page based on a layer at 1 : 25 (job 175): an inch on paper is 635 mm
        let r = hatch_record(0x09, 256, &[([0.0, 0.0], [2.0, 1.0], [0.0, 1.5], 0.5, 257, 1)]);
        let h = hatch(&rec(&r)).unwrap();
        let rect = [[0.0, 0.0], [2000.0, 0.0], [2000.0, 2000.0], [0.0, 2000.0]];
        let lines = hatch_lines(&rect, &h, &Frame::hatch(None, true, 25.0), 1000).unwrap();
        let has = |a: P2, b: P2| lines.iter().any(|([p, q], _)| dist(*p, a) < 0.1 && dist(*q, b) < 0.1);
        assert!(has([0.0, 0.0], [635.0, 317.5]) && has([1270.0, 1587.5], [1905.0, 1905.0]) && has([0.0, 1905.0], [190.0, 2000.0]), "{lines:?}");
    }

    #[test]
    fn hatch_follows_its_fill_space_and_leaves_holes_out() {
        // horizontal solid lines an inch apart (25.4 mm at 1 : 1), in a square of 100 with a
        // hole of 40 reached along a side that is not drawn, as a polyline with a hole runs
        let r = hatch_record(0x00, 256, &[([0.0, 0.0], [1.0, 0.0], [0.0, 1.0], 1.0, 257, 1)]);
        let h = hatch(&rec(&r)).unwrap();
        let ring = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0], [0.0, 0.0], [30.0, 30.0], [30.0, 70.0], [70.0, 70.0], [70.0, 30.0], [30.0, 30.0]];
        let space = FillSpace { origin: [0.0, 10.0], i: [25.4, 10.0], j: [0.0, 35.4] };
        let lines = hatch_lines(&ring, &h, &Frame::hatch(Some(&space), false, 1.0), 1000).unwrap();
        // lines at y = 10, 35.4, 60.8, 86.2; the two through the hole in two pieces
        assert_eq!(lines.len(), 6, "{lines:?}");
        assert!(lines.iter().all(|([a, b], _)| (a[1] - b[1]).abs() < 1e-9 && !(a[1] > 30.0 && a[1] < 70.0 && a[0] < 70.0 && b[0] > 30.0)));
    }

    #[test]
    fn too_many_lines_are_not_drawn() {
        let r = hatch_record(0x00, 256, &[([0.0, 0.0], [1.0, 0.0], [0.0, 0.001], 1.0, 257, 1)]);
        let h = hatch(&rec(&r)).unwrap();
        let rect = [[0.0, 0.0], [1e5, 0.0], [1e5, 1e5], [0.0, 1e5]];
        assert!(hatch_lines(&rect, &h, &Frame::hatch(None, false, 1.0), MAX_FILL_SEGMENTS).is_none());
    }

    #[test]
    fn gradient_tile_and_fill_space_as_jobs_171_and_174() {
        let mut g = record(kinds::GRADIENT, 0x54 + 72, &[(0x54, 0.0), (0x5c, 0.5), (0x6c, 0.3), (0x74, 0.4), (0x84, 1.0), (0x8c, 0.5)]);
        g[0x52] = 3;
        for (k, c) in [[255u16, 0, 0], [0, 255, 0], [0, 0, 255]].iter().enumerate() {
            for (n, v) in c.iter().chain([&100u16]).enumerate() {
                g[0x54 + 24 * k + 16 + 2 * n..0x54 + 24 * k + 18 + 2 * n].copy_from_slice(&v.to_le_bytes());
            }
        }
        let stops = gradient(&rec(&g)).unwrap();
        assert_eq!((stops.len(), stops[1].position, stops[1].midpoint, stops[1].color, stops[1].opacity), (3, 0.3, 0.4, [0, 255, 0], 100));
        assert_eq!(gradient_color(&stops), Some([85, 85, 85]));
        let mut t = record(kinds::TILE, 122, &[(0x5a, 25.0), (0x62, 40.0), (0x6a, 4.0), (0x72, 3.0)]);
        t[0x58..0x5a].copy_from_slice(&1242u16.to_le_bytes());
        assert_eq!(tile(&rec(&t)), Some(Tile { repeat: [40.0, 25.0], offset: [3.0, 4.0], background: 1242 }));
        let s = record(kinds::FILL_SPACE, 136, &[(0x50, 250.0), (0x58, 500.0), (0x60, 250.0), (0x68, 1008.0), (0x70, 758.0), (0x78, 500.0)]);
        assert_eq!(fill_space(&rec(&s)), Some(FillSpace { origin: [500.0, 250.0], i: [1008.0, 250.0], j: [500.0, 758.0] }));
    }

    #[test]
    fn tiles_repeat_on_their_grid() {
        // a tile of one line from (0, 0) to (10, 0) every 20 along x and 20 along y, in a square
        // of 40: two tiles in each of two rows inside (and the rows at 0 and 40 on its border)
        let t = Tile { repeat: [20.0, 0.0], offset: [0.0, 20.0], background: 256 };
        let square = [[0.0, -1.0], [40.0, -1.0], [40.0, 39.0], [0.0, 39.0]];
        let frame = Frame { origin: [0.0; 2], x: [1.0, 0.0], y: [0.0, 1.0] };
        let lines = tile_lines(&square, &t, &[[[0.0, 0.0], [10.0, 0.0]]], &frame, 1000).unwrap();
        assert_eq!(lines.len(), 4, "{lines:?}");
    }

    #[test]
    fn image_as_its_png() {
        let mut r = record(kinds::IMAGE, 0xea + 8, &[]);
        r[0x7c] = 1;
        r[0x80] = 2;
        r[0x84] = 32;
        r[0x94] = 0x56;
        r[0x98..0x9c].copy_from_slice(b" gnp");
        r[0xc0] = 8;
        r[0xea..0xf2].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        let i = image(&rec(&r)).unwrap();
        assert_eq!((i.width, i.height, i.depth, i.format.as_str(), i.data), (1, 2, 32, "png", &b"\x89PNG\r\n\x1a\n"[..]));
    }
}
