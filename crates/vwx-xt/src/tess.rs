//! Triangles of the faces of XT bodies.
//!
//! A face is part of a surface bounded by loops. The loops are followed fin by fin into rings of
//! points ([`crate::curve`]). Then:
//!
//! - a **plane** face is triangulated in its plane, the other loops as holes;
//! - a face of a **cylinder, cone, sphere or torus** is triangulated in the parameters (u, v) of
//!   its surface, where `u` goes around the axis. A loop that goes once around the axis does not
//!   close in (u, v); two such loops bound a band, one of them a cap up to the pole or apex.
//!   The triangulation is a constrained Delaunay one, so the boundary stays as the curves gave
//!   it and neighbouring faces share their points;
//! - a face of a **B-surface**, a **swept** or a **spun surface** likewise, with its parameters
//!   searched from point to point along each loop;
//! - a face of a **blend** (a rolling ball along an edge) likewise, by its spine and the two
//!   surfaces the ball touches;
//! - a face of an **offset surface** likewise: its surface is another, moved along its normal;
//! - a face whose loops cannot be laid out in the parameters of its surface: its loops are filled
//!   as if they lay in one plane, and it is counted as approximate, with the reason.
//!
//! The loop direction has the face on its left seen from outside, so the outer loop of a face
//! runs counter-clockwise about the face normal.

use std::f64::consts::TAU;

use crate::curve;
use crate::geom::{add, cross, dot, len, mul, sub, unit, P2, P3};
use crate::parse::{File, Node};
use crate::surface::{face_sign, Surface};
use crate::{code, surface};

/// Most fins followed around one loop, and most triangles made for one face.
const MAX_FINS: usize = 200_000;
const MAX_FACE_TRIANGLES: usize = 200_000;

/// The triangles of a body, in the units of the XT data.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub positions: Vec<P3>,
    /// Unit normals, one per position, pointing out of the body.
    pub normals: Vec<P3>,
    pub triangles: Vec<[u32; 3]>,
    /// Faces in the data.
    pub faces: usize,
    /// Faces on surfaces that are not evaluated: filled flat between their loops.
    pub approximate: usize,
    /// Faces that gave no triangles.
    pub skipped: usize,
    /// The node type of the curve of each edge use that was drawn straight because curves of
    /// that kind are not followed (0: the edge has no curve).
    pub straight_edges: Vec<u16>,
    /// The node type of the surface of each face counted in `approximate` (0: no surface), and
    /// why it was not triangulated on its surface.
    pub approximate_surfaces: Vec<(u16, &'static str)>,
    /// Per face: its first triangle and the node type of its surface (0: none).
    pub face_starts: Vec<(usize, u16)>,
    /// The faces counted in `approximate` or `skipped`: the index of the face node and why.
    pub failures: Vec<(u32, &'static str)>,
}

/// The triangles of every face in `file`.
pub fn tessellate(file: &File) -> Mesh {
    let mut mesh = Mesh::default();
    for (index, face) in file.of_type(code::FACE) {
        mesh.faces += 1;
        let before = mesh.triangles.len();
        mesh.face_starts.push((before, file.node(face.ptr("surface")).map_or(0, Node::code)));
        let loops = face_loops(file, face, &mut mesh.straight_edges);
        match Surface::load(file, face.ptr("surface")) {
            Some(s) if s.kind == surface::Kind::Plane => {
                let normal = mul(s.frame.z, face_sign(face, &s));
                flat(&mut mesh, &loops, Some(normal));
            }
            Some(s) => {
                if let Err(why) = curved(&mut mesh, &loops, &s, face_sign(face, &s)) {
                    approximate(&mut mesh, file, face, &loops, why);
                    mesh.failures.push((index, why));
                }
            }
            None => {
                approximate(&mut mesh, file, face, &loops, "the surface is not evaluated");
                mesh.failures.push((index, "the surface is not evaluated"));
            }
        }
        if mesh.triangles.len() == before {
            mesh.skipped += 1;
            mesh.failures.push((index, "no triangles"));
        }
    }
    mesh
}

/// Triangles of the part of a B-surface inside rings given in its parameters (u, v), counted
/// even-odd: an outline and its holes, in any order. Without rings, or with rings that cannot
/// be used, the whole surface. The normals are the natural normals.
pub fn trimmed(surface: &std::rc::Rc<crate::nurbs::NurbsSurface>, loops: &[Vec<P2>]) -> Mesh {
    let s = Surface::of(surface.clone());
    let step = s.step();
    let mut rings = Vec::new();
    for uv in loops {
        // a ring has three points or more and goes around something
        if uv.len() < 3 || area2(uv) == 0.0 {
            continue;
        }
        let mut ring = Ring::default();
        for (i, a) in uv.iter().enumerate() {
            let b = uv[(i + 1) % uv.len()];
            // an edge that is straight in (u, v) bends with the surface
            let wide = ((b[0] - a[0]) / step[0]).abs().max(((b[1] - a[1]) / step[1]).abs());
            let parts = if wide.is_finite() { (wide.ceil() as usize).clamp(1, 256) } else { 1 };
            for k in 0..parts {
                let t = k as f64 / parts as f64;
                ring.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], None);
            }
        }
        ring.corners.dedup_by(|a, b| a.uv == b.uv);
        if ring.corners.len() >= 3 {
            rings.push(ring);
        }
    }
    let mut mesh = Mesh::default();
    if rings.is_empty() || !triangulate(&mut mesh, &rings, &s, 1.0, false) {
        mesh = Mesh::default();
        let _ = curved(&mut mesh, &[], &s, 1.0);
    }
    mesh.faces = 1;
    mesh
}

