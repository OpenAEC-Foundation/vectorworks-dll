//! Surfaces of faces: evaluation, natural normal and the parameters of a point.
//!
//! Parametrisations as in Annex E of the reference (E.5.2.2):
//!
//! - plane `P + uX + vY`;
//! - cylinder `P + r(X cos u + Y sin u) + vA`;
//! - cone `P - vA + (X cos u + Y sin u)(r + v tan a)`;
//! - sphere `C + (X cos u + Y sin u) r cos v + r A sin v`;
//! - torus `C + (X cos u + Y sin u)(a + b cos v) + b A sin v`.
//!
//! The natural normal is `dP/du × dP/dv`. A face's normal is the natural normal when the senses
//! of the face and of the surface are equal, its opposite when they differ.

use std::f64::consts::TAU;

use crate::code;

const B_SURFACE: u16 = 124;
use std::rc::Rc;

use crate::curve::ARC_STEP;
use crate::geom::{unit, Frame, P3};
use crate::nurbs::NurbsSurface;
use crate::parse::{File, Node, Value};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Plane,
    Cylinder { radius: f64 },
    Cone { radius: f64, tan: f64 },
    Sphere { radius: f64 },
    Torus { major: f64, minor: f64 },
    /// A B-surface: see [`Surface::nurbs`].
    Nurbs,
}

#[derive(Debug, Clone)]
pub struct Surface {
    pub kind: Kind,
    /// Origin, reference direction and main axis (the normal of a plane); not used by a
    /// B-surface.
    pub frame: Frame,
    /// The `sense` of the surface node.
    pub positive: bool,
    /// The definition of a B-surface.
    pub nurbs: Option<Rc<NurbsSurface>>,
}

impl Surface {
    /// The surface of node `index`; `None` for the kinds not evaluated (swept, spun, blends,
    /// offsets).
    pub fn load(file: &File, index: u32) -> Option<Surface> {
        let n = file.node(index)?;
        let axes = |origin: &str, axis: &str| Some(Frame::new(n.vector(origin)?, n.vector(axis)?, n.vector("x_axis")?));
        let (kind, frame) = match n.code() {
            code::PLANE => (Kind::Plane, axes("pvec", "normal")?),
            code::CYLINDER => (Kind::Cylinder { radius: n.real("radius")? }, axes("pvec", "axis")?),
            code::CONE => {
                let (sin, cos) = (n.real("sin_half_angle")?, n.real("cos_half_angle")?);
                (Kind::Cone { radius: n.real("radius")?, tan: sin / cos }, axes("pvec", "axis")?)
            }
            code::SPHERE => (Kind::Sphere { radius: n.real("radius")? }, axes("centre", "axis")?),
            code::TORUS => (Kind::Torus { major: n.real("major_radius")?, minor: n.real("minor_radius")? }, axes("centre", "axis")?),
            B_SURFACE => return Surface::load_nurbs(file, index, false),
            _ => return None,
        };
        Some(Surface { kind, frame, positive: n.positive(), nurbs: None })
    }

    /// The B-surface of node `index` (a `B_SURFACE`). The reference does not say in which order
    /// the control points come: with u running slowest, as the edges of 225 faces of a corpus
    /// file showed (none fitted the other order). `v_first` takes them the other way.
    pub fn load_nurbs(file: &File, index: u32, v_first: bool) -> Option<Surface> {
        let n = file.node(index).filter(|n| n.code() == B_SURFACE)?;
        let def = file.node(n.ptr("nurbs"))?;
        let reals = |node: u32, field: &str| -> Option<Vec<f64>> {
            Some(file.node(node)?.many(field).iter().filter_map(|v| if let Value::Real(r) = v { Some(*r) } else { None }).collect())
        };
        let knots = |values: &str, mult: &str| -> Option<Vec<f64>> {
            let distinct = reals(def.ptr(values), "knots")?;
            let mult: Vec<i64> = file.node(def.ptr(mult))?.many("mult").iter().filter_map(|v| if let Value::Int(i) = v { Some(*i) } else { None }).collect();
            Some(distinct.iter().zip(&mult).flat_map(|(k, m)| std::iter::repeat_n(*k, (*m).clamp(0, 64) as usize)).collect())
        };
        let count = |field: &str| def.int(field).and_then(|v| usize::try_from(v).ok());
        let closed = |a: &str, b: &str| def.int(a) == Some(1) || def.int(b) == Some(1);
        let nurbs = NurbsSurface::new(
            [count("u_degree")?, count("v_degree")?],
            [count("n_u_vertices")?, count("n_v_vertices")?],
            count("vertex_dim")?,
            reals(def.ptr("bspline_vertices"), "vertices")?,
            [knots("u_knots", "u_knot_mult")?, knots("v_knots", "v_knot_mult")?],
            [closed("u_closed", "u_periodic"), closed("v_closed", "v_periodic")],
            v_first,
        )?;
        let frame = Frame::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
        Some(Surface { kind: Kind::Nurbs, frame, positive: n.positive(), nurbs: Some(Rc::new(nurbs)) })
    }

