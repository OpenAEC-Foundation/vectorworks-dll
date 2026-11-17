//! B-spline (NURBS) surfaces: the point at given parameters, and the parameters of a point.
//!
//! A surface has control points in a grid of `count[0]` by `count[1]`, a degree and a knot
//! vector per direction, and for a rational surface a weight per control point, stored as a
//! fourth coordinate with x, y and z multiplied by it (as the reference describes for curves).

use crate::curve::ARC_STEP;
use crate::geom::{cross, dist, dot, sub, unit, P3};

/// Steps per knot span when a surface of degree 2 or more is drawn or searched.
const SPAN_STEPS: usize = 6;
/// Most steps along one direction of a surface.
const MAX_STEPS: usize = 96;

#[derive(Debug, Clone, PartialEq)]
pub struct NurbsSurface {
    pub degree: [usize; 2],
    /// Control points along u and along v.
    pub count: [usize; 2],
    /// Values per control point: 3, or 4 with a weight.
    pub dim: usize,
    /// The control points, `dim` values each; point (i, j) is at `i * count[1] + j`.
    pub vertices: Vec<f64>,
    /// The knots of u and of v, multiplicities written out.
    pub knots: [Vec<f64>; 2],
    /// The surface meets itself along u, along v (its first and last parameter line coincide).
    pub closed: [bool; 2],
    /// Points of the surface on a coarse grid, to start searches from: (u, v, point).
    samples: Vec<(f64, f64, P3)>,
    /// The size of the surface: the diagonal of the box around those points.
    size: f64,
    /// Steps along u and along v the surface is drawn with.
    steps: [usize; 2],
}