/// A description of face node `face` for finding out why it is not triangulated exactly: its
/// surface, and per loop its fins and its points in the parameters of the surface (all of them
/// with `points`).
#[doc(hidden)]
pub fn explain(file: &File, face: u32, points: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let Some(f) = file.node(face) else { return out };
    let surface = file.node(f.ptr("surface"));
    let _ = writeln!(out, "face {face} sense {} surface {} {:?}", if f.positive() { '+' } else { '-' }, f.ptr("surface"), surface.map(|n| (n.def.name.clone(), n.values.clone())));
    let s = Surface::load(file, f.ptr("surface"));
    if let Some(s) = &s {
        let _ = writeln!(out, "  kind {:?} positive {} offset {} sign {} period {:?} {:?}", s.kind, s.positive, s.offset, face_sign(f, s), s.period_u(), s.period_v());
    }
    let mut l = f.ptr("loop");
    let mut straight = Vec::new();
    while let Some(lp) = file.node(l) {
        let _ = writeln!(out, " loop {l}");
        let first = lp.ptr("fin");
        let mut fi = first;
        for _ in 0..10_000 {
            let Some(fin) = file.node(fi) else { break };
            let edge = file.node(fin.ptr("edge"));
            let curve = edge.map_or(0, |e| e.ptr("curve"));
            let curve = if curve == 0 { fin.ptr("curve") } else { curve };
            let (shape, _) = curve::shape(file, curve);
            let other = file.node(fin.ptr("other")).and_then(|o| file.node(o.ptr("loop"))).map_or(0, |lp| lp.ptr("face"));
            let _ = writeln!(
                out,
                "  fin {fi} sense {} edge {} curve {curve} ({}) vertex {} {:?} other face {other}",
                if fin.positive() { '+' } else { '-' },
                fin.ptr("edge"),
                crate::schema::base(shape).map_or("none", |b| b.0),
                fin.ptr("vertex"),
                vertex_point(file, fin.ptr("vertex")),
            );
            fi = fin.ptr("forward");
            if fi == first || fi == 0 {
                break;
            }
        }
        if let (Some(s), Some(ring)) = (&s, loop_points(file, lp, &mut straight).and_then(clean)) {
            let (uv, tu, tv) = unwrap(s, &ring);
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for p in &uv {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            let _ = writeln!(out, "  {} points, turns {tu} {tv}, u {:.6}..{:.6} v {:.6}..{:.6}, area {:.6}", ring.len(), lo[0], hi[0], lo[1], hi[1], area2(&uv) / 2.0);
            if points {
                for (p, q) in ring.iter().zip(&uv) {
                    let miss = crate::geom::dist(s.eval(q[0], q[1]), *p);
                    let _ = writeln!(out, "    {:.6} {:.6} {:.6}  uv {:.6} {:.6}  miss {:.2e}", p[0], p[1], p[2], q[0], q[1], miss);
                }
            }
        }
        l = lp.ptr("next");
    }
    out
}

/// Fills a face flat between its loops, for a surface that is not evaluated.
fn approximate(mesh: &mut Mesh, file: &File, face: &Node, loops: &[Vec<P3>], why: &'static str) {
    mesh.approximate += 1;
    mesh.approximate_surfaces.push((file.node(face.ptr("surface")).map_or(0, Node::code), why));
    flat(mesh, loops, None);
}

fn vertex_point(file: &File, vertex: u32) -> Option<P3> {
    file.node(file.node(vertex)?.ptr("point"))?.vector("pvec")
}

/// The loops of a face as rings of points (the last point is not the first again).
fn face_loops(file: &File, face: &Node, straight: &mut Vec<u16>) -> Vec<Vec<P3>> {
    let mut out = Vec::new();
    let mut l = face.ptr("loop");
    let mut seen = 0;
    while let Some(lp) = file.node(l) {
        if let Some(ring) = loop_points(file, lp, straight).and_then(clean) {
            out.push(ring);
        }
        l = lp.ptr("next");
        seen += 1;
        if seen > MAX_FINS {
            break;
        }
    }
    out
}

/// A ring fit to triangulate: every coordinate a number of ordinary size, no point twice in a
/// row, three points or more. Damaged data gives none.
fn clean(mut ring: Vec<P3>) -> Option<Vec<P3>> {
    // the size box of a Parasolid body is 1000 units; anything far outside is not a coordinate
    if ring.iter().flatten().any(|c| !c.is_finite() || c.abs() > 1e9) {
        return None;
    }
    ring.dedup();
    while ring.len() > 1 && ring[0] == ring[ring.len() - 1] {
        ring.pop();
    }
    (ring.len() >= 3).then_some(ring)
}

fn loop_points(file: &File, lp: &Node, straight: &mut Vec<u16>) -> Option<Vec<P3>> {
    let first = lp.ptr("fin");
    let mut ring = Vec::new();
    let mut f = first;
    for _ in 0..MAX_FINS {
        let fin = file.node(f)?;
        // a loop that is one vertex has a fin without an edge
        let edge = file.node(fin.ptr("edge"))?;
        // the curve of the edge runs along the edge, which the fin follows or not. A tolerant
        // edge has none: each of its fins has a curve of its own (a trimmed SP-curve in the
        // surface of its face), and that runs along the fin, whatever the fin's sense (the
        // ends of every such curve of three corpus files: 12 691 along the fin, none against)
        let (curve, forward) = match edge.ptr("curve") {
            0 => (fin.ptr("curve"), true),
            c => (c, fin.positive()),
        };
        let (shape, followed) = curve::shape(file, curve);
        if !followed {
            straight.push(shape);
        }
        let end = vertex_point(file, fin.ptr("vertex"));
        let start = file.node(fin.ptr("backward")).and_then(|b| vertex_point(file, b.ptr("vertex")));
        match end {
            Some(end) => {
                ring.extend(curve::between(file, curve, start.or(Some(end)), Some(end), forward));
                ring.push(end);
            }
            // a ring edge: the whole closed curve
            None => ring.extend(curve::between(file, curve, None, None, forward)),
        }
        f = fin.ptr("forward");
        if f == first || f == 0 {
            break;
        }
    }
    Some(ring)
}

/// The normal of a ring of points (Newell), not normalised: its length is twice the area.
fn newell(p: &[P3]) -> P3 {
    let mut n = [0.0; 3];
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

fn area2(p: &[P2]) -> f64 {
    (0..p.len()).map(|i| p[i][0] * p[(i + 1) % p.len()][1] - p[(i + 1) % p.len()][0] * p[i][1]).sum()
}

/// Fills loops that lie in one plane: the largest is the outside, the others are holes.
/// `normal` is the face normal; without it the direction of the outer loop gives it.
fn flat(mesh: &mut Mesh, loops: &[Vec<P3>], normal: Option<P3>) {
    let Some(outer) = (0..loops.len()).max_by(|&a, &b| len(newell(&loops[a])).total_cmp(&len(newell(&loops[b])))) else {
        return;
    };
    let around = newell(&loops[outer]);
    let normal = unit(normal.unwrap_or(around));
    if len(normal) == 0.0 {
        return;
    }
    // axes in the plane
    let helper = if normal[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let ex = unit(cross(helper, normal));
    let ey = cross(normal, ex);
    let mut order = vec![outer];
    order.extend((0..loops.len()).filter(|k| *k != outer));
    let mut coords = Vec::new();
    let mut holes = Vec::new();
    let mut points = Vec::new();
    for (n, &k) in order.iter().enumerate() {
        if n > 0 {
            holes.push(points.len());
        }
        for p in &loops[k] {
            coords.extend([dot(*p, ex), dot(*p, ey)]);
            points.push(*p);
        }
    }
    if coords.iter().any(|c: &f64| !c.is_finite()) {
        return;
    }
    let Ok(indices) = earcutr::earcut(&coords, &holes, 2) else {
        return;
    };
    let base = mesh.positions.len() as u32;
    mesh.positions.extend(&points);
    mesh.normals.extend(std::iter::repeat_n(normal, points.len()));
    for t in indices.as_chunks::<3>().0 {
        let at = |i: usize| [coords[2 * t[i]], coords[2 * t[i] + 1]];
        let turn = area2(&[at(0), at(1), at(2)]);
        if turn == 0.0 {
            continue;
        }
        let t = if turn > 0.0 { [t[0], t[1], t[2]] } else { [t[0], t[2], t[1]] };
        mesh.triangles.push(t.map(|i| base + i as u32));
    }
}

/// A point of a face in parameter space: (u, v), and the point itself when it is on the boundary.
#[derive(Debug, Clone, Copy)]
struct Corner {
    uv: P2,
    exact: Option<P3>,
}

/// A ring in parameter space.
#[derive(Debug, Clone, Default)]
struct Ring {
    corners: Vec<Corner>,
}

impl Ring {
    fn push(&mut self, uv: P2, exact: Option<P3>) {
        self.corners.push(Corner { uv, exact });
    }

    fn mid_u(&self) -> f64 {
        let (lo, hi) = self.corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(c.uv[0]), hi.max(c.uv[0])));
        (lo + hi) / 2.0
    }

    fn shift_u(&mut self, by: f64) {
        self.corners.iter_mut().for_each(|c| c.uv[0] += by);
    }
}

fn wrap(delta: f64, period: f64) -> f64 {
    delta - period * (delta / period).round()
}

/// The parameters of the points of a loop on `s`. On a B-surface a point is searched from its
/// neighbour, which keeps a loop on one side of a seam; a search that ends away from the point
/// is done again over the whole surface; and a second pass, backwards, moves points to the side
/// their other neighbour is on (the first points of a loop that starts on a seam).
fn follow(s: &Surface, points: &[P3]) -> Vec<P2> {
    if !s.searched() {
        return points.iter().map(|p| s.invert(*p)).collect();
    }
    let miss = |p: P3, uv: P2| crate::geom::dist(s.eval(uv[0], uv[1]), p);
    let size = points.iter().map(|p| crate::geom::dist(*p, points[0])).fold(0.0, f64::max);
    let tolerance = 1e-6 * size.max(1e-9);
    let mut uv: Vec<P2> = Vec::with_capacity(points.len());
    for p in points {
        let mut found = match uv.last() {
            Some(last) => s.invert_near(*p, *last),
            None => s.invert(*p),
        };
        if miss(*p, found) > tolerance {
            let again = s.invert(*p);
            if miss(*p, again) < miss(*p, found) {
                found = again;
            }
        }
        uv.push(found);
    }
    let n = uv.len();
    let step = s.step();
    let apart = |a: P2, b: P2| ((a[0] - b[0]) / step[0]).abs() + ((a[1] - b[1]) / step[1]).abs();
    for i in (0..n).rev() {
        let (next, previous) = (uv[(i + 1) % n], uv[(i + n - 1) % n]);
        let other = s.invert_near(points[i], next);
        if miss(points[i], other) <= miss(points[i], uv[i]) + tolerance && apart(other, next) + apart(other, previous) < apart(uv[i], next) + apart(uv[i], previous) {
            uv[i] = other;
        }
    }
    uv
}

/// A loop in the parameters of `s`, `u` (and `v` on a torus) continuous along it, and how many
/// times it goes around in `u` and in `v`.
fn unwrap(s: &Surface, points: &[P3]) -> (Vec<P2>, i32, i32) {
    let uv = follow(s, points);
    let mut uv = uv;
    let singular: Vec<bool> = points.iter().map(|p| s.singular(*p)).collect();
    // at a pole or apex the angle about the axis is that of a neighbour
    let angular = s.angular();
    if let Some(first) = singular.iter().position(|x| !x) {
        let n = uv.len();
        let mut last = uv[first][angular];
        for k in 0..n {
            let i = (first + k) % n;
            if singular[i] {
                uv[i][angular] = last;
            } else {
                last = uv[i][angular];
            }
        }
    }
    let mut turns = [0i32; 2];
    for (axis, period) in [(0, s.period_u()), (1, s.period_v())] {
        let Some(period) = period else { continue };
        for i in 1..uv.len() {
            uv[i][axis] = uv[i - 1][axis] + wrap(uv[i][axis] - uv[i - 1][axis], period);
        }
        let n = uv.len();
        let closing = uv[n - 1][axis] + wrap(uv[0][axis] - uv[n - 1][axis], period);
        turns[axis] = ((closing - uv[0][axis]) / period).round() as i32;
    }
    (uv, turns[0], turns[1])
}

/// Triangles of a face of a curved surface. An error says why the face is of a shape this does
/// not handle (the caller then fills it flat).
///
/// Loops may go around in u (a cylinder) or in v (the tube of a torus), not both: the work is
/// done in (a, b), which is (u, v) or, when loops go around in v, (v, u).
fn curved(mesh: &mut Mesh, loops: &[Vec<P3>], s: &Surface, sign: f64) -> Result<(), &'static str> {
    let unwrapped: Vec<(Vec<P2>, i32, i32)> = loops.iter().map(|points| unwrap(s, points)).collect();
    let (in_u, in_v) = (unwrapped.iter().any(|l| l.1 != 0), unwrapped.iter().any(|l| l.2 != 0));
    if in_u && in_v {
        return Err("loops go around in u and in v");
    }
    let swap = in_v;
    let period = if swap { s.period_v() } else { s.period_u() }.unwrap_or(TAU);
    let mut plain: Vec<Ring> = Vec::new();
    let mut around: Vec<(Ring, i32)> = Vec::new();
    for (points, (uv, turns_u, turns_v)) in loops.iter().zip(&unwrapped) {
        let turns = if swap { *turns_v } else { *turns_u };
        if turns.abs() > 1 {
            return Err("a loop goes around more than once");
        }
        let mut ring = Ring::default();
        for (p, uv) in points.iter().zip(uv) {
            ring.push(if swap { [uv[1], uv[0]] } else { *uv }, Some(*p));
        }
        match turns {
            0 => plain.push(ring),
            t => around.push((ring, t)),
        }
    }
    // Which way a blend faces follows from its loops, not from its senses (the reference says
    // how its parameters run, but not so that every blend of the corpus comes out the right
    // way): the face is left of its loops, so its outer loop runs counter-clockwise in (u, v)
    // when the face normal is the natural normal; and of two loops around, the one that runs
    // towards larger u has the face at larger v.
    let sign = if s.kind == surface::Kind::Blend {
        let mean_b = |r: &Ring| r.corners.iter().map(|c| c.uv[1]).sum::<f64>() / r.corners.len().max(1) as f64;
        let derived = match around.as_slice() {
            [] => plain.iter().map(|r| area2(&r.corners.iter().map(|c| c.uv).collect::<Vec<_>>())).max_by(|a, b| a.abs().total_cmp(&b.abs())).filter(|a| *a != 0.0).map(f64::signum),
            [(a, ta), (b, _)] => Some(if (mean_b(a) < mean_b(b)) == (*ta > 0) { 1.0 } else { -1.0 }),
            _ => None,
        };
        // (in (v, u) the turn is the other way round)
        derived.map_or(sign, |d| if swap { -d } else { d })
    } else {
        sign
    };
    // the outside ring
    let outer = match around.len() {
        0 if !plain.is_empty() => {
            let scale = s.scale(plain[0].corners[0].uv[usize::from(!swap)]);
            let scale = if swap { [scale[1], scale[0]] } else { scale };
            let size = |r: &Ring| area2(&r.corners.iter().map(|c| [c.uv[0] * scale[0], c.uv[1] * scale[1]]).collect::<Vec<_>>()).abs();
            let k = (0..plain.len()).max_by(|&a, &b| size(&plain[a]).total_cmp(&size(&plain[b]))).unwrap();
            plain.swap_remove(k)
        }
        // no boundary at all: the whole of a closed surface
        0 => match whole(s) {
            Some(ring) => ring,
            None => return Err("no loops on an open surface"),
        },
        1 => {
            let (ring, turns) = around.pop().unwrap();
            // the face is left of the loop seen from outside: towards larger v when the loop
            // runs towards larger u and the face normal is the natural normal; and towards
            // smaller u when it runs towards larger v
            let up = ((turns > 0) == (sign > 0.0)) != swap;
            match cap(s, ring, turns, up, period, swap) {
                Some(ring) => ring,
                None => return Err("one loop around the axis and no pole on that side"),
            }
        }
        2 => {
            let (b, tb) = around.pop().unwrap();
            let (a, ta) = around.pop().unwrap();
            if ta + tb != 0 {
                return Err("two loops around the axis the same way");
            }
            band(s, a, ta, b, tb, period)
        }
        _ => return Err("more than two loops around the axis"),
    };
    let mut rings = vec![outer];
    let middle = rings[0].mid_u();
    for mut hole in plain {
        hole.shift_u(period * ((middle - hole.mid_u()) / period).round());
        rings.push(hole);
    }
    if triangulate(mesh, &rings, s, sign, swap) {
        Ok(())
    } else {
        Err("the loops cross in parameter space")
    }
}

