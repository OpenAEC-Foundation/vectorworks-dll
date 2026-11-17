//! Points along the curve of an edge.
//!
//! An edge runs from its start vertex to its end vertex along its curve; whether that is the
//! direction of increasing curve parameter follows from the `sense` of the curve nodes (a
//! trimmed curve is positive, its basis curve says whether its parameter increases). A fin uses
//! the edge in its own direction or against it.
//!
//! Lines need no points between their ends. Circles and ellipses are stepped by angle. B-curves
//! are evaluated; intersection curves come with their own points (the chart); a curve given in
//! the parameters of a surface (SP-curve) is evaluated on that surface. For the kinds not
//! evaluated the edge is a straight segment.

use std::f64::consts::TAU;

use crate::geom::{add, dist, mul, sub, Frame, P3};
use crate::parse::{File, Node, Value};
use crate::surface::{Kind, Surface};

/// Largest angle between two points on a circle.
pub const ARC_STEP: f64 = TAU / 32.0;
/// Points per span of a B-curve of degree 2 or more.
const SPAN_STEPS: usize = 6;

const LINE: u16 = 30;
const CIRCLE: u16 = 31;
const ELLIPSE: u16 = 32;
const INTERSECTION: u16 = 38;
const TRIMMED_CURVE: u16 = 133;
const B_CURVE: u16 = 134;
const SP_CURVE: u16 = 137;

/// The node type of the curve that gives node `curve` its shape (a trimmed curve is followed
/// to its basis curve), and whether [`between`] follows that kind; other kinds are drawn as a
/// straight edge.
pub fn shape(file: &File, curve: u32) -> (u16, bool) {
    let mut node = file.node(curve);
    while let Some(n) = node.filter(|n| n.code() == TRIMMED_CURVE) {
        node = file.node(n.ptr("basis_curve"));
    }
    let code = node.map_or(0, Node::code);
    (code, matches!(code, LINE | CIRCLE | ELLIPSE | B_CURVE | INTERSECTION | SP_CURVE))
}

/// The points of the curve of node `curve` strictly between `from` and `to`, going along the
/// edge when `forward`. With `from` and `to` `None` (a ring edge, without vertices): the whole
/// closed curve, as a ring of points.
pub fn between(file: &File, curve: u32, from: Option<P3>, to: Option<P3>, forward: bool) -> Vec<P3> {
    // down to the curve that has a shape, collecting the direction
    let mut increasing = forward;
    let mut node = file.node(curve);
    while let Some(n) = node {
        increasing ^= !n.positive();
        if n.code() != TRIMMED_CURVE {
            break;
        }
        node = file.node(n.ptr("basis_curve"));
    }
    let Some(n) = node else { return Vec::new() };
    match n.code() {
        LINE => Vec::new(),
        CIRCLE | ELLIPSE => conic(n, from, to, increasing).unwrap_or_default(),
        B_CURVE => {
            let points = bspline_points(file, n).unwrap_or_default();
            along(&points, from, to, increasing)
        }
        INTERSECTION => {
            let points: Vec<P3> = file.node(n.ptr("chart")).map(|c| c.many("Hvec").iter().filter_map(vector).collect()).unwrap_or_default();
            along(&points, from, to, increasing)
        }
        // a curve in the parameters of a surface (the edge of a face with a tolerance)
        SP_CURVE => along(&surface_curve_points(file, n).unwrap_or_default(), from, to, increasing),
        _ => Vec::new(),
    }
}

fn vector(v: &Value) -> Option<P3> {
    match v {
        Value::Vector(p) => Some(*p),
        _ => None,
    }
}

/// The frame and the two radii of a circle or an ellipse `C + a X cos t + b Y sin t`.
fn conic_shape(n: &Node) -> Option<(Frame, f64, f64)> {
    let frame = Frame::new(n.vector("centre")?, n.vector("normal")?, n.vector("x_axis")?);
    let (a, b) = match n.real("radius") {
        Some(r) => (r, r),
        None => (n.real("major_radius")?, n.real("minor_radius")?),
    };
    (a.is_finite() && b.is_finite() && a != 0.0 && b != 0.0).then_some((frame, a, b))
}