/// The index of the first basis function that is not zero at `t`, and the values of the
/// `degree + 1` that are (Cox-de Boor).
fn basis(knots: &[f64], degree: usize, count: usize, t: f64) -> (usize, Vec<f64>) {
    let t = t.clamp(knots[degree], knots[count]);
    // the span k with knots[k] <= t < knots[k + 1], the last one at the end of the range
    let mut k = degree;
    while k + 1 < count && knots[k + 1] <= t {
        k += 1;
    }
    let mut n = vec![0.0; degree + 1];
    let mut left = vec![0.0; degree + 1];
    let mut right = vec![0.0; degree + 1];
    n[0] = 1.0;
    for j in 1..=degree {
        left[j] = t - knots[k + 1 - j];
        right[j] = knots[k + j] - t;
        let mut saved = 0.0;
        for r in 0..j {
            let span = right[r + 1] + left[j - r];
            let temp = if span != 0.0 { n[r] / span } else { 0.0 };
            n[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        n[j] = saved;
    }
    (k - degree, n)
}

impl NurbsSurface {
    /// A surface from its parts; `None` when they do not fit together. With `v_first` the
    /// control points come with v running slowest (point (i, j) at `j * count[0] + i`).
    pub fn new(degree: [usize; 2], count: [usize; 2], dim: usize, mut vertices: Vec<f64>, knots: [Vec<f64>; 2], closed: [bool; 2], v_first: bool) -> Option<NurbsSurface> {
        let fits = (3..=4).contains(&dim)
            && count.iter().all(|c| *c >= 2)
            && vertices.len() == count[0] * count[1] * dim
            && (0..2).all(|a| degree[a] >= 1 && count[a] > degree[a] && knots[a].len() == count[a] + degree[a] + 1 && knots[a].windows(2).all(|w| w[0] <= w[1]))
            && vertices.iter().chain(knots.iter().flatten()).all(|v| v.is_finite());
        if !fits {
            return None;
        }
        if v_first {
            let mut turned = vec![0.0; vertices.len()];
            for i in 0..count[0] {
                for j in 0..count[1] {
                    let (from, to) = ((j * count[0] + i) * dim, (i * count[1] + j) * dim);
                    turned[to..to + dim].copy_from_slice(&vertices[from..from + dim]);
                }
            }
            vertices = turned;
        }
        let mut s = NurbsSurface { degree, count, dim, vertices, knots, closed, samples: Vec::new(), size: 0.0, steps: [1, 1] };
        let (us, vs) = (s.parameters(0, 3), s.parameters(1, 3));
        s.samples = us.iter().flat_map(|&u| vs.iter().map(move |&v| (u, v))).map(|(u, v)| (u, v, s.eval_inside(u, v))).collect();
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for sample in &s.samples {
            for k in 0..3 {
                lo[k] = lo[k].min(sample.2[k]);
                hi[k] = hi[k].max(sample.2[k]);
            }
        }
        s.size = dist(lo, hi);
        if !s.size.is_finite() {
            return None;
        }
        s.steps = s.turning([us.len(), vs.len()]);
        Some(s)
    }

    /// Steps along u and along v to draw the surface with: one for every [`ARC_STEP`] its
    /// lines turn, or the lines across them turn from one to the next (a twisted surface),
    /// measured on the sample grid of `grid[0]` by `grid[1]` points. A direction of degree 1
    /// keeps its spans: its lines have corners there.
    fn turning(&self, grid: [usize; 2]) -> [usize; 2] {
        let at = |i: usize, j: usize| self.samples[i * grid[1] + j].2;
        // the angle between two directions; none where one of them has no length
        let angle = |a: P3, b: P3| {
            let (la, lb) = (dot(a, a).sqrt(), dot(b, b).sqrt());
            if la > 1e-12 * self.size && lb > 1e-12 * self.size {
                (dot(a, b) / (la * lb)).clamp(-1.0, 1.0).acos()
            } else {
                0.0
            }
        };
        [0, 1].map(|axis| {
            if self.degree[axis] <= 1 {
                return self.parameters(axis, 1).len().saturating_sub(1).clamp(1, MAX_STEPS);
            }
            // point k along `axis` on line `line` across it
            let point = |k: usize, line: usize| if axis == 0 { at(k, line) } else { at(line, k) };
            let (along, across) = (grid[axis], grid[1 - axis]);
            let mut most = 0.0f64;
            for line in 0..across {
                let bend: f64 = (1..along.saturating_sub(1)).map(|k| angle(sub(point(k, line), point(k - 1, line)), sub(point(k + 1, line), point(k, line)))).sum();
                let twist: f64 = if line + 1 < across { (1..along).map(|k| angle(sub(point(k - 1, line + 1), point(k - 1, line)), sub(point(k, line + 1), point(k, line)))).sum() } else { 0.0 };
                most = most.max(bend).max(twist);
            }
            // the corners of a line of n pieces show (n - 1) / n of what the curve turns
            let pieces = along.saturating_sub(1).max(2) as f64;
            ((most * pieces / (pieces - 1.0) / ARC_STEP - 1e-9).ceil() as usize).clamp(1, MAX_STEPS)
        })
    }

    /// The range of u (`axis` 0) or v (1) in which the surface is defined.
    pub fn range(&self, axis: usize) -> (f64, f64) {
        (self.knots[axis][self.degree[axis]], self.knots[axis][self.count[axis]])
    }

    /// The point at (u, v). Outside its range a surface goes on straight (faces may lie on
    /// such an extension); a closed direction comes round again.
    pub fn eval(&self, u: f64, v: f64) -> P3 {
        let inside = self.inside([u, v]);
        let mut p = self.eval_inside(inside[0], inside[1]);
        // how far beyond the end of an open direction
        let beyond = [0, 1].map(|k| if self.closed[k] { 0.0 } else { [u, v][k] - inside[k] });
        if beyond != [0.0, 0.0] {
            let d = self.derivatives(inside[0], inside[1]);
            for k in 0..3 {
                p[k] += beyond[0] * d[0][k] + beyond[1] * d[1][k];
            }
        }
        p
    }

    /// (u, v) brought into the range of the surface: wrapped in a closed direction, else limited.
    fn inside(&self, uv: [f64; 2]) -> [f64; 2] {
        [0, 1].map(|k| {
            let (lo, hi) = self.range(k);
            if self.closed[k] && hi > lo {
                lo + (uv[k] - lo).rem_euclid(hi - lo)
            } else {
                uv[k].clamp(lo, hi)
            }
        })
    }

    fn eval_inside(&self, u: f64, v: f64) -> P3 {
        let (iu, nu) = basis(&self.knots[0], self.degree[0], self.count[0], u);
        let (iv, nv) = basis(&self.knots[1], self.degree[1], self.count[1], v);
        let mut sum = [0.0; 4];
        for (a, bu) in nu.iter().enumerate() {
            for (b, bv) in nv.iter().enumerate() {
                let at = ((iu + a) * self.count[1] + iv + b) * self.dim;
                let w = bu * bv;
                for c in 0..self.dim {
                    sum[c] += w * self.vertices[at + c];
                }
            }
        }
        let w = if self.dim == 4 && sum[3] != 0.0 { sum[3] } else { 1.0 };
        [sum[0] / w, sum[1] / w, sum[2] / w]
    }

    /// The derivatives along u and along v at (u, v), by differences inside the range.
    fn derivatives(&self, u: f64, v: f64) -> [P3; 2] {
        // outside the range the surface goes on as it left it
        let [u, v] = self.inside([u, v]);
        let d = |axis: usize| {
            let (lo, hi) = self.range(axis);
            let h = (hi - lo) * 1e-6;
            let at = |t: f64| {
                if axis == 0 {
                    self.eval_inside(t, v)
                } else {
                    self.eval_inside(u, t)
                }
            };
            let t = if axis == 0 { u } else { v };
            let (a, b) = ((t - h).max(lo), (t + h).min(hi));
            sub(at(b), at(a)).map(|c| c / (b - a))
        };
        [d(0), d(1)]
    }

    /// The natural normal `dP/du × dP/dv` at (u, v), unit length.
    pub fn normal(&self, u: f64, v: f64) -> P3 {
        let [du, dv] = self.derivatives(u, v);
        unit(cross(du, dv))
    }

    /// The parameters of the point of the surface nearest to `p`, searched from `guess`: on the
    /// surface itself, and for a point that is not there on its extension as well.
    pub fn invert_near(&self, p: P3, guess: [f64; 2]) -> [f64; 2] {
        let on = self.search(p, guess, 0.0);
        if on.0 <= 1e-7 * self.size {
            return on.1;
        }
        let beyond = self.search(p, on.1, 1.0);
        if beyond.0 < on.0 {
            beyond.1
        } else {
            on.1
        }
    }

    /// The nearest point found from `guess`, as (distance, parameters). An open direction is
    /// searched up to `reach` times its range beyond its ends; a closed one is followed round.
    fn search(&self, p: P3, guess: [f64; 2], reach: f64) -> (f64, [f64; 2]) {
        let limit = |uv: [f64; 2]| {
            [0, 1].map(|k| {
                let (lo, hi) = self.range(k);
                if self.closed[k] {
                    uv[k]
                } else {
                    uv[k].clamp(lo - (hi - lo) * reach, hi + (hi - lo) * reach)
                }
            })
        };
        let mut uv = limit(guess);
        let mut best = (dist(self.eval(uv[0], uv[1]), p), uv);
        for _ in 0..32 {
            let [du, dv] = self.derivatives(uv[0], uv[1]);
            let r = sub(p, self.eval(uv[0], uv[1]));
            // least squares: [du dv] step = r
            let (a, b, c) = (dot(du, du), dot(du, dv), dot(dv, dv));
            let (e, f) = (dot(du, r), dot(dv, r));
            let det = a * c - b * b;
            let step = if det.abs() > 1e-30 {
                [(e * c - f * b) / det, (a * f - b * e) / det]
            } else if a > 1e-30 {
                [e / a, 0.0]
            } else if c > 1e-30 {
                [0.0, f / c]
            } else {
                break;
            };
            // the whole step, or as much of it as brings the point nearer (near a pole, where
            // one of the directions has no length, a whole step overshoots)
            let mut gain = None;
            let mut part = 1.0;
            for _ in 0..10 {
                let next = limit([uv[0] + step[0] * part, uv[1] + step[1] * part]);
                let d = dist(self.eval(next[0], next[1]), p);
                if d < best.0 {
                    gain = Some(best.0 - d);
                    best = (d, next);
                    uv = next;
                    break;
                }
                part /= 2.0;
            }
            if !gain.is_some_and(|g| g > 1e-13) {
                break;
            }
        }
        best
    }

    /// The parameters of the point of the surface nearest to `p`.
    pub fn invert(&self, p: P3) -> [f64; 2] {
        let start = self.samples.iter().min_by(|a, b| dist(a.2, p).total_cmp(&dist(b.2, p))).map_or([self.range(0).0, self.range(1).0], |s| [s.0, s.1]);
        self.invert_near(p, start)
    }

    /// Parameters along one direction to draw or search the surface with: its knots and `steps`
    /// steps between them (one for a degree of 1), at most [`MAX_STEPS`] in all.
    pub fn parameters(&self, axis: usize, steps: usize) -> Vec<f64> {
        let (lo, hi) = self.range(axis);
        let mut breaks: Vec<f64> = self.knots[axis].iter().copied().filter(|k| *k >= lo && *k <= hi).collect();
        breaks.dedup();
        let spans = breaks.len().saturating_sub(1).max(1);
        let steps = if self.degree[axis] <= 1 { 1 } else { steps.min(MAX_STEPS / spans).max(1) };
        let mut out = Vec::new();
        for w in breaks.windows(2) {
            out.extend((0..steps).map(|s| w[0] + (w[1] - w[0]) * s as f64 / steps as f64));
        }
        out.push(hi);
        out
    }

    /// The parameter step along u and along v at which the surface is drawn.
    pub fn step(&self) -> [f64; 2] {
        [0, 1].map(|axis| {
            let (lo, hi) = self.range(axis);
            (hi - lo) / self.steps[axis] as f64
        })
    }

    /// Lengths per unit of u and of v, from the sample grid.
    pub fn scale(&self) -> [f64; 2] {
        let (mut su, mut sv, mut n) = (0.0, 0.0, 0usize);
        for s in self.samples.iter().step_by((self.samples.len() / 16).max(1)) {
            let [du, dv] = self.derivatives(s.0, s.1);
            su += dot(du, du).sqrt();
            sv += dot(dv, dv).sqrt();
            n += 1;
        }
        [su / n.max(1) as f64, sv / n.max(1) as f64]
    }

    /// The whole surface as a grid of triangles: positions, unit normals (the natural normal)
    /// and triangles.
    pub fn patch(&self) -> (Vec<P3>, Vec<P3>, Vec<[u32; 3]>) {
        let (us, vs) = (self.parameters(0, SPAN_STEPS), self.parameters(1, SPAN_STEPS));
        let mut positions = Vec::with_capacity(us.len() * vs.len());
        let mut normals = Vec::with_capacity(us.len() * vs.len());
        for &u in &us {
            for &v in &vs {
                positions.push(self.eval(u, v));
                normals.push(self.normal(u, v));
            }
        }
        let mut triangles = Vec::new();
        let at = |i: usize, j: usize| (i * vs.len() + j) as u32;
        for i in 0..us.len() - 1 {
            for j in 0..vs.len() - 1 {
                triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
                triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
            }
        }
        (positions, normals, triangles)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A quarter of a cylinder of radius 1 about z, from z = 0 to 2: rational quadratic along u
    /// (the arc from (1, 0) to (0, 1)), straight along v.
    pub(crate) fn quarter_cylinder() -> NurbsSurface {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let arc = [[1.0, 0.0, 1.0], [w, w, w], [0.0, 1.0, 1.0]];
        let vertices: Vec<f64> = arc.iter().flat_map(|a| [[a[0], a[1], 0.0, a[2]], [a[0], a[1], 2.0 * a[2], a[2]]]).flatten().collect();
        NurbsSurface::new([2, 1], [3, 2], 4, vertices, [vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], vec![0.0, 0.0, 1.0, 1.0]], [false, false], false).unwrap()
    }

    #[test]
    fn points_are_on_the_cylinder() {
        let s = quarter_cylinder();
        assert_eq!((s.range(0), s.range(1)), ((0.0, 1.0), (0.0, 1.0)));
        for (u, v) in [(0.0, 0.0), (0.3, 0.5), (0.5, 1.0), (1.0, 0.25)] {
            let p = s.eval(u, v);
            assert!((p[0].hypot(p[1]) - 1.0).abs() < 1e-12 && (p[2] - 2.0 * v).abs() < 1e-12, "{u} {v}: {p:?}");
            // the normal points away from the axis
            let n = s.normal(u, v);
            assert!((n[0] * p[0] + n[1] * p[1] - 1.0).abs() < 1e-6 && n[2].abs() < 1e-6, "{n:?}");
        }
        assert_eq!(s.eval(0.0, 0.0), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn finds_the_parameters_of_a_point() {
        let s = quarter_cylinder();
        for (u, v) in [(0.2, 0.7), (0.9, 0.1), (0.5, 0.5), (0.0, 1.0)] {
            let p = s.eval(u, v);
            let [u2, v2] = s.invert(p);
            assert!(dist(s.eval(u2, v2), p) < 1e-9, "{u} {v} -> {u2} {v2}");
        }
    }

    #[test]
    fn goes_on_straight_outside_its_range() {
        // a flat patch from (0, 0) to (2, 1): a point beyond its corner is still found
        let flat = NurbsSurface::new([1, 1], [2, 2], 3, vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 2.0, 1.0, 0.0], [vec![0.0, 0.0, 1.0, 1.0], vec![0.0, 0.0, 1.0, 1.0]], [false, false], false)
            .unwrap();
        assert_eq!(flat.eval(1.5, -0.5), [3.0, -0.5, 0.0]);
        let [u, v] = flat.invert([3.0, -0.5, 0.0]);
        assert!((u - 1.5).abs() < 1e-9 && (v + 0.5).abs() < 1e-9, "{u} {v}");
    }

    #[test]
    fn a_closed_direction_comes_round_again() {
        // a square tube about z: four straight sides in u, closed; straight along v
        let corners = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0], [1.0, 0.0]];
        let vertices: Vec<f64> = corners.iter().flat_map(|c| [[c[0], c[1], 0.0], [c[0], c[1], 1.0]]).flatten().collect();
        let tube = NurbsSurface::new([1, 1], [5, 2], 3, vertices, [vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0], vec![0.0, 0.0, 1.0, 1.0]], [true, false], false).unwrap();
        for (u, v) in [(0.5, 0.5), (3.25, 0.0)] {
            assert!(dist(tube.eval(u - 4.0, v), tube.eval(u, v)) < 1e-12);
            assert!(dist(tube.eval(u + 8.0, v), tube.eval(u, v)) < 1e-12);
        }
        // a search that starts on the other side of the seam goes across it
        let [u, _] = tube.invert_near(tube.eval(3.9, 0.5), [0.1, 0.5]);
        assert!((u + 0.1).abs() < 1e-9, "{u}");
    }

    #[test]
    fn steps_follow_the_bend_of_the_surface() {
        // a quarter circle takes a quarter of the steps of a circle (or one more: the turn is
        // an estimate); straight along v, one
        let [u, v] = quarter_cylinder().step();
        assert!((1.0 / 9.0 - 1e-12..=1.0 / 8.0 + 1e-12).contains(&u) && (v - 1.0).abs() < 1e-12, "{u} {v}");
        // a flat patch of degree 2 either way, with many control points: one step each way
        let grid: Vec<f64> = (0..5).flat_map(|i| (0..5).flat_map(move |j| [i as f64, j as f64, 0.0])).collect();
        let knots = vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0];
        let flat = NurbsSurface::new([2, 2], [5, 5], 3, grid, [knots.clone(), knots], [false, false], false).unwrap();
        assert_eq!(flat.step(), [3.0, 3.0]);
    }

    #[test]
    fn control_points_given_v_first_are_the_same_surface() {
        let a = quarter_cylinder();
        // the same control points with v running slowest
        let mut turned = vec![0.0; a.vertices.len()];
        for i in 0..3 {
            for j in 0..2 {
                turned[(j * 3 + i) * 4..(j * 3 + i) * 4 + 4].copy_from_slice(&a.vertices[(i * 2 + j) * 4..(i * 2 + j) * 4 + 4]);
            }
        }
        let b = NurbsSurface::new(a.degree, a.count, 4, turned, a.knots.clone(), [false, false], true).unwrap();
        assert_eq!(a.eval(0.37, 0.61), b.eval(0.37, 0.61));
    }

    #[test]
    fn patch_covers_the_surface_and_bad_data_is_refused() {
        let s = quarter_cylinder();
        let (positions, normals, triangles) = s.patch();
        assert_eq!(positions.len(), normals.len());
        assert_eq!(triangles.len(), SPAN_STEPS * 2);
        let area: f64 = triangles.iter().map(|t| t.map(|i| positions[i as usize])).map(|[a, b, c]| dot(cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(c, a))).sqrt() / 2.0).sum();
        // a quarter of 2 pi r h
        assert!((area / std::f64::consts::PI - 1.0).abs() < 0.01, "{area}");
        assert!(NurbsSurface::new([2, 1], [3, 2], 4, vec![0.0; 5], [vec![], vec![]], [false, false], false).is_none());
    }
}
