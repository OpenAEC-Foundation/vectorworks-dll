//! B-spline (NURBS) surfaces: the point at given parameters, and the parameters of a point.
//!
//! A surface has control points in a grid of `count[0]` by `count[1]`, a degree and a knot
//! vector per direction, and for a rational surface a weight per control point, stored as a
//! fourth coordinate with x, y and z multiplied by it (as the reference describes for curves).

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
        let mut s = NurbsSurface { degree, count, dim, vertices, knots, closed, samples: Vec::new() };
        let (us, vs) = (s.parameters(0, 3), s.parameters(1, 3));
        s.samples = us.iter().flat_map(|&u| vs.iter().map(move |&v| (u, v))).map(|(u, v)| (u, v, s.eval(u, v))).collect();
        Some(s)
    }

    /// The range of u (`axis` 0) or v (1) in which the surface is defined.
    pub fn range(&self, axis: usize) -> (f64, f64) {
        (self.knots[axis][self.degree[axis]], self.knots[axis][self.count[axis]])
    }

    pub fn eval(&self, u: f64, v: f64) -> P3 {
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
        let d = |axis: usize| {
            let (lo, hi) = self.range(axis);
            let h = (hi - lo) * 1e-6;
            let at = |t: f64| if axis == 0 { self.eval(t, v) } else { self.eval(u, t) };
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

    /// The parameters of the point of the surface nearest to `p`, searched from `guess`.
    pub fn invert_near(&self, p: P3, guess: [f64; 2]) -> [f64; 2] {
        let ranges = [self.range(0), self.range(1)];
        let mut uv = guess;
        let mut best = (dist(self.eval(uv[0], uv[1]), p), uv);
        for _ in 0..24 {
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
            for k in 0..2 {
                uv[k] = (uv[k] + step[k]).clamp(ranges[k].0, ranges[k].1);
            }
            let d = dist(self.eval(uv[0], uv[1]), p);
            if d < best.0 {
                let gain = best.0 - d;
                best = (d, uv);
                if gain < 1e-13 {
                    break;
                }
            } else {
                break;
            }
        }
        best.1
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
            (hi - lo) / (self.parameters(axis, SPAN_STEPS).len() - 1).max(1) as f64
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