/// Points on a circle or an ellipse.
fn conic(n: &Node, from: Option<P3>, to: Option<P3>, increasing: bool) -> Option<Vec<P3>> {
    let (frame, a, b) = conic_shape(n)?;
    let angle = |p: P3| {
        let l = frame.local(p);
        (l[1] / b).atan2(l[0] / a)
    };
    let at = |t: f64| frame.world([a * t.cos(), b * t.sin(), 0.0]);
    let (start, sweep) = match (from, to) {
        (Some(p), Some(q)) => {
            let (t0, t1) = (angle(p), angle(q));
            let mut sweep = if increasing { t1 - t0 } else { t0 - t1 }.rem_euclid(TAU);
            // the same point at both ends (an edge with one vertex) is the whole curve
            if sweep < 1e-9 {
                sweep = TAU;
            }
            (t0, sweep)
        }
        _ => (0.0, TAU),
    };
    let steps = (sweep / ARC_STEP).ceil().max(1.0) as usize;
    let sign = if increasing { 1.0 } else { -1.0 };
    let ring = from.is_none();
    // a ring gives all its points; an arc those strictly between its ends
    let range = if ring { 0..steps } else { 1..steps };
    Some(range.map(|k| at(start + sign * sweep * k as f64 / steps as f64)).collect())
}

/// The part of the polyline `points` strictly between `from` and `to`. A closed polyline is
/// followed in its own order when `increasing`, else against it, past its end; on an open one
/// there is only one way from the one point to the other.
///
/// The ends are placed on the polyline where it passes nearest to them, between its points:
/// an edge shorter than the step between the points of its curve must not be taken for the
/// curve all the way round (as it would be if its ends were matched to the nearest points,
/// which can come in the wrong order).
fn along(points: &[P3], from: Option<P3>, to: Option<P3>, increasing: bool) -> Vec<P3> {
    if points.len() < 2 {
        return Vec::new();
    }
    let size = points.iter().map(|p| dist(*p, points[0])).fold(0.0, f64::max);
    let closed = dist(points[0], points[points.len() - 1]) < 1e-9 * (1.0 + size);
    let (Some(from), Some(to)) = (from, to) else {
        let mut ring: Vec<P3> = points[..points.len() - usize::from(closed)].to_vec();
        if !increasing {
            ring.reverse();
        }
        return ring;
    };
    let (a, b) = (place(points, from), place(points, to));
    // the points strictly between, by their place along the polyline
    let mut out = Vec::new();
    if closed {
        let n = points.len() - 1;
        let length = n as f64;
        let ahead = |from: f64, to: f64| if increasing { to - from } else { from - to }.rem_euclid(length);
        // an edge with one vertex goes all the way round
        let span = if dist(from, to) <= 1e-12 * (1.0 + size) { length } else { ahead(a, b) };
        let mut inside: Vec<(f64, usize)> = (0..n).map(|k| (ahead(a, k as f64), k)).filter(|(d, _)| *d > 0.0 && *d < span).collect();
        inside.sort_by(|x, y| x.0.total_cmp(&y.0));
        out.extend(inside.iter().map(|(_, k)| points[*k]));
    } else {
        let inside = (0..points.len()).filter(|&k| (k as f64) > a.min(b) && (k as f64) < a.max(b));
        if a < b {
            out.extend(inside.map(|k| points[k]));
        } else {
            out.extend(inside.rev().map(|k| points[k]));
        }
    }
    // the ends themselves belong to the vertices
    let tolerance = 1e-9 * (1.0 + dist(from, to));
    out.retain(|p| dist(*p, from) > tolerance && dist(*p, to) > tolerance);
    out
}

/// Where the polyline `points` passes nearest to `p`: `k + t` on the piece from point `k` to
/// point `k + 1`.
fn place(points: &[P3], p: P3) -> f64 {
    let mut best = (f64::MAX, 0.0);
    for k in 0..points.len() - 1 {
        let (a, b) = (points[k], points[k + 1]);
        let d = sub(b, a);
        let l = crate::geom::dot(d, d);
        let t = if l > 0.0 { (crate::geom::dot(sub(p, a), d) / l).clamp(0.0, 1.0) } else { 0.0 };
        let miss = dist(add(a, mul(d, t)), p);
        if miss < best.0 {
            best = (miss, k as f64 + t);
        }
    }
    best.1
}

/// A curve by its parameter, as the surfaces made from a curve use it: a swept surface its
/// section, a spun surface its profile. The parametrisations are those of the reference
/// (E.5.2.1): a line `P + tD`, a circle or ellipse `C + a X cos t + b Y sin t`, a B-curve its
/// own. The `sense` of the curve node plays no part: the parameter is the natural one.
#[derive(Debug, Clone)]
pub enum Path {
    Line {
        point: P3,
        direction: P3,
    },
    Conic {
        frame: Frame,
        a: f64,
        b: f64,
    },
    Spline(Nurbs),
    /// The points an intersection curve comes with (its chart), straight between them; the
    /// parameter is the number of the point.
    Chain(Vec<P3>),
}

