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

/// The ends of a line (type 2): (y1, x1, y2, x2) at 0x68.
pub fn line(r: &Record) -> Option<[P2; 2]> {
    Some([yx(r, 0x68)?, yx(r, 0x78)?])
}

/// The corners of a rectangle or the frame of an oval (types 3 and 4): origin (y, x) at 0x8c,
/// direction (y, x) at 0x9c, width along the direction at 0xac, height to its left at 0xb4.
fn frame(r: &Record) -> Option<(P2, P2, f64, f64)> {
    Some((yx(r, 0x8c)?, yx(r, 0x9c)?, real(r, 0xac)?, real(r, 0xb4)?))
}

/// The closed outline of a 2D object, counter-clockwise or clockwise as drawn: a rectangle, an
/// oval, a polygon of three or more points, a full circle. `None` for anything else.
pub fn outline(r: &Record) -> Option<Vec<P2>> {
    match r.kind() {
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
