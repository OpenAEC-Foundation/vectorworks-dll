//! The geometry in the payload of drawn objects (`docs/FORMAT.md` § Objects).
//!
//! Lengths are millimetres. 2D points are stored as (y, x) and returned here as [x, y]; 3D
//! points are stored and returned as (x, y, z).

use vwx_core::container::Record;
use vwx_core::le;

use crate::raw::kind;

pub type P2 = [f64; 2];
pub type P3 = [f64; 3];

/// Segments a full circle is drawn with.
const CIRCLE_SEGMENTS: usize = 48;
/// Segments a Bezier piece of a polyline is drawn with.
const BEZIER_SEGMENTS: usize = 12;

/// A placement: `world = x·rows[0] + y·rows[1] + z·rows[2] + offset`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub rows: [P3; 3],
    pub offset: P3,
}

impl Affine {
    pub const IDENTITY: Affine = Affine { rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], offset: [0.0; 3] };

    pub fn translation(offset: P3) -> Affine {
        Affine { offset, ..Affine::IDENTITY }
    }

    fn linear(&self, p: P3) -> P3 {
        std::array::from_fn(|k| p[0] * self.rows[0][k] + p[1] * self.rows[1][k] + p[2] * self.rows[2][k])
    }

    /// The direction `d` placed (without the offset).
    pub fn direction(&self, d: P3) -> P3 {
        self.linear(d)
    }

    pub fn apply(&self, p: P3) -> P3 {
        let q = self.linear(p);
        [q[0] + self.offset[0], q[1] + self.offset[1], q[2] + self.offset[2]]
    }

    /// This placement, then `outer`.
    pub fn then(&self, outer: &Affine) -> Affine {
        Affine { rows: self.rows.map(|r| outer.linear(r)), offset: outer.apply(self.offset) }
    }

    /// True when the placement mirrors (turns a counter-clockwise outline clockwise).
    pub fn mirrors(&self) -> bool {
        let [a, b, c] = self.rows;
        a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0]) < 0.0
    }
}

/// A finite `f64` at `at`.
fn real(r: &Record, at: usize) -> Option<f64> {
    le::f64(r.bytes, at).filter(|v| v.is_finite())
}

/// A 2D point stored as (y, x) at `at`.
fn yx(r: &Record, at: usize) -> Option<P2> {
    Some([real(r, at + 8)?, real(r, at)?])
}

fn xyz(r: &Record, at: usize) -> Option<P3> {
    Some([real(r, at)?, real(r, at + 8)?, real(r, at + 16)?])
}

/// Rows of a 3×3 matrix and the offset after it, at `at`.
fn placement(r: &Record, at: usize) -> Option<Affine> {
    Some(Affine { rows: [xyz(r, at)?, xyz(r, at + 24)?, xyz(r, at + 48)?], offset: xyz(r, at + 72)? })
}

/// Where a plug-in object (type 86: a door, a window, a column, a label) is put: what it holds
/// is drawn in its own coordinates, placed by the matrix at 0xf0 and the offset at 0x138. (Of
/// the 40 such objects with extrudes in `Residential New Construction.sta` the extrudes of 38
/// lie inside the box of their object once placed so, and those of 9 without.)
pub fn plug_in_placement(r: &Record) -> Option<Affine> {
    placement(r, 0xf0)
}

/// A viewport (type 122) as it lies on its sheet layer.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewport {
    /// The box it takes on the sheet: its lower left and upper right corner (millimetres).
    pub min: P2,
    pub max: P2,
    /// 1 : `scale`.
    pub scale: f64,
    /// Where the origin of the model lies on the sheet (millimetres).
    pub origin: P2,
    /// Right on the sheet, up on the sheet and towards the eye, as directions in the model.
    pub axes: [P3; 3],
}

/// A viewport (type 122): the box it takes on its sheet at 0x68 (top, left, bottom, right); a
/// placement at 0x8c whose offset at 0xd4 is where the origin of the model lies, in the
/// millimetres of the model; at 0x23e nine `f64`, where the x, y and z of the model point in
/// the view (x right, y up, z towards the eye); the scale as an `f64` at 0x2e4 (pair 200/201:
/// 50 and 100; views 203, 204; the eight viewports of `Residential New Construction.sta`, all
/// 1 : 50, whose two plans have the origin of the model at the same x on their sheet).
pub fn viewport(r: &Record) -> Option<Viewport> {
    let (top, left, bottom, right) = (real(r, 0x68)?, real(r, 0x70)?, real(r, 0x78)?, real(r, 0x80)?);
    // a viewport that was never drawn has a box without a size
    if [top, left, bottom, right].iter().any(|v| v.abs() > 1e7) || right <= left || top <= bottom {
        return None;
    }
    let scale = real(r, 0x2e4).filter(|s| *s > 0.0).unwrap_or(1.0);
    let at = xyz(r, 0xd4)?;
    let m = placement(r, 0x23e)?.rows;
    // the columns: what right, up and towards the eye are in the model
    let axes = [0, 1, 2].map(|k| [m[0][k], m[1][k], m[2][k]]);
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let square = (0..3).all(|i| (dot(axes[i], axes[i]) - 1.0).abs() < 1e-6 && dot(axes[i], axes[(i + 1) % 3]).abs() < 1e-6);
    square.then_some(Viewport { min: [left, bottom], max: [right, top], scale, origin: [at[0] / scale, at[1] / scale], axes })
}