    pub fn eval(&self, u: f64, v: f64) -> P3 {
        if let Some(n) = &self.nurbs {
            return n.eval(u, v);
        }
        let (s, c) = u.sin_cos();
        self.frame.world(match self.kind {
            Kind::Plane => [u, v, 0.0],
            Kind::Cylinder { radius } => [radius * c, radius * s, v],
            Kind::Cone { radius, tan } => {
                let rho = radius + v * tan;
                [rho * c, rho * s, -v]
            }
            Kind::Sphere { radius } => [radius * v.cos() * c, radius * v.cos() * s, radius * v.sin()],
            Kind::Torus { major, minor } => {
                let rho = major + minor * v.cos();
                [rho * c, rho * s, minor * v.sin()]
            }
            Kind::Nurbs => [0.0; 3],
        })
    }

    /// The natural normal at (u, v), unit length.
    pub fn normal(&self, u: f64, v: f64) -> P3 {
        if let Some(n) = &self.nurbs {
            return n.normal(u, v);
        }
        let (s, c) = u.sin_cos();
        self.frame.direction(unit(match self.kind {
            Kind::Plane => [0.0, 0.0, 1.0],
            Kind::Cylinder { .. } => [c, s, 0.0],
            // inward and towards the axis direction: -(radial + A tan a), see the derivation of
            // dP/du × dP/dv for the cone parametrisation
            Kind::Cone { tan, .. } => [-c, -s, -tan],
            Kind::Sphere { .. } | Kind::Torus { .. } => [v.cos() * c, v.cos() * s, v.sin()],
            Kind::Nurbs => [0.0, 0.0, 1.0],
        }))
    }

    /// The parameters of a point on the surface. At a pole of a sphere or the apex of a cone `u`
    /// has no meaning (see [`Surface::singular`]).
    pub fn invert(&self, p: P3) -> [f64; 2] {
        if let Some(n) = &self.nurbs {
            return n.invert(p);
        }
        let l = self.frame.local(p);
        let u = l[1].atan2(l[0]);
        match self.kind {
            Kind::Plane => [l[0], l[1]],
            Kind::Cylinder { .. } => [u, l[2]],
            Kind::Cone { .. } => [u, -l[2]],
            Kind::Sphere { radius } => [u, (l[2] / radius).clamp(-1.0, 1.0).asin()],
            Kind::Torus { major, .. } => [u, l[2].atan2(l[0].hypot(l[1]) - major)],
            Kind::Nurbs => [0.0; 2],
        }
    }

    /// As [`Surface::invert`], for a point near one whose parameters are `guess`: a B-surface is
    /// searched from there, which follows a boundary from point to point.
    pub fn invert_near(&self, p: P3, guess: [f64; 2]) -> [f64; 2] {
        match &self.nurbs {
            Some(n) => n.invert_near(p, guess),
            None => self.invert(p),
        }
    }

    /// True when `u` means nothing at this point: on the axis of the surface.
    pub fn singular(&self, p: P3) -> bool {
        let l = self.frame.local(p);
        let scale = match self.kind {
            Kind::Plane | Kind::Torus { .. } | Kind::Nurbs => return false,
            Kind::Cylinder { radius } | Kind::Cone { radius, .. } | Kind::Sphere { radius } => radius.abs().max(l[2].abs()),
        };
        l[0].hypot(l[1]) <= 1e-9 * scale.max(1e-12)
    }

    /// The period of `u`, for the surfaces that close around their axis.
    pub fn period_u(&self) -> Option<f64> {
        match &self.nurbs {
            Some(n) => n.closed[0].then(|| n.range(0).1 - n.range(0).0),
            None => (self.kind != Kind::Plane).then_some(TAU),
        }
    }

    /// The period of `v` (a torus).
    pub fn period_v(&self) -> Option<f64> {
        match &self.nurbs {
            Some(n) => n.closed[1].then(|| n.range(1).1 - n.range(1).0),
            None => matches!(self.kind, Kind::Torus { .. }).then_some(TAU),
        }
    }

    /// The `v` where the surface meets its axis towards larger (`up`) or smaller `v`: a pole of a
    /// sphere, the apex of a cone.
    pub fn pole(&self, up: bool) -> Option<f64> {
        match self.kind {
            Kind::Sphere { .. } => Some(if up { TAU / 4.0 } else { -TAU / 4.0 }),
            // the radius r + v tan a is 0 at v = -r / tan a
            Kind::Cone { radius, tan } if tan != 0.0 => {
                let apex = -radius / tan;
                // which side of the cone the apex is on follows from the sign of tan
                ((apex > 0.0) == up || apex == 0.0).then_some(apex)
            }
            _ => None,
        }
    }

    /// Lengths per unit of `u` and of `v` near (u, v), to give triangles in parameter space a
    /// sensible shape.
    pub fn scale(&self, v: f64) -> [f64; 2] {
        match self.kind {
            Kind::Plane => [1.0, 1.0],
            Kind::Cylinder { radius } => [radius.abs(), 1.0],
            Kind::Cone { radius, tan } => [(radius + v * tan).abs(), (1.0 + tan * tan).sqrt()],
            Kind::Sphere { radius } => [radius.abs(), radius.abs()],
            Kind::Torus { major, minor } => [major.abs() + minor.abs(), minor.abs()],
            Kind::Nurbs => self.nurbs.as_ref().map_or([1.0, 1.0], |n| n.scale()),
        }
    }