impl Path {
    /// The curve of node `curve`; `None` for the kinds that have no parameter here.
    pub fn load(file: &File, curve: u32) -> Option<Path> {
        let mut n = file.node(curve)?;
        for _ in 0..8 {
            if n.code() != TRIMMED_CURVE {
                break;
            }
            n = file.node(n.ptr("basis_curve"))?;
        }
        match n.code() {
            LINE => {
                let (point, direction) = (n.vector("pvec")?, n.vector("direction")?);
                (crate::geom::len(direction) > 0.0).then_some(Path::Line { point, direction })
            }
            CIRCLE | ELLIPSE => conic_shape(n).map(|(frame, a, b)| Path::Conic { frame, a, b }),
            B_CURVE => {
                let nurbs = Nurbs::curve(file, file.node(n.ptr("nurbs"))?)?;
                (nurbs.dim == 3 + usize::from(nurbs.rational) && nurbs.range().is_some_and(|(lo, hi)| hi > lo)).then_some(Path::Spline(nurbs))
            }
            INTERSECTION => {
                let mut points: Vec<P3> = file.node(n.ptr("chart"))?.many("Hvec").iter().filter_map(vector).filter(|p| p.iter().all(|c| c.is_finite())).collect();
                points.dedup();
                (points.len() >= 2).then_some(Path::Chain(points))
            }
            _ => None,
        }
    }

    pub fn eval(&self, t: f64) -> P3 {
        match self {
            Path::Line { point, direction } => add(*point, mul(*direction, t)),
            Path::Conic { frame, a, b } => frame.world([a * t.cos(), b * t.sin(), 0.0]),
            Path::Spline(nurbs) => {
                let p = nurbs.eval(t);
                [p[0], p[1], p[2]]
            }
            // (beyond its ends it goes on straight)
            Path::Chain(points) => {
                let i = (t.floor().max(0.0) as usize).min(points.len() - 2);
                lerp(points[i], points[i + 1], t - i as f64)
            }
        }
    }

    /// The parameters the curve is defined between; a line has no ends.
    pub fn range(&self) -> Option<(f64, f64)> {
        match self {
            Path::Line { .. } => None,
            Path::Conic { .. } => Some((0.0, TAU)),
            Path::Spline(nurbs) => nurbs.range(),
            Path::Chain(points) => Some((0.0, (points.len() - 1) as f64)),
        }
    }

    /// The period of a curve that closes on itself.
    pub fn period(&self) -> Option<f64> {
        let (lo, hi) = self.range()?;
        let (a, b, mid) = (self.eval(lo), self.eval(hi), self.eval((lo + hi) / 2.0));
        (dist(a, b) < 1e-9 * (1.0 + dist(a, mid))).then_some(hi - lo)
    }

    /// Parameters to draw and search the curve with; a line is known by any two.
    pub fn parameters(&self) -> Vec<f64> {
        match self {
            Path::Line { .. } => vec![0.0, 1.0],
            Path::Conic { .. } => (0..=32).map(|k| ARC_STEP * k as f64).collect(),
            Path::Spline(nurbs) => nurbs.parameters(),
            Path::Chain(points) => (0..points.len()).map(|k| k as f64).collect(),
        }
    }
}

/// A B-spline (NURBS) curve as the reference stores it.
#[derive(Debug, Clone)]
pub struct Nurbs {
    pub degree: usize,
    pub dim: usize,
    /// The control points, `dim` values each; a rational curve has its weight last and the
    /// coordinates multiplied by it.
    pub vertices: Vec<f64>,
    /// The knots with their multiplicities written out.
    pub knots: Vec<f64>,
    pub rational: bool,
}

impl Nurbs {
    /// The curve of a `NURBS_CURVE` node.
    pub fn curve(file: &File, nurbs: &Node) -> Option<Nurbs> {
        let reals = |n: u32, field: &str| -> Option<Vec<f64>> { Some(file.node(n)?.many(field).iter().filter_map(|v| if let Value::Real(r) = v { Some(*r) } else { None }).collect()) };
        let mult: Vec<i64> = file.node(nurbs.ptr("knot_mult"))?.many("mult").iter().filter_map(|v| if let Value::Int(i) = v { Some(*i) } else { None }).collect();
        let distinct = reals(nurbs.ptr("knots"), "knots")?;
        let knots = distinct.iter().zip(&mult).flat_map(|(k, m)| std::iter::repeat_n(*k, (*m).max(0) as usize)).collect();
        Some(Nurbs {
            degree: nurbs.int("degree")? as usize,
            dim: nurbs.int("vertex_dim")? as usize,
            vertices: reals(nurbs.ptr("bspline_vertices"), "vertices")?,
            knots,
            rational: nurbs.int("rational") == Some(1),
        })
    }