/// Closes a loop that goes around the axis: back to the first point a period on, then along the
/// pole or apex.
fn close_around(ring: &mut Ring, turns: i32, period: f64) {
    let first = ring.corners[0];
    // the last boundary edge ends at the first point, one period on
    ring.push([first.uv[0] + turns as f64 * period, first.uv[1]], first.exact);
}

/// A loop around the axis made into a ring by going back along the pole or apex on its `up`
/// side. The ring is in (u, v), or with `swap` in (v, u).
fn cap(s: &Surface, mut ring: Ring, turns: i32, up: bool, period: f64, swap: bool) -> Option<Ring> {
    let pole = s.pole(usize::from(!swap), up)?;
    close_around(&mut ring, turns, period);
    let (from, to) = (ring.corners[ring.corners.len() - 1].uv[0], ring.corners[0].uv[0]);
    let steps = (((from - to).abs() / s.step()[usize::from(swap)]).ceil().max(1.0) as usize).min(512);
    for k in 0..=steps {
        ring.push([from + (to - from) * k as f64 / steps as f64, pole], None);
    }
    Some(ring)
}

fn band(s: &Surface, mut a: Ring, ta: i32, mut b: Ring, tb: i32, period: f64) -> Ring {
    close_around(&mut a, ta, period);
    let end = a.corners[a.corners.len() - 1].uv;
    // start the other loop where this one ends, so that the seam between them runs along v
    let start = (0..b.corners.len()).min_by(|&i, &j| wrap(b.corners[i].uv[0] - end[0], period).abs().total_cmp(&wrap(b.corners[j].uv[0] - end[0], period).abs())).unwrap_or(0);
    for c in &mut b.corners[..start] {
        c.uv[0] += tb as f64 * period;
    }
    b.corners.rotate_left(start);
    b.shift_u(period * ((end[0] - b.corners[0].uv[0]) / period).round());
    close_around(&mut b, tb, period);
    // the two seams, stepped where the surface bends along v
    let seam = |ring: &mut Ring, from: P2, to: P2| {
        let steps = if s.curved_in_v() { (((to[1] - from[1]).abs() / s.step()[1]).ceil().max(1.0) as usize).min(512) } else { 1 };
        for k in 1..steps {
            let t = k as f64 / steps as f64;
            ring.push([from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t], None);
        }
    };
    let mut ring = Ring::default();
    let (a0, b0, b_end) = (a.corners[0].uv, b.corners[0].uv, b.corners[b.corners.len() - 1].uv);
    ring.corners.extend(&a.corners);
    seam(&mut ring, end, b0);
    ring.corners.extend(&b.corners);
    seam(&mut ring, b_end, a0);
    ring
}