/// The way a view looks at the model (type 43, a child of a saved view or of a layer): at 0x9c
/// nine `f64`, the directions in the document of right on the screen, up on the screen and
/// towards the eye (sample 140: a view set to 60 and 45 degrees). Returned: right, up, towards
/// the eye.
pub fn view_axes(r: &Record) -> Option<[P3; 3]> {
    let axes = placement(r, 0x9c)?.rows;
    // three directions square to each other, or it is not a view
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let square = (0..3).all(|i| (dot(axes[i], axes[i]) - 1.0).abs() < 1e-6 && dot(axes[i], axes[(i + 1) % 3]).abs() < 1e-6);
    square.then_some(axes)
}

/// The ends of a line (type 2): (y1, x1, y2, x2) at 0x68.
pub fn line(r: &Record) -> Option<[P2; 2]> {
    Some([yx(r, 0x68)?, yx(r, 0x78)?])
}

/// The corners of a rectangle or the frame of an oval (types 3 and 4): origin (y, x) at 0x8c,
/// direction (y, x) at 0x9c, width along the direction at 0xac, height to its left at 0xb4.
fn frame(r: &Record) -> Option<(P2, P2, f64, f64)> {
    Some((yx(r, 0x8c)?, yx(r, 0x9c)?, real(r, 0xac)?, real(r, 0xb4)?))
}

/// A vertex of a polyline: where it is, what kind it is (0 a corner, 1 the control point of a
/// Bezier curve, 3 a corner rounded by an arc; 2, a point a cubic curve goes through, is drawn
/// as a corner) and the radius of its arc (0: as large as fits).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub at: P2,
    pub kind: u8,
    pub radius: f64,
}

/// The vertices of a polyline (type 21): `u16` count at 0x8e, then 36 bytes each from 0x90:
/// the point (y, x), a `u32` whose second byte is the kind, the radius, 8 bytes.
pub fn polyline(r: &Record) -> Option<Vec<Vertex>> {
    let count = le::u16(r.bytes, 0x8e)? as usize;
    (0..count)
        .map(|k| {
            let at = 0x90 + 36 * k;
            Some(Vertex { at: yx(r, at)?, kind: *r.bytes.get(at + 17)?, radius: real(r, at + 20)? })
        })
        .collect()
}