    fn count(&self) -> usize {
        self.vertices.len() / self.dim.max(1)
    }

    /// The parameter range in which the curve is defined.
    pub fn range(&self) -> Option<(f64, f64)> {
        let n = self.count();
        (self.dim >= 1 && n > self.degree && self.knots.len() > n + self.degree).then(|| (self.knots[self.degree], self.knots[n]))
    }

    /// The point at parameter `t` (de Boor), with the weight divided out: `dim` values, or
    /// `dim - 1` for a rational curve.
    pub fn eval(&self, t: f64) -> Vec<f64> {
        let (p, n) = (self.degree, self.count());
        // the span k with knots[k] <= t < knots[k + 1], kept inside the defined range
        let mut k = p;
        while k + 1 < n && self.knots[k + 1] <= t {
            k += 1;
        }
        let mut d: Vec<Vec<f64>> = (0..=p).map(|j| self.vertices[(k - p + j) * self.dim..(k - p + j + 1) * self.dim].to_vec()).collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = k - p + j;
                let span = self.knots[i + p + 1 - r] - self.knots[i];
                let a = if span > 0.0 { (t - self.knots[i]) / span } else { 0.0 };
                for c in 0..self.dim {
                    d[j][c] = (1.0 - a) * d[j - 1][c] + a * d[j][c];
                }
            }
        }
        let mut out = d.swap_remove(p);
        if self.rational {
            let w = out.pop().filter(|w| *w != 0.0).unwrap_or(1.0);
            out.iter_mut().for_each(|c| *c /= w);
        }
        out
    }

    /// Parameters to draw the curve with: the knots of its range and steps between them.
    pub fn parameters(&self) -> Vec<f64> {
        let Some((lo, hi)) = self.range() else {
            return Vec::new();
        };
        let steps = if self.degree <= 1 { 1 } else { SPAN_STEPS };
        let mut breaks: Vec<f64> = self.knots.iter().copied().filter(|k| *k >= lo && *k <= hi).collect();
        breaks.dedup();
        let mut out = Vec::new();
        for w in breaks.windows(2) {
            out.extend((0..steps).map(|s| w[0] + (w[1] - w[0]) * s as f64 / steps as f64));
        }
        out.push(hi);
        out
    }
}

/// The points a `B_CURVE` node is drawn with (3D curves).
fn bspline_points(file: &File, b_curve: &Node) -> Option<Vec<P3>> {
    let nurbs = Nurbs::curve(file, file.node(b_curve.ptr("nurbs"))?)?;
    if nurbs.dim - usize::from(nurbs.rational) != 3 {
        return None;
    }
    Some(nurbs.parameters().into_iter().map(|t| nurbs.eval(t)).map(|p| [p[0], p[1], p[2]]).collect())
}

/// The points an `SP_CURVE` node is drawn with: its B-curve gives (u, v), its surface the
/// points.
fn surface_curve_points(file: &File, sp_curve: &Node) -> Option<Vec<P3>> {
    let surface = Surface::load(file, sp_curve.ptr("surface"))?;
    let nurbs = Nurbs::curve(file, file.node(file.node(sp_curve.ptr("b_curve"))?.ptr("nurbs"))?)?;
    if nurbs.dim != 2 + usize::from(nurbs.rational) {
        return None;
    }
    // what is straight in (u, v) bends with the surface: no step wider than the surface is
    // drawn with
    let step = surface.step();
    let bends = [surface.kind != Kind::Plane, surface.curved_in_v()];
    let mut out: Vec<P3> = Vec::new();
    let mut last: Option<(f64, Vec<f64>)> = None;
    for t in nurbs.parameters() {
        let uv = nurbs.eval(t);
        if let Some((t0, uv0)) = &last {
            let wide = (0..2).filter(|k| bends[*k]).map(|k| ((uv[k] - uv0[k]) / step[k]).abs()).fold(0.0, f64::max);
            let parts = if wide.is_finite() { (wide.ceil() as usize).clamp(1, 256) } else { 1 };
            for k in 1..parts {
                let uv = nurbs.eval(t0 + (t - t0) * k as f64 / parts as f64);
                out.push(surface.eval(uv[0], uv[1]));
            }
        }
        out.push(surface.eval(uv[0], uv[1]));
        last = Some((t, uv));
    }
    Some(out)
}