/// The whole of a closed surface as one ring: a sphere, a torus.
fn whole(s: &Surface) -> Option<Ring> {
    let [(u0, u1), (v0, v1)] = s.domain()?;
    let step = s.step();
    let mut ring = Ring::default();
    let mut side = |from: P2, to: P2| {
        let steps = (((to[0] - from[0]) / step[0]).abs().max(((to[1] - from[1]) / step[1]).abs()).round() as usize).clamp(1, 512);
        for k in 0..steps {
            let t = k as f64 / steps as f64;
            ring.push([from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t], None);
        }
    };
    side([u0, v0], [u1, v0]);
    side([u1, v0], [u1, v1]);
    side([u1, v1], [u0, v1]);
    side([u0, v1], [u0, v0]);
    Some(ring)
}

/// True when `p` is inside the rings (even-odd), in the same coordinates as the rings.
fn inside(rings: &[Vec<P2>], p: P2) -> bool {
    let mut odd = false;
    for ring in rings {
        for i in 0..ring.len() {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                odd = !odd;
            }
        }
    }
    odd
}

/// Triangles of rings in parameter space (the first is the outside), added to `mesh`: a
/// constrained Delaunay triangulation of the rings, with points inside where the surface bends
/// along both parameters. The edges of the rings stay as they are, so neighbouring faces share
/// their points.
///
/// The corners are in (a, b): (u, v), or with `swap` (v, u).
fn triangulate(mesh: &mut Mesh, rings: &[Ring], s: &Surface, sign: f64, swap: bool) -> bool {
    use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};
    let count: usize = rings.iter().map(|r| r.corners.len()).sum();
    if count < 3 {
        return false;
    }
    if rings.iter().flat_map(|r| &r.corners).any(|c| !c.uv[0].is_finite() || !c.uv[1].is_finite()) {
        return false;
    }
    let uv_of = |ab: P2| if swap { [ab[1], ab[0]] } else { ab };
    let mean_v = rings.iter().flat_map(|r| &r.corners).map(|c| uv_of(c.uv)[1]).sum::<f64>() / count as f64;
    let scale = s.scale(mean_v).map(|k| k.max(1e-12));
    let scale = uv_of(scale);
    // (the triangulation takes no numbers that are next to nothing: such a one is 0)
    let flat = |uv: P2| [uv[0] * scale[0], uv[1] * scale[1]].map(|c| if c.abs() < 1e-30 { 0.0 } else { c });
    let outlines: Vec<Vec<P2>> = rings.iter().map(|r| r.corners.iter().map(|c| flat(c.uv)).collect()).collect();
    let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::new();
    // the corner of each vertex of the triangulation
    let mut corners: Vec<Corner> = Vec::new();
    let insert = |cdt: &mut ConstrainedDelaunayTriangulation<Point2<f64>>, corners: &mut Vec<Corner>, c: Corner| {
        let p = flat(c.uv);
        let handle = cdt.insert(Point2::new(p[0], p[1])).ok()?;
        // a point at a place taken already is that vertex
        if handle.index() == corners.len() {
            corners.push(c);
        }
        Some(handle)
    };
    for ring in rings {
        let handles: Option<Vec<_>> = ring.corners.iter().map(|c| insert(&mut cdt, &mut corners, *c)).collect();
        let Some(handles) = handles else { return false };
        for i in 0..handles.len() {
            let (a, b) = (handles[i], handles[(i + 1) % handles.len()]);
            if a != b {
                // rings that cross (damaged data) cannot both be kept
                if !cdt.can_add_constraint(a, b) {
                    return false;
                }
                cdt.add_constraint(a, b);
            }
        }
    }
    // points inside, a step apart, where the surface bends along v as well as along u
    if swap || s.curved_in_v() {
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for c in rings.iter().flat_map(|r| &r.corners) {
            for k in 0..2 {
                lo[k] = lo[k].min(c.uv[k]);
                hi[k] = hi[k].max(c.uv[k]);
            }
        }
        let step = uv_of(s.step());
        let steps = [0, 1].map(|k| ((hi[k] - lo[k]) / step[k]).ceil().max(1.0) as usize);
        if steps[0] * steps[1] <= MAX_FACE_TRIANGLES {
            for i in 1..steps[0] {
                for j in 1..steps[1] {
                    let uv = [lo[0] + (hi[0] - lo[0]) * i as f64 / steps[0] as f64, lo[1] + (hi[1] - lo[1]) * j as f64 / steps[1] as f64];
                    if inside(&outlines, flat(uv)) {
                        insert(&mut cdt, &mut corners, Corner { uv, exact: None });
                    }
                }
            }
        }
    }
    let base = mesh.positions.len() as u32;
    for c in &corners {
        let [u, v] = uv_of(c.uv);
        mesh.positions.push(c.exact.unwrap_or_else(|| s.eval(u, v)));
        mesh.normals.push(mul(s.normal(u, v), sign));
    }
    let before = mesh.triangles.len();
    for face in cdt.inner_faces() {
        let v = face.vertices();
        let t = [v[0].fix().index(), v[1].fix().index(), v[2].fix().index()];
        let centre = [0, 1].map(|k| {
            (v[0].position().x * (1 - k) as f64
                + v[0].position().y * k as f64
                + v[1].position().x * (1 - k) as f64
                + v[1].position().y * k as f64
                + v[2].position().x * (1 - k) as f64
                + v[2].position().y * k as f64)
                / 3.0
        });
        if !inside(&outlines, centre) {
            continue;
        }
        let [a, b, c] = t.map(|i| mesh.positions[base as usize + i]);
        // a triangle without area (two of its corners at a pole) is left out
        let longest = len(sub(b, a)).max(len(sub(c, b))).max(len(sub(a, c)));
        if len(cross(sub(b, a), sub(c, a))) <= 1e-12 * longest * longest {
            continue;
        }
        // turned to the normals at its corners. Where those do not say (a sliver, a corner
        // where the surface has no normal): counter-clockwise in (u, v) is counter-clockwise
        // about the natural normal, and the triangulation gives counter-clockwise in (a, b).
        // That alone would not do: at a pole a triangle can turn the other way in space
        let geometric = cross(sub(b, a), sub(c, a));
        let wanted = add(add(mesh.normals[base as usize + t[0]], mesh.normals[base as usize + t[1]]), mesh.normals[base as usize + t[2]]);
        let agree = dot(geometric, wanted);
        let keep = if agree.abs() > 1e-6 * len(geometric) * len(wanted) { agree > 0.0 } else { (sign > 0.0) != swap };
        let t = if keep { t } else { [t[0], t[2], t[1]] };
        mesh.triangles.push(t.map(|i| base + i as u32));
    }
    mesh.triangles.len() > before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{dist, Frame};
    use crate::surface::Kind;

    fn circle(z: f64, r: f64, reverse: bool) -> Vec<P3> {
        let n = 32;
        let mut ring: Vec<P3> = (0..n).map(|k| k as f64 / n as f64 * TAU).map(|a| [r * a.cos(), r * a.sin(), z]).collect();
        if reverse {
            ring.reverse();
        }
        ring
    }

    fn upright(kind: Kind) -> Surface {
        Surface { kind, frame: Frame::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]), positive: true, nurbs: None, profile: None, offset: 0.0 }
    }

    fn area(mesh: &Mesh) -> f64 {
        mesh.triangles.iter().map(|t| t.map(|i| mesh.positions[i as usize])).map(|[a, b, c]| len(cross(sub(b, a), sub(c, a))) / 2.0).sum()
    }

    /// Every triangle with some area turns the way `outward` points at its middle, and so do
    /// its normals.
    fn consistent(mesh: &Mesh, outward: impl Fn(P3) -> P3) -> bool {
        mesh.triangles.iter().all(|t| {
            let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
            let n = cross(sub(b, a), sub(c, a));
            let out = outward(mul(add(add(a, b), c), 1.0 / 3.0));
            // (a triangle on edge to the surface, as one up a meridian to a pole, turns neither way)
            let on_edge = dot(n, out).abs() < 1e-9 * len(n) * len(out);
            len(n) < 1e-9 || on_edge || (dot(n, out) > 0.0 && t.iter().all(|&i| dot(mesh.normals[i as usize], out) > 0.0))
        })
    }

    #[test]
    fn damaged_rings_are_dropped_and_repeated_points_removed() {
        assert_eq!(clean(vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0; 3]]).unwrap().len(), 3);
        assert!(clean(vec![[0.0; 3], [f64::NAN, 0.0, 0.0], [1.0, 1.0, 0.0]]).is_none());
        assert!(clean(vec![[0.0; 3], [1e30, 0.0, 0.0], [1.0, 1.0, 0.0]]).is_none());
        assert!(clean(vec![[0.0; 3], [1.0, 0.0, 0.0]]).is_none());
        // a face of nothing but damaged loops gives no triangles
        let mut mesh = Mesh::default();
        flat(&mut mesh, &[], None);
        assert!(mesh.triangles.is_empty());
    }

    #[test]
    fn plane_face_with_a_hole() {
        let outer = vec![[0.0, 0.0, 1.0], [4.0, 0.0, 1.0], [4.0, 3.0, 1.0], [0.0, 3.0, 1.0]];
        let hole = vec![[1.0, 1.0, 1.0], [1.0, 2.0, 1.0], [2.0, 2.0, 1.0], [2.0, 1.0, 1.0]];
        let mut mesh = Mesh::default();
        flat(&mut mesh, &[hole, outer], Some([0.0, 0.0, -1.0]));
        assert!((area(&mesh) - 11.0).abs() < 1e-12);
        assert_eq!(mesh.normals[0], [0.0, 0.0, -1.0]);
        assert!(consistent(&mesh, |_| [0.0, 0.0, -1.0]));
    }

    #[test]
    fn cylinder_side_between_two_circles() {
        let s = upright(Kind::Cylinder { radius: 0.5 });
        // seen from outside the lower circle runs towards larger u, the upper one back
        let mut mesh = Mesh::default();
        assert_eq!(curved(&mut mesh, &[circle(0.0, 0.5, false), circle(2.0, 0.5, true)], &s, 1.0), Ok(()));
        // the two circles have their points at the same angles: the 32-sided prism
        let side = 2.0 * 0.5 * (TAU / 64.0).sin();
        assert!((area(&mesh) - 32.0 * side * 2.0).abs() < 1e-9, "{}", area(&mesh));
        assert_eq!(mesh.triangles.len(), 64);
        // away from the axis
        assert!(consistent(&mesh, |p| [p[0], p[1], 0.0]));
    }

    #[test]
    fn sphere_cap_reaches_the_pole() {
        let s = upright(Kind::Sphere { radius: 1.0 });
        let z = 0.5f64;
        let ring = circle(z, (1.0 - z * z).sqrt(), false);
        let mut mesh = Mesh::default();
        assert_eq!(curved(&mut mesh, &[ring], &s, 1.0), Ok(()));
        // the cap above z = 0.5 has area 2 pi r h = pi
        assert!((area(&mesh) / (TAU / 2.0) - 1.0).abs() < 0.05, "{}", area(&mesh));
        assert!(mesh.positions.iter().any(|p| dist(*p, [0.0, 0.0, 1.0]) < 1e-9));
        assert!(mesh.positions.iter().all(|p| (len(*p) - 1.0).abs() < 1e-9 && p[2] > z - 1e-9));
        assert!(consistent(&mesh, |p| p));
    }

    #[test]
    fn sphere_without_loops_is_whole() {
        let s = upright(Kind::Sphere { radius: 2.0 });
        let mut mesh = Mesh::default();
        assert_eq!(curved(&mut mesh, &[], &s, 1.0), Ok(()));
        assert!((area(&mesh) / (2.0 * TAU * 4.0) - 1.0).abs() < 0.03, "{}", area(&mesh));
        assert!(mesh.positions.iter().all(|p| (len(*p) - 2.0).abs() < 1e-9));
        assert!(consistent(&mesh, |p| p));
    }

    #[test]
    fn face_on_a_b_surface_follows_the_surface() {
        // the quarter cylinder (radius 1, height 2), bounded by its own four edges
        let n = std::rc::Rc::new(crate::nurbs::tests::quarter_cylinder());
        let s = Surface { kind: Kind::Nurbs, frame: Frame::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]), positive: true, nurbs: Some(n.clone()), profile: None, offset: 0.0 };
        let mut ring = Vec::new();
        for k in 0..8 {
            ring.push(n.eval(k as f64 / 8.0, 0.0));
        }
        ring.push(n.eval(1.0, 0.0));
        for k in (0..=8).rev() {
            ring.push(n.eval(k as f64 / 8.0, 1.0));
        }
        let mut mesh = Mesh::default();
        assert_eq!(curved(&mut mesh, &[ring], &s, 1.0), Ok(()));
        assert!(mesh.positions.iter().all(|p| (p[0].hypot(p[1]) - 1.0).abs() < 1e-9));
        // a quarter of 2 pi r h
        assert!((area(&mesh) / (TAU / 2.0) - 1.0).abs() < 0.01, "{}", area(&mesh));
        assert!(consistent(&mesh, |p| [p[0], p[1], 0.0]));
        // and the surface on its own is the same patch
        let mut whole_patch = Mesh::default();
        assert_eq!(curved(&mut whole_patch, &[], &s, 1.0), Ok(()));
        assert!((area(&whole_patch) / (TAU / 2.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn b_surface_inside_rings_in_its_parameters() {
        let n = std::rc::Rc::new(crate::nurbs::tests::quarter_cylinder());
        // half of it along the arc, with a hole
        let outline = vec![[0.0, 0.0], [0.5, 0.0], [0.5, 1.0], [0.0, 1.0]];
        let hole = vec![[0.2, 0.4], [0.3, 0.4], [0.3, 0.6], [0.2, 0.6]];
        let (whole, half, holed) = (trimmed(&n, &[]), trimmed(&n, std::slice::from_ref(&outline)), trimmed(&n, &[hole.clone(), outline]));
        assert!((area(&whole) / (TAU / 2.0) - 1.0).abs() < 0.01);
        assert!(area(&half) > 0.2 * area(&whole) && area(&half) < 0.8 * area(&whole));
        assert!(area(&holed) < area(&half) && area(&holed) > 0.8 * area(&half));
        assert!(holed.positions.iter().all(|p| (p[0].hypot(p[1]) - 1.0).abs() < 1e-9));
        // rings that are no rings give the whole surface
        assert_eq!(trimmed(&n, &[vec![[0.0, 0.0], [1.0, 1.0]]]).triangles.len(), whole.triangles.len());
    }

    #[test]
    fn band_around_the_tube_of_a_torus() {
        // the torus with major radius 2 and minor radius 0.5 about z; a face between the two
        // circles around the tube at u = 0 and u = 1 (they go around in v)
        let s = upright(Kind::Torus { major: 2.0, minor: 0.5 });
        let tube = |u: f64, reverse: bool| {
            let mut ring: Vec<P3> = (0..32).map(|k| s.eval(u, k as f64 / 32.0 * TAU)).collect();
            if reverse {
                ring.reverse();
            }
            ring
        };
        let mut mesh = Mesh::default();
        assert_eq!(curved(&mut mesh, &[tube(0.0, true), tube(1.0, false)], &s, 1.0), Ok(()));
        // a part of the torus of 1 radian: area (2 pi b)(a * 1)
        assert!((area(&mesh) / (TAU * 0.5 * 2.0) - 1.0).abs() < 0.03, "{}", area(&mesh));
        let on_torus = |p: &P3| ((p[0].hypot(p[1]) - 2.0).hypot(p[2]) - 0.5).abs() < 1e-9;
        assert!(mesh.positions.iter().all(on_torus));
        // away from the centre line of the tube
        assert!(consistent(&mesh, |p| {
            let r = p[0].hypot(p[1]);
            [p[0] * (1.0 - 2.0 / r), p[1] * (1.0 - 2.0 / r), p[2]]
        }));
    }

    #[test]
    fn faces_this_cannot_follow_say_why() {
        let s = upright(Kind::Cylinder { radius: 0.5 });
        let mut mesh = Mesh::default();
        // one circle on a cylinder bounds nothing: a cylinder has no pole
        assert_eq!(curved(&mut mesh, &[circle(0.0, 0.5, false)], &s, 1.0), Err("one loop around the axis and no pole on that side"));
        assert_eq!(curved(&mut mesh, &[circle(0.0, 0.5, false), circle(2.0, 0.5, false)], &s, 1.0), Err("two loops around the axis the same way"));
        assert_eq!(curved(&mut mesh, &[], &s, 1.0), Err("no loops on an open surface"));
        assert!(mesh.triangles.is_empty());
    }

    #[test]
    fn cone_from_its_base_circle_to_the_apex() {
        // radius 1 at v = 0, widening towards +z: R = P + vA + radial (r + v tan a), tan = 1,
        // so the apex is at v = -1, which is z = -1
        let s = upright(Kind::Cone { radius: 1.0, tan: 1.0 });
        let mut mesh = Mesh::default();
        // the natural normal of a cone points outward; the face is below its circle, which
        // then runs clockwise seen from above
        assert_eq!(curved(&mut mesh, &[circle(0.0, 1.0, true)], &s, 1.0), Ok(()));
        assert!(mesh.positions.iter().any(|p| dist(*p, [0.0, 0.0, -1.0]) < 1e-9));
        // lateral area pi r l = pi sqrt(2)
        assert!((area(&mesh) / (TAU / 2.0 * 2f64.sqrt()) - 1.0).abs() < 0.04, "{}", area(&mesh));
        // away from the axis and downwards along the slope
        assert!(consistent(&mesh, |p| [p[0], p[1], -p[0].hypot(p[1])]));
    }
}