    /// True when the surface bends along `v` as well as along `u`.
    pub fn curved_in_v(&self) -> bool {
        matches!(self.kind, Kind::Sphere { .. } | Kind::Torus { .. } | Kind::Nurbs)
    }

    /// The step in `u` and in `v` between points the surface is drawn with.
    pub fn step(&self) -> [f64; 2] {
        match &self.nurbs {
            Some(n) => n.step(),
            None => [ARC_STEP, ARC_STEP],
        }
    }

    /// The ranges of `u` and `v` of a surface that is bounded by itself: a sphere, a torus, a
    /// B-surface.
    pub fn domain(&self) -> Option<[(f64, f64); 2]> {
        match self.kind {
            Kind::Sphere { .. } => Some([(0.0, TAU), (-TAU / 4.0, TAU / 4.0)]),
            Kind::Torus { .. } => Some([(0.0, TAU), (0.0, TAU)]),
            Kind::Nurbs => self.nurbs.as_ref().map(|n| [n.range(0), n.range(1)]),
            _ => None,
        }
    }
}

/// The normal of a face from the natural normal of its surface.
pub fn face_sign(face: &Node, surface: &Surface) -> f64 {
    if face.positive() == surface.positive {
        1.0
    } else {
        -1.0
    }
}

/// `dP/du × dP/dv` by differences, for checks.
#[cfg(test)]
fn numeric_normal(s: &Surface, u: f64, v: f64) -> P3 {
    use crate::geom::{cross, sub};
    let h = 1e-6;
    let du = sub(s.eval(u + h, v), s.eval(u - h, v));
    let dv = sub(s.eval(u, v + h), s.eval(u, v - h));
    unit(cross(du, dv))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{dist, dot};

    fn all() -> Vec<Surface> {
        let frame = Frame::new([1.0, -2.0, 0.5], [0.0, 0.6, 0.8], [1.0, 0.0, 0.0]);
        [
            Kind::Plane,
            Kind::Cylinder { radius: 0.3 },
            Kind::Cone { radius: 0.4, tan: 0.5 },
            Kind::Sphere { radius: 0.7 },
            Kind::Torus { major: 1.2, minor: 0.25 },
        ]
        .map(|kind| Surface { kind, frame, positive: true, nurbs: None })
        .to_vec()
    }

    #[test]
    fn invert_is_the_inverse_of_eval() {
        for s in all() {
            for (u, v) in [(0.3, 0.2), (2.5, -0.4), (-1.9, 1.1)] {
                let [u2, v2] = s.invert(s.eval(u, v));
                assert!(dist(s.eval(u2, v2), s.eval(u, v)) < 1e-9, "{:?}", s.kind);
                assert_eq!(s.invert_near(s.eval(u, v), [0.0, 0.0]), [u2, v2]);
                assert!((u2 - u).abs() < 1e-9 && (v2 - v).abs() < 1e-9, "{:?} {u} {v} -> {u2} {v2}", s.kind);
            }
        }
    }

    #[test]
    fn normal_is_the_cross_product_of_the_derivatives() {
        for s in all() {
            for (u, v) in [(0.3, 0.2), (2.5, -0.4), (-1.9, 1.1)] {
                assert!(dot(s.normal(u, v), numeric_normal(&s, u, v)) > 1.0 - 1e-6, "{:?} at {u} {v}", s.kind);
            }
        }
    }

    #[test]
    fn a_b_surface_is_evaluated_through_its_definition() {
        let n = crate::nurbs::tests::quarter_cylinder();
        let s = Surface { kind: Kind::Nurbs, frame: all()[0].frame, positive: true, nurbs: Some(Rc::new(n)) };
        let p = s.eval(0.4, 0.5);
        assert!((p[0].hypot(p[1]) - 1.0).abs() < 1e-12 && (p[2] - 1.0).abs() < 1e-12);
        assert!(dot(s.normal(0.4, 0.5), [p[0], p[1], 0.0]) > 0.999);
        let [u, v] = s.invert(p);
        assert!(dist(s.eval(u, v), p) < 1e-9);
        assert_eq!((s.period_u(), s.domain()), (None, Some([(0.0, 1.0), (0.0, 1.0)])));
        assert!(s.curved_in_v() && !s.singular(p));
    }

    #[test]
    fn poles_and_apex() {
        let s = all();
        assert!(s[3].singular(s[3].eval(1.0, TAU / 4.0)));
        assert!(!s[3].singular(s[3].eval(1.0, 0.3)));
        // cone radius 0.4, tan 0.5: apex at v = -0.8
        assert_eq!(s[2].pole(false), Some(-0.8));
        assert_eq!(s[2].pole(true), None);
        assert!(s[2].singular(s[2].eval(2.0, -0.8)));
    }
}