/// A point between two others, for callers that step along straight edges.
pub fn lerp(a: P3, b: P3, t: f64) -> P3 {
    add(a, mul(sub(b, a), t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nurbs_quarter_circle_is_on_the_circle() {
        // the rational quadratic quarter circle from (1, 0) to (0, 1)
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let n = Nurbs { degree: 2, dim: 4, vertices: vec![1.0, 0.0, 0.0, 1.0, w, w, 0.0, w, 0.0, 1.0, 0.0, 1.0], knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], rational: true };
        assert_eq!(n.range(), Some((0.0, 1.0)));
        for t in n.parameters() {
            let p = n.eval(t);
            assert!((p[0].hypot(p[1]) - 1.0).abs() < 1e-12, "{t}: {p:?}");
        }
        assert_eq!(n.eval(0.0), [1.0, 0.0, 0.0]);
        let end = n.eval(1.0);
        assert!(end[0].abs() < 1e-12 && (end[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn paths_by_their_parameter() {
        let line = Path::Line { point: [1.0, 0.0, 0.0], direction: [0.0, 2.0, 0.0] };
        assert_eq!(line.eval(1.5), [1.0, 3.0, 0.0]);
        assert_eq!((line.range(), line.period()), (None, None));
        let ellipse = Path::Conic { frame: Frame::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]), a: 2.0, b: 1.0 };
        assert!(dist(ellipse.eval(TAU / 4.0), [0.0, 1.0, 0.0]) < 1e-12);
        assert_eq!(ellipse.period(), Some(TAU));
        assert_eq!(ellipse.parameters().len(), 33);
        let chain = Path::Chain(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 2.0, 0.0]]);
        assert_eq!((chain.eval(0.5), chain.eval(1.5), chain.eval(3.0), chain.range()), ([0.5, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 4.0, 0.0], Some((0.0, 2.0))));
    }

    #[test]
    fn nurbs_polyline_with_inner_knots() {
        // degree 1 through (0,0), (1,0), (1,1)
        let n = Nurbs { degree: 1, dim: 2, vertices: vec![0.0, 0.0, 1.0, 0.0, 1.0, 1.0], knots: vec![0.0, 0.0, 1.0, 2.0, 2.0], rational: false };
        assert_eq!(n.parameters(), [0.0, 1.0, 2.0]);
        assert_eq!(n.eval(0.5), [0.5, 0.0]);
        assert_eq!(n.eval(1.5), [1.0, 0.5]);
        assert_eq!(n.eval(2.0), [1.0, 1.0]);
    }

    #[test]
    fn polyline_part_between_two_points_either_way() {
        let line: Vec<P3> = (0..6).map(|k| [k as f64, 0.0, 0.0]).collect();
        let (a, b) = (Some([1.0, 0.0, 0.0]), Some([4.0, 0.0, 0.0]));
        assert_eq!(along(&line, a, b, true), [[2.0, 0.0, 0.0], [3.0, 0.0, 0.0]]);
        assert_eq!(along(&line, b, a, false), [[3.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
        // an open curve has one way from the one point to the other, whatever the direction asked
        assert_eq!(along(&line, b, a, true), [[3.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
        assert!(along(&line, a, a, true).is_empty());
    }

    #[test]
    fn closed_polyline_is_followed_past_its_end() {
        let square: Vec<P3> = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]].iter().map(|p| [p[0], p[1], 0.0]).collect();
        let (a, b) = (Some([0.0, 1.0, 0.0]), Some([1.0, 0.0, 0.0]));
        assert_eq!(along(&square, a, b, true), [[0.0, 0.0, 0.0]]);
        assert_eq!(along(&square, None, None, false).len(), 4);
        // an edge between two points on one piece of the polyline has no points between
        let (c, d) = (Some([0.2, 0.0, 0.0]), Some([0.4, 0.0, 0.0]));
        assert!(along(&square, c, d, true).is_empty());
        assert_eq!(along(&square, d, c, true).len(), 4);
        // nor when its end is nearer to the next point than its start
        let (e, f) = (Some([0.45, 0.0, 0.0]), Some([0.55, 0.0, 0.0]));
        assert!(along(&square, e, f, true).is_empty());
        // and an edge with one vertex is the whole curve
        assert_eq!(along(&square, c, c, false).len(), 4);
    }
}