/// The closed outline through the vertices of a polyline, curves stepped.
///
/// A curve at a vertex runs between its two neighbours: from and to a neighbour that is a
/// corner, and half way to a neighbour that is a curve itself. A Bezier vertex is the control
/// point of a quadratic curve between those two points; an arc vertex is rounded by a circle
/// that touches both sides (sample 113: Vectorworks' own points of such a profile).
pub fn polyline_outline(vertices: &[Vertex]) -> Vec<P2> {
    const BEZIER: u8 = 1;
    const ARC: u8 = 3;
    let n = vertices.len();
    let mut out: Vec<P2> = Vec::new();
    for i in 0..n {
        let (previous, v, next) = (vertices[(i + n - 1) % n], vertices[i], vertices[(i + 1) % n]);
        let towards = |other: Vertex| if matches!(other.kind, BEZIER | ARC) { [(v.at[0] + other.at[0]) / 2.0, (v.at[1] + other.at[1]) / 2.0] } else { other.at };
        let (a, b) = (towards(previous), towards(next));
        match v.kind {
            BEZIER => out.extend((0..=BEZIER_SEGMENTS).map(|k| {
                let t = k as f64 / BEZIER_SEGMENTS as f64;
                let (wa, wv, wb) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
                [wa * a[0] + wv * v.at[0] + wb * b[0], wa * a[1] + wv * v.at[1] + wb * b[1]]
            })),
            ARC => {
                let (da, db) = ([a[0] - v.at[0], a[1] - v.at[1]], [b[0] - v.at[0], b[1] - v.at[1]]);
                let (la, lb) = (da[0].hypot(da[1]), db[0].hypot(db[1]));
                let cos = if la > 0.0 && lb > 0.0 { ((da[0] * db[0] + da[1] * db[1]) / (la * lb)).clamp(-1.0, 1.0) } else { 1.0 };
                // half the angle between the two sides; no arc in a corner that is no corner
                let half = cos.acos() / 2.0;
                if half < 1e-6 || half > std::f64::consts::FRAC_PI_2 - 1e-6 {
                    out.push(v.at);
                    continue;
                }
                // the arc touches the sides at `reach` from the corner
                let fits = la.min(lb);
                let reach = if v.radius > 0.0 { (v.radius / half.tan()).min(fits) } else { fits };
                let (ua, ub) = ([da[0] / la, da[1] / la], [db[0] / lb, db[1] / lb]);
                let (from, to) = ([v.at[0] + ua[0] * reach, v.at[1] + ua[1] * reach], [v.at[0] + ub[0] * reach, v.at[1] + ub[1] * reach]);
                let bisector = [ua[0] + ub[0], ua[1] + ub[1]];
                let scale = reach / half.cos() / bisector[0].hypot(bisector[1]);
                let centre = [v.at[0] + bisector[0] * scale, v.at[1] + bisector[1] * scale];
                let (start, end) = ((from[1] - centre[1]).atan2(from[0] - centre[0]), (to[1] - centre[1]).atan2(to[0] - centre[0]));
                // the short way round
                let turn = (end - start + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
                let radius = reach * half.tan();
                let steps = ((turn.abs() / std::f64::consts::TAU * CIRCLE_SEGMENTS as f64).ceil() as usize).max(1);
                out.push(from);
                out.extend((1..steps).map(|k| {
                    let angle = start + turn * k as f64 / steps as f64;
                    [centre[0] + radius * angle.cos(), centre[1] + radius * angle.sin()]
                }));
                out.push(to);
            }
            _ => out.push(v.at),
        }
    }
    let near = |p: &P2, q: &P2| (p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9;
    out.dedup_by(|p, q| near(p, q));
    while out.len() > 1 && near(&out[0], &out[out.len() - 1]) {
        out.pop();
    }
    out
}

/// The closed outline of a 2D object, counter-clockwise or clockwise as drawn: a rectangle, an
/// oval, a polygon or polyline of three or more points, a full circle. `None` for anything else.
pub fn outline(r: &Record) -> Option<Vec<P2>> {
    match r.kind() {
        kind::POLYLINE => Some(polyline_outline(&polyline(r)?)).filter(|p| p.len() >= 3),
        kind::ROUNDED_RECTANGLE => {
            // a rectangle, and the width and height of the ellipse its corners are quarters
            // of at 0xbc and 0xc4
            let (o, d, w, h) = frame(r)?;
            let n = [-d[1], d[0]];
            let (rx, ry) = ((real(r, 0xbc)? / 2.0).clamp(0.0, w.abs() / 2.0), (real(r, 0xc4)? / 2.0).clamp(0.0, h.abs() / 2.0));
            let (su, sv) = (w.signum(), h.signum());
            let quarter = CIRCLE_SEGMENTS / 4;
            let mut out = Vec::with_capacity(4 * (quarter + 1));
            // the corners in turn, each from where the side before it ends
            for (k, (cu, cv)) in [(w - su * rx, sv * ry), (w - su * rx, h - sv * ry), (su * rx, h - sv * ry), (su * rx, sv * ry)].into_iter().enumerate() {
                for step in 0..=quarter {
                    let a = (k as f64 - 1.0 + step as f64 / quarter as f64) * std::f64::consts::FRAC_PI_2;
                    let (u, v) = (cu + su * rx * a.cos(), cv + sv * ry * a.sin());
                    out.push([o[0] + u * d[0] + v * n[0], o[1] + u * d[1] + v * n[1]]);
                }
            }
            out.dedup_by(|p, q| (p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9);
            Some(out).filter(|p| p.len() >= 3)
        }
        kind::RECTANGLE => {
            let (o, d, w, h) = frame(r)?;
            let n = [-d[1], d[0]];
            Some(vec![o, [o[0] + w * d[0], o[1] + w * d[1]], [o[0] + w * d[0] + h * n[0], o[1] + w * d[1] + h * n[1]], [o[0] + h * n[0], o[1] + h * n[1]]])
        }
        kind::OVAL => {
            let (o, d, w, h) = frame(r)?;
            let n = [-d[1], d[0]];
            let c = [o[0] + (w * d[0] + h * n[0]) / 2.0, o[1] + (w * d[1] + h * n[1]) / 2.0];
            Some(
                (0..CIRCLE_SEGMENTS)
                    .map(|k| {
                        let a = k as f64 / CIRCLE_SEGMENTS as f64 * std::f64::consts::TAU;
                        let (u, v) = (a.cos() * w / 2.0, a.sin() * h / 2.0);
                        [c[0] + u * d[0] + v * n[0], c[1] + u * d[1] + v * n[1]]
                    })
                    .collect(),
            )
        }
        kind::POLYGON => {
            // u16 count at 0x8e, points (y, x) from 0x90
            let count = le::u16(r.bytes, 0x8e)? as usize;
            let points: Option<Vec<P2>> = (0..count).map(|k| yx(r, 0x90 + 16 * k)).collect();
            points.filter(|p| p.len() >= 3)
        }
        kind::ARC => {
            // start point at 0x90, centre at 0xb0 (both y, x), sweep in degrees at 0xc8
            let (start, centre, sweep) = (yx(r, 0x90)?, yx(r, 0xb0)?, real(r, 0xc8)?);
            if sweep.abs() < 360.0 {
                return None;
            }
            let radius = ((start[0] - centre[0]).powi(2) + (start[1] - centre[1]).powi(2)).sqrt();
            Some(
                (0..CIRCLE_SEGMENTS)
                    .map(|k| {
                        let a = k as f64 / CIRCLE_SEGMENTS as f64 * std::f64::consts::TAU;
                        [centre[0] + radius * a.cos(), centre[1] + radius * a.sin()]
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// An extrude (type 24): its children are the profile, drawn in the local xy plane and extruded
/// from `bottom` to `top` along local z. Placement at 0xcc, bottom at 0x130, top at 0x138.
pub struct Extrude {
    pub placement: Affine,
    pub bottom: f64,
    pub top: f64,
}

pub fn extrude(r: &Record) -> Option<Extrude> {
    Some(Extrude { placement: placement(r, 0xcc)?, bottom: real(r, 0x130)?, top: real(r, 0x138)? })
}

/// A placed symbol (type 15): reference number of its definition at 0x8e, placement at 0x96,
/// scale factors along the local axes at 0x100.
pub struct Instance {
    pub definition: u32,
    pub placement: Affine,
}

pub fn instance(r: &Record) -> Option<Instance> {
    let mut placement = placement(r, 0x96)?;
    for (k, row) in placement.rows.iter_mut().enumerate() {
        // a scale of 0 is "not scaled" in files that do not use the field
        let s = real(r, 0x100 + 8 * k).filter(|s| *s != 0.0).unwrap_or(1.0);
        *row = row.map(|c| c * s);
    }
    Some(Instance { definition: le::u32(r.bytes, 0x8e)?, placement })
}

/// The points of a 3D polygon (type 25): `u16` count at 0xd0, points from 0xd2.
pub fn polygon_3d(r: &Record) -> Option<Vec<P3>> {
    let count = le::u16(r.bytes, 0xd0)? as usize;
    (0..count).map(|k| xyz(r, 0xd2 + 24 * k)).collect()
}

/// The points of a profile as Vectorworks worked them out (type 42, a child of the profile or of
/// a multiple extrude): `u16` points per ring at 0x6c, `u16` rings at 0x6e, `u16` total at 0x70,
/// then x, y, z from 0x72, for each point of the profile one per ring. Returned by ring.
///
/// An extrude has two rings (bottom and top), a multiple extrude one per profile; curves are
/// already stepped. For a sweep every ring is the profile where it was drawn.
pub fn grid(r: &Record) -> Option<Vec<Vec<P3>>> {
    let (points, rings, total) = (le::u16(r.bytes, 0x6c)? as usize, le::u16(r.bytes, 0x6e)? as usize, le::u16(r.bytes, 0x70)? as usize);
    if points == 0 || rings == 0 || points * rings != total {
        return None;
    }
    (0..rings).map(|ring| (0..points).map(|point| xyz(r, 0x72 + 24 * (point * rings + ring))).collect()).collect()
}

/// A sweep (type 34): its profile, drawn in the local xy plane, turned about the line through
/// the smallest x of the profile along local y. Placement at 0xcc; pitch (the rise along the
/// axis in one turn) at 0x140, step, start and arc in degrees at 0x148, 0x150, 0x158.
pub struct Sweep {
    pub placement: Affine,
    pub pitch: f64,
    pub step: f64,
    pub start: f64,
    pub arc: f64,
}

pub fn sweep(r: &Record) -> Option<Sweep> {
    Some(Sweep { placement: placement(r, 0xcc)?, pitch: real(r, 0x140)?, step: real(r, 0x148)?, start: real(r, 0x150)?, arc: real(r, 0x158)? })
}

impl Sweep {
    /// The rings the profile goes through, and whether the last ring is the first again (a full
    /// turn without pitch). At angle a the point (x, y) is at
    /// (axis + (x - axis) sin a, y + pitch (a - start) / 360, (x - axis) cos a).
    pub fn rings(&self, profile: &[P2]) -> (Vec<Vec<P3>>, bool) {
        let axis = profile.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        let closed = self.arc.abs() >= 360.0 - 1e-9 && self.pitch == 0.0;
        let step = if self.step.abs() < 0.5 { 15.0 } else { self.step.abs() };
        let steps = ((self.arc.abs() / step).ceil() as usize).clamp(1, 2880);
        let rings = (0..=steps)
            .take(steps + usize::from(!closed))
            .map(|k| {
                let turned = self.arc * k as f64 / steps as f64;
                let (sin, cos) = (self.start + turned).to_radians().sin_cos();
                profile.iter().map(|p| [axis + (p[0] - axis) * sin, p[1] + self.pitch * turned / 360.0, (p[0] - axis) * cos]).collect()
            })
            .collect();
        (rings, closed)
    }
}

/// The placement of a 3D object that has one at 0xcc (extrude, sweep, multiple extrude, CSG
/// solid), and its box in its own coordinates at 0x9c (largest then smallest x, y, z).
pub fn solid_frame(r: &Record) -> Option<(Affine, [P3; 2])> {
    Some((placement(r, 0xcc)?, [xyz(r, 0x9c + 24)?, xyz(r, 0x9c)?]))
}

/// Millimetres per unit of the Parasolid bodies in a file (sample 60: a wall of 5000 mm is 0.5).
pub const BODY_UNIT: f64 = 10_000.0;

/// The solid in a type 114 record: the triangles of its Parasolid body, in millimetres.
pub fn body(r: &Record) -> Option<vwx_xt::Mesh> {
    let at = *vwx_xt::find(r.bytes).first()?;
    let file = vwx_xt::parse(&r.bytes[at..]).ok()?;
    let mut mesh = vwx_xt::tessellate(&file);
    if mesh.triangles.is_empty() {
        return None;
    }
    for p in &mut mesh.positions {
        *p = p.map(|c| c * BODY_UNIT);
    }
    Some(mesh)
}

/// Most pieces of one boundary of a NURBS surface.
const MAX_PIECES: usize = 100_000;

/// A NURBS surface (type 113): at 0x98 six `u32`: the control points along u and along v, the
/// knots of u and of v, the degree along u and along v. From 0xb0 the control points, four
/// `f64` each (x·w, y·w, z·w, w), u running slowest; then the knots of u and those of v, all
/// written out. In millimetres, in the coordinates the object is placed in.
pub fn nurbs_surface(r: &Record) -> Option<vwx_xt::nurbs::NurbsSurface> {
    let n = |k: usize| le::u32(r.bytes, 0x98 + 4 * k).map(|v| v as usize);
    let (nu, nv, ku, kv, du, dv) = (n(0)?, n(1)?, n(2)?, n(3)?, n(4)?, n(5)?);
    let points = nu.checked_mul(nv)?;
    let knots_at = points.checked_mul(32)?.checked_add(0xb0)?;
    if knots_at.checked_add(ku.checked_add(kv)?.checked_mul(8)?)? > r.bytes.len() {
        return None;
    }
    let reals = |at: usize, count: usize| -> Option<Vec<f64>> { (0..count).map(|k| le::f64(r.bytes, at + 8 * k)).collect() };
    vwx_xt::nurbs::NurbsSurface::new([du, dv], [nu, nv], 4, reals(0xb0, points * 4)?, [reals(knots_at, ku)?, reals(knots_at + 8 * ku, kv)?], [false, false], false)
}

/// A boundary of a NURBS surface (type 111 as a child of the surface), in the parameters
/// (u, v) of the surface: a `u32` count of pieces at 0x8c, then from 0x90 piece after piece
/// around the boundary. A piece: `u32` counts of control points and of knots, `u32` degree and
/// a `u32`; its control points, four `f64` each (u·w, v·w, 0, w); its knots. Returned: the
/// ring of (u, v) the boundary is drawn with.
pub fn trim_loop(r: &Record) -> Option<Vec<P2>> {
    let pieces = le::u32(r.bytes, 0x8c)? as usize;
    let mut at = 0x90usize;
    let mut ring: Vec<P2> = Vec::new();
    for _ in 0..pieces.min(MAX_PIECES) {
        let count = |k: usize| le::u32(r.bytes, at + 4 * k).map(|v| v as usize);
        let (points, knots, degree) = (count(0)?, count(1)?, count(2)?);
        let knots_at = points.checked_mul(32)?.checked_add(at + 16)?;
        let end = knots.checked_mul(8)?.checked_add(knots_at)?;
        if end > r.bytes.len() {
            return None;
        }
        let reals = |at: usize, count: usize| -> Option<Vec<f64>> { (0..count).map(|k| le::f64(r.bytes, at + 8 * k)).collect() };
        let curve = vwx_xt::curve::Nurbs { degree, dim: 4, vertices: reals(at + 16, points * 4)?, knots: reals(knots_at, knots)?, rational: true };
        curve.range()?;
        for t in curve.parameters() {
            let p = curve.eval(t);
            let uv = [p[0], p[1]];
            if !(uv[0].is_finite() && uv[1].is_finite()) {
                return None;
            }
            if ring.last() != Some(&uv) {
                ring.push(uv);
            }
        }
        at = end;
    }
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    (ring.len() >= 3).then_some(ring)
}

/// The points of a mesh (type 40): `u32` count at 0xd0, then 28 bytes each from 0xd4 (x, y, z
/// and a `u32`).
pub fn mesh_points(r: &Record) -> Option<Vec<P3>> {
    let count = le::u32(r.bytes, 0xd0)? as usize;
    (0..count).map(|k| xyz(r, 0xd4 + 28 * k)).collect()
}

/// A face of a mesh (type 41, a child of the mesh): `u16` count at 0xcc, then the numbers of
/// its points from 0xce as `u32`, counted from 1. Returned counted from 0.
pub fn mesh_face(r: &Record) -> Option<Vec<usize>> {
    let count = le::u16(r.bytes, 0xcc)? as usize;
    (0..count).map(|k| (le::u32(r.bytes, 0xce + 4 * k)? as usize).checked_sub(1)).collect()
}

/// The mesh a primitive solid keeps (type 85, a child of the type 95 solid), in the coordinates
/// the solid is placed in: `u32` counts of points, corner numbers, faces and edges at 0x8c; the
/// points (x, y, z) from 0x9c; the corner numbers as `u32`, counted from 1; per face a `u32`
/// count of corners and the `u32` position of its first in that list; per edge 16 bytes.
/// Returned: the points and, per face, its corners counted from 0.
pub fn poly_mesh(r: &Record) -> Option<(Vec<P3>, Vec<Vec<usize>>)> {
    let count = |at: usize| le::u32(r.bytes, at).map(|n| n as usize);
    let (points, corners, faces) = (count(0x8c)?, count(0x90)?, count(0x94)?);
    let corners_at = 0x9cusize.checked_add(points.checked_mul(24)?)?;
    let faces_at = corners_at.checked_add(corners.checked_mul(4)?)?;
    if faces_at.checked_add(faces.checked_mul(8)?)? > r.bytes.len() {
        return None;
    }
    let positions: Option<Vec<P3>> = (0..points).map(|k| xyz(r, 0x9c + 24 * k)).collect();
    let face = |k: usize| -> Option<Vec<usize>> {
        let (n, first) = (count(faces_at + 8 * k)?, count(faces_at + 8 * k + 4)?);
        if first.checked_add(n)? > corners {
            return None;
        }
        (0..n).map(|c| count(corners_at + 4 * (first + c))?.checked_sub(1).filter(|p| *p < points)).collect()
    };
    Some((positions?, (0..faces).map(face).collect::<Option<_>>()?))
}

/// The palette index of the fill colour of a drawn object (`u16` at 0x58).
pub fn fill_index(r: &Record) -> Option<u16> {
    le::u16(r.bytes, 0x58)
}

/// True for a polygon or polyline (types 5 and 21) whose last point is joined to its first:
/// the byte at 0x8d (pair 190/191). Other shapes are closed by what they are.
pub fn closed(r: &Record) -> bool {
    !matches!(r.kind(), kind::POLYGON | kind::POLYLINE) || r.bytes.get(0x8d) != Some(&0)
}

/// A text (type 10) as it lies in the plane of its layer.
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub text: String,
    /// The box around it: its top left corner, its width and its height.
    pub at: P2,
    pub width: f64,
    pub height: f64,
    /// The size of its letters in points.
    pub size: f64,
    pub lines: u16,
}

/// A text (type 10): the box around it at 0x68 (top, left, bottom, right); a `u16` count of
/// characters at 0xa6; a `u16` count of lines at 0x100, two more `u16` and from 0x106 a `u16`
/// per line, the number of its first character; then the size of its letters in points as an
/// `f64`; its characters as UTF-16 at the end of the record, a carriage return between its
/// lines (samples 160 to 164).
pub fn text(r: &Record) -> Option<Text> {
    let (top, left, bottom, right) = (real(r, 0x68)?, real(r, 0x70)?, real(r, 0x78)?, real(r, 0x80)?);
    let count = le::u16(r.bytes, 0xa6)? as usize;
    let characters = r.bytes.get(r.bytes.len().checked_sub(2 * count)?..)?;
    let text = crate::raw::utf16(characters).replace('\r', "\n");
    let lines = le::u16(r.bytes, 0x100)?;
    Some(Text { text, at: [left, top], width: right - left, height: top - bottom, size: real(r, 0x106 + 2 * lines as usize)?, lines })
}

/// The palette index of the pen colour of a drawn object (`u16` at 0x5a).
pub fn pen_index(r: &Record) -> Option<u16> {
    le::u16(r.bytes, 0x5a)
}

/// True when a drawn object has a fill: the `u32` at 0x64 is 0 for none (sample 102), 1 for a
/// solid fill; other numbers are other fills.
pub fn filled(r: &Record) -> bool {
    le::u32(r.bytes, 0x64) != Some(0)
}

/// The points of an arc (type 6) that is not a full circle: start point at 0x90, centre at
/// 0xb0, an angle `a` at 0xc0 and the sweep at 0xc8, in degrees: the arc runs from
/// 90 - (a + sweep) to 90 - a, counter-clockwise from the x axis.
pub fn arc(r: &Record) -> Option<Vec<P2>> {
    let (start, centre, a, sweep) = (yx(r, 0x90)?, yx(r, 0xb0)?, real(r, 0xc0)?, real(r, 0xc8)?);
    let radius = (start[0] - centre[0]).hypot(start[1] - centre[1]);
    if radius <= 0.0 || sweep == 0.0 || sweep.abs() >= 360.0 {
        return None;
    }
    let (from, to) = ((90.0 - (a + sweep)).to_radians(), (90.0 - a).to_radians());
    let steps = (((to - from).abs() / std::f64::consts::TAU * CIRCLE_SEGMENTS as f64).ceil() as usize).max(1);
    Some(
        (0..=steps)
            .map(|k| {
                let t = from + (to - from) * k as f64 / steps as f64;
                [centre[0] + radius * t.cos(), centre[1] + radius * t.sin()]
            })
            .collect(),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use vwx_core::container::RECORD_HEADER;

    /// A record of `kind` of `len` bytes with `f64` values at the given offsets.
    pub(crate) fn record(kind: u8, len: usize, reals: &[(usize, f64)]) -> Vec<u8> {
        let mut r = vec![0u8; len.max(RECORD_HEADER)];
        let n = r.len() as u64;
        r[0..8].copy_from_slice(&n.to_le_bytes());
        r[0x19] = kind;
        for &(at, v) in reals {
            r[at..at + 8].copy_from_slice(&v.to_le_bytes());
        }
        r
    }

    fn rec(bytes: &[u8]) -> Record<'_> {
        Record { offset: 0, bytes }
    }

    #[test]
    fn polyline_as_sample_113_with_an_arc_and_a_bezier_vertex() {
        // corner, arc of radius 200, corner, Bezier control point, corner
        let vertices = [([0.0, 0.0], 0u8, 0.0), ([1000.0, 0.0], 3, 200.0), ([1000.0, 800.0], 0, 0.0), ([500.0, 1200.0], 1, 0.0), ([0.0, 800.0], 0, 0.0)];
        let mut b = record(kind::POLYLINE, 0x90 + 36 * vertices.len(), &[]);
        b[0x8e] = vertices.len() as u8;
        for (k, (at, kind, radius)) in vertices.iter().enumerate() {
            let o = 0x90 + 36 * k;
            b[o..o + 8].copy_from_slice(&f64::to_le_bytes(at[1]));
            b[o + 8..o + 16].copy_from_slice(&f64::to_le_bytes(at[0]));
            b[o + 16] = 0x80;
            b[o + 17] = *kind;
            b[o + 20..o + 28].copy_from_slice(&f64::to_le_bytes(*radius));
        }
        let o = outline(&rec(&b)).unwrap();
        let has = |x: f64, y: f64| o.iter().any(|p| (p[0] - x).abs() < 1e-6 && (p[1] - y).abs() < 1e-6);
        // the points Vectorworks has for it: the arc from (800, 0) to (1000, 200) about
        // (800, 200), the curve from (1000, 800) over (500, 1000) to (0, 800)
        assert!(has(0.0, 0.0) && has(800.0, 0.0) && has(1000.0, 200.0) && has(1000.0, 800.0) && has(500.0, 1000.0) && has(0.0, 800.0), "{o:?}");
        assert!(!has(1000.0, 0.0) && !has(500.0, 1200.0));
        let on_arc = o.iter().filter(|p| p[0] > 800.0 && p[1] < 200.0);
        assert!(on_arc.clone().count() >= 5 && on_arc.clone().all(|p| ((p[0] - 800.0).hypot(p[1] - 200.0) - 200.0).abs() < 1e-9));
        // an arc with radius 0 takes all the room there is: here the whole shorter side
        let all = polyline_outline(&[Vertex { at: [0.0, 0.0], kind: 0, radius: 0.0 }, Vertex { at: [1000.0, 0.0], kind: 3, radius: 0.0 }, Vertex { at: [1000.0, 800.0], kind: 0, radius: 0.0 }]);
        assert!(all.iter().any(|p| (p[0] - 200.0).abs() < 1e-6 && p[1].abs() < 1e-6) && all.iter().all(|p| (p[0] - 1000.0).abs() > 1e-6 || (p[1] - 800.0).abs() < 1e-6), "{all:?}");
    }

    #[test]
    fn arc_as_sample_16_and_fills() {
        // a quarter about the origin with radius 1000: a = 0, sweep 90: from 0 to 90 degrees
        let b = record(kind::ARC, 210, &[(0x98, 1000.0), (0xc8, 90.0)]);
        let points = arc(&rec(&b)).unwrap();
        let near = |p: P2, x: f64, y: f64| (p[0] - x).abs() < 1e-6 && (p[1] - y).abs() < 1e-6;
        assert!(near(points[0], 1000.0, 0.0) && near(points[points.len() - 1], 0.0, 1000.0), "{points:?}");
        assert!(points.len() >= 8 && points.iter().all(|p| (p[0].hypot(p[1]) - 1000.0).abs() < 1e-6));
        // a full circle is an outline, not an arc
        assert!(arc(&rec(&record(kind::ARC, 210, &[(0x98, 1000.0), (0xc8, 360.0)]))).is_none());
        let mut rect = record(kind::RECTANGLE, 188, &[]);
        assert!(!filled(&rec(&rect)));
        rect[0x64] = 1;
        assert!(filled(&rec(&rect)));
    }

    #[test]
    fn text_as_sample_161() {
        // "Hallo wereld" of 12 points with its top left corner at (1000, 500)
        let mut b = record(kind::TEXT, 0x19c, &[(0x68, 500.0), (0x70, 1000.0), (0x78, 402.63), (0x80, 1467.57), (0x108, 12.0)]);
        b[0xa6] = 12;
        b[0x100] = 1;
        b.extend("Hallo wereld".encode_utf16().flat_map(u16::to_le_bytes));
        let t = text(&rec(&b)).unwrap();
        assert_eq!((t.text.as_str(), t.at, t.size, t.lines), ("Hallo wereld", [1000.0, 500.0], 12.0, 1));
        assert!((t.width - 467.57).abs() < 1e-9 && (t.height - 97.37).abs() < 1e-9);
    }

    #[test]
    fn open_and_closed_polygons() {
        let mut b = record(kind::POLYGON, 208, &[]);
        assert!(!closed(&rec(&b)));
        b[0x8d] = 1;
        assert!(closed(&rec(&b)));
        assert!(closed(&rec(&record(kind::RECTANGLE, 188, &[]))));
    }

    #[test]
    fn viewport_as_the_front_elevation_of_the_corpus() {
        // on its sheet from (-477, -125) to (48, 106), 1 : 50, the model seen from the front:
        // its x to the right, its z up, its y away from the eye
        let reals = [(0x68, 106.0), (0x70, -477.0), (0x78, -125.0), (0x80, 48.0), (0xd4, -15846.1), (0xdc, -4853.2), (0x23e, 1.0), (0x266, -1.0), (0x276, 1.0), (0x2e4, 50.0)];
        let v = viewport(&rec(&record(kind::VIEWPORT, 1130, &reals))).unwrap();
        assert_eq!((v.min, v.max, v.scale), ([-477.0, -125.0], [48.0, 106.0], 50.0));
        assert!((v.origin[0] + 316.922).abs() < 1e-9 && (v.origin[1] + 97.064).abs() < 1e-9);
        assert_eq!(v.axes, [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]]);
        // one that was never drawn (made by a script) has no box
        let never = [(0x68, 6.7e9), (0x70, -6.7e9), (0x78, -6.7e9), (0x80, 6.7e9), (0x23e, 1.0), (0x25e, 1.0), (0x27e, 1.0)];
        assert!(viewport(&rec(&record(kind::VIEWPORT, 866, &never))).is_none());
    }

    #[test]
    fn rounded_rectangle_as_sample_220() {
        // 1000 x 500 with corners of diameter 200
        let b = record(kind::ROUNDED_RECTANGLE, 204, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0), (0xbc, 200.0), (0xc4, 200.0)]);
        let o = outline(&rec(&b)).unwrap();
        let has = |x: f64, y: f64| o.iter().any(|p| (p[0] - x).abs() < 1e-6 && (p[1] - y).abs() < 1e-6);
        // where Vectorworks' own points of it have the sides meet the corners
        assert!(has(900.0, 500.0) && has(1000.0, 400.0) && has(1000.0, 100.0) && has(900.0, 0.0) && has(100.0, 0.0) && has(0.0, 100.0), "{o:?}");
        assert!(!has(0.0, 0.0) && !has(1000.0, 500.0));
        assert!(o.iter().filter(|p| p[0] > 900.0 && p[1] > 400.0).all(|p| ((p[0] - 900.0).hypot(p[1] - 400.0) - 100.0).abs() < 1e-9));
        // without rounding it is the rectangle
        let plain = record(kind::ROUNDED_RECTANGLE, 204, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]);
        assert_eq!(outline(&rec(&plain)).unwrap().len(), 4);
    }

    #[test]
    fn rectangle_as_sample_12_rotated_30_degrees() {
        let (s, c) = 30f64.to_radians().sin_cos();
        let b = record(kind::RECTANGLE, 188, &[(0x9c, s), (0xa4, c), (0xac, 1000.0), (0xb4, 500.0)]);
        let o = outline(&rec(&b)).unwrap();
        let near = |p: P2, x: f64, y: f64| (p[0] - x).abs() < 1e-6 && (p[1] - y).abs() < 1e-6;
        assert!(near(o[0], 0.0, 0.0) && near(o[1], 866.0254037844386, 500.0), "{o:?}");
        assert!(near(o[2], 616.0254037844386, 933.0127018922193) && near(o[3], -250.0, 433.0127018922193), "{o:?}");
    }

    #[test]
    fn polygon_as_sample_13() {
        let pts = [(0.0, 0.0), (1000.0, 0.0), (1200.0, 600.0), (500.0, 900.0), (-200.0, 600.0)];
        let reals: Vec<(usize, f64)> = pts.iter().enumerate().flat_map(|(k, &(x, y))| [(0x90 + 16 * k, y), (0x98 + 16 * k, x)]).collect();
        let mut b = record(kind::POLYGON, 224, &reals);
        b[0x8e] = 5;
        assert_eq!(outline(&rec(&b)).unwrap(), pts.map(|(x, y)| [x, y]));
    }

    #[test]
    fn extrude_as_sample_23_rotated_about_z() {
        let (s, c) = 30f64.to_radians().sin_cos();
        let b = record(kind::EXTRUDE, 324, &[(0xcc, c), (0xd4, s), (0xe4, -s), (0xec, c), (0x10c, 1.0), (0x138, 3000.0)]);
        let e = extrude(&rec(&b)).unwrap();
        assert_eq!((e.bottom, e.top), (0.0, 3000.0));
        let p = e.placement.apply([1000.0, 0.0, 0.0]);
        assert!((p[0] - 866.0254037844386).abs() < 1e-9 && (p[1] - 500.0).abs() < 1e-9);
        assert!(!e.placement.mirrors());
    }

    #[test]
    fn mesh_as_sample_120() {
        let mut m = record(kind::MESH, 0xd4 + 2 * 28, &[(0xd4 + 16, 3000.0), (0xd4 + 28, 1000.0), (0xd4 + 28 + 16, 3000.0)]);
        m[0xd0] = 2;
        assert_eq!(mesh_points(&rec(&m)).unwrap(), [[0.0, 0.0, 3000.0], [1000.0, 0.0, 3000.0]]);
        let mut f = record(kind::MESH_FACE, 218, &[]);
        f[0xcc] = 3;
        for (k, n) in [2u32, 4, 3].iter().enumerate() {
            f[0xce + 4 * k..0xd2 + 4 * k].copy_from_slice(&n.to_le_bytes());
        }
        assert_eq!(mesh_face(&rec(&f)).unwrap(), [1, 3, 2]);
        // a point number of 0 is not a point
        f[0xce] = 0;
        assert!(mesh_face(&rec(&f)).is_none());
    }

    #[test]
    fn grid_as_sample_28_two_rings_of_four() {
        // (250, 750, 2000) on the first ring, (0, 1000, 0) on the second, for the first point
        let mut b = record(42, 0x72 + 8 * 24, &[]);
        b[0x6c] = 4;
        b[0x6e] = 2;
        b[0x70] = 8;
        for (k, v) in [250.0f64, 750.0, 2000.0, 0.0, 1000.0, 0.0].iter().enumerate() {
            b[0x72 + 8 * k..0x7a + 8 * k].copy_from_slice(&v.to_le_bytes());
        }
        let g = grid(&rec(&b)).unwrap();
        assert_eq!((g.len(), g[0].len()), (2, 4));
        assert_eq!((g[0][0], g[1][0]), ([250.0, 750.0, 2000.0], [0.0, 1000.0, 0.0]));
        // counts that do not multiply out are not a grid
        b[0x70] = 7;
        assert!(grid(&rec(&b)).is_none());
    }

    #[test]
    fn sweep_as_samples_200_201_203() {
        let profile = [[500.0, 0.0], [1000.0, 0.0], [1000.0, 300.0], [500.0, 300.0]];
        let bounds = |rings: &[Vec<P3>]| {
            let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            for p in rings.iter().flatten() {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            (lo.map(|v| (v * 10.0).round() / 10.0), hi.map(|v| (v * 10.0).round() / 10.0))
        };
        let sweep = |start, arc, step, pitch| Sweep { placement: Affine::IDENTITY, pitch, step, start, arc };
        // the bounds of Vectorworks' STL export of the samples
        let (quarter, closed) = sweep(0.0, 90.0, 15.0, 0.0).rings(&profile);
        assert_eq!((quarter.len(), closed), (7, false));
        assert_eq!(bounds(&quarter), ([500.0, 0.0, 0.0], [1000.0, 300.0, 500.0]));
        let (partial, _) = sweep(30.0, 200.0, 10.0, 0.0).rings(&profile);
        assert_eq!(bounds(&partial), ([117.0, 0.0, -500.0], [1000.0, 300.0, 433.0]));
        let (pitched, closed) = sweep(0.0, 720.0, 15.0, 400.0).rings(&profile);
        assert_eq!((bounds(&pitched), closed), (([0.0, 0.0, -500.0], [1000.0, 1100.0, 500.0]), false));
        let (full, closed) = sweep(0.0, 360.0, 15.0, 0.0).rings(&profile);
        assert_eq!((full.len(), closed), (24, true));
    }

    #[test]
    fn poly_mesh_of_one_triangle_and_one_quad() {
        let points: [P3; 4] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
        let mut b = record(85, 0x9c, &[]);
        for (at, n) in [(0x8c, 4u32), (0x90, 7), (0x94, 2), (0x98, 0)] {
            b[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        b.extend(points.iter().flatten().flat_map(|c| c.to_le_bytes()));
        b.extend([1u32, 2, 3, 1, 2, 3, 4].iter().flat_map(|c| c.to_le_bytes()));
        b.extend([3u32, 0, 4, 3].iter().flat_map(|c| c.to_le_bytes()));
        let (positions, faces) = poly_mesh(&rec(&b)).unwrap();
        assert_eq!(positions, points);
        assert_eq!(faces, [vec![0, 1, 2], vec![0, 1, 2, 3]]);
        // a corner number past the points is not a mesh
        let at = 0x9c + 4 * 24;
        b[at..at + 4].copy_from_slice(&9u32.to_le_bytes());
        assert!(poly_mesh(&rec(&b)).is_none());
    }

    #[test]
    fn placements_compose() {
        let turn = Affine { rows: [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]], offset: [0.0; 3] };
        let both = turn.then(&Affine::translation([10.0, 0.0, 5.0]));
        assert_eq!(both.apply([1.0, 0.0, 0.0]), [10.0, 1.0, 5.0]);
    }
}
