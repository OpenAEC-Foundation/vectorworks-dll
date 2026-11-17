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
use crate::geom::{unit, Frame, P3};
use crate::parse::{File, Node};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Plane,
    Cylinder { radius: f64 },
    Cone { radius: f64, tan: f64 },
    Sphere { radius: f64 },
    Torus { major: f64, minor: f64 },
}

#[derive(Debug, Clone, Copy)]
pub struct Surface {
    pub kind: Kind,
    /// Origin, reference direction and main axis (the normal of a plane).
    pub frame: Frame,
    /// The `sense` of the surface node.
    pub positive: bool,
}

impl Surface {
    /// The surface of node `index`; `None` for the kinds not evaluated (B-surfaces, swept,
    /// spun, blends, offsets).
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
            _ => return None,
        };
        Some(Surface { kind, frame, positive: n.positive() })
    }

    pub fn eval(&self, u: f64, v: f64) -> P3 {
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
        })
    }

    /// The natural normal at (u, v), unit length.
    pub fn normal(&self, u: f64, v: f64) -> P3 {
        let (s, c) = u.sin_cos();
        self.frame.direction(unit(match self.kind {
            Kind::Plane => [0.0, 0.0, 1.0],
            Kind::Cylinder { .. } => [c, s, 0.0],
            // inward and towards the axis direction: -(radial + A tan a), see the derivation of
            // dP/du × dP/dv for the cone parametrisation
            Kind::Cone { tan, .. } => [-c, -s, -tan],
            Kind::Sphere { .. } | Kind::Torus { .. } => [v.cos() * c, v.cos() * s, v.sin()],
        }))
    }

    /// The parameters of a point on the surface. At a pole of a sphere or the apex of a cone `u`
    /// has no meaning (see [`Surface::singular`]).
    pub fn invert(&self, p: P3) -> [f64; 2] {
        let l = self.frame.local(p);
        let u = l[1].atan2(l[0]);
        match self.kind {
            Kind::Plane => [l[0], l[1]],
            Kind::Cylinder { .. } => [u, l[2]],
            Kind::Cone { .. } => [u, -l[2]],
            Kind::Sphere { radius } => [u, (l[2] / radius).clamp(-1.0, 1.0).asin()],
            Kind::Torus { major, .. } => [u, l[2].atan2(l[0].hypot(l[1]) - major)],
        }
    }

    /// True when `u` means nothing at this point: on the axis of the surface.
    pub fn singular(&self, p: P3) -> bool {
        let l = self.frame.local(p);
        let scale = match self.kind {
            Kind::Plane | Kind::Torus { .. } => return false,
            Kind::Cylinder { radius } | Kind::Cone { radius, .. } | Kind::Sphere { radius } => radius.abs().max(l[2].abs()),
        };
        l[0].hypot(l[1]) <= 1e-9 * scale.max(1e-12)
    }

    /// The period of `u`, for the surfaces that close around their axis.
    pub fn period_u(&self) -> Option<f64> {
        (self.kind != Kind::Plane).then_some(TAU)
    }

    /// The period of `v` (a torus).
    pub fn period_v(&self) -> Option<f64> {
        matches!(self.kind, Kind::Torus { .. }).then_some(TAU)
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
        }
    }

    /// True when the surface bends along `v` as well as along `u`.
    pub fn curved_in_v(&self) -> bool {
        matches!(self.kind, Kind::Sphere { .. } | Kind::Torus { .. })
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
        .map(|kind| Surface { kind, frame, positive: true })
        .to_vec()
    }

    #[test]
    fn invert_is_the_inverse_of_eval() {
        for s in all() {
            for (u, v) in [(0.3, 0.2), (2.5, -0.4), (-1.9, 1.1)] {
                let [u2, v2] = s.invert(s.eval(u, v));
                assert!(dist(s.eval(u2, v2), s.eval(u, v)) < 1e-9, "{:?}", s.kind);
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
