//! Surfaces of faces: evaluation, natural normal and the parameters of a point.
//!
//! Parametrisations as in Annex E of the reference (E.5.2.2):
//!
//! - plane `P + uX + vY`;
//! - cylinder `P + r(X cos u + Y sin u) + vA`;
//! - cone `P + vA + (X cos u + Y sin u)(r + v tan a)`;
//! - sphere `C + (X cos u + Y sin u) r cos v + r A sin v`;
//! - torus `C + (X cos u + Y sin u)(a + b cos v) + b A sin v`;
//! - swept surface `C(u) + vD`, with `C` its section curve;
//! - spun surface: `C(u)`, its profile curve, turned about the axis by the angle `v`;
//! - blend (a rolling ball along an edge): `u` is that of its spine, the path of the centre of
//!   the ball; at each `u` a circular arc about the spine, from where the ball touches the
//!   first supporting surface (`v` = 0) to where it touches the second (`v` = 1).
//!
//! The cone is not as the reference prints it (`P - vA + ...`, the mirror image): the corners of
//! 58 cone faces of a corpus file lie on the surface above and off the printed one, and the
//! curves given in the parameters of a cone have `v` along `A`.
//!
//! The natural normal is `dP/du × dP/dv`. A face's normal is the natural normal when the senses
//! of the face and of the surface are equal, its opposite when they differ.

use std::f64::consts::TAU;

use crate::code;

const B_SURFACE: u16 = 124;
const SWEPT_SURF: u16 = 67;
const SPUN_SURF: u16 = 68;
const BLENDED_EDGE: u16 = 56;
const OFFSET_SURF: u16 = 60;
/// How deep surfaces made from surfaces are followed (a blend on a blend).
const MAX_DEPTH: usize = 3;
use std::rc::Rc;

use crate::curve::{Path, ARC_STEP};
use crate::geom::{add, cross, dist, dot, mul, sub, unit, Frame, P3};
use crate::nurbs::NurbsSurface;
use crate::parse::{File, Node, Value};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Plane,
    Cylinder {
        radius: f64,
    },
    Cone {
        radius: f64,
        tan: f64,
    },
    Sphere {
        radius: f64,
    },
    Torus {
        major: f64,
        minor: f64,
    },
    /// A B-surface: see [`Surface::nurbs`].
    Nurbs,
    /// A curve moved along the main axis of the frame: see [`Surface::profile`].
    Swept,
    /// A curve turned about the main axis of the frame: see [`Surface::profile`].
    Spun,
    /// A rolling ball blend: its spine is [`Surface::profile`], which has its two supporting
    /// surfaces.
    Blend,
}

/// The curve a swept or spun surface is made from, with what is needed to search it.
#[derive(Debug, Clone)]
pub struct Profile {
    pub path: Path,
    /// Parameters along the curve and the points there.
    samples: Vec<(f64, P3)>,
    /// The parameter step between the samples, and the length per unit of parameter.
    step: f64,
    speed: f64,
    /// A spun surface: the parameters at which the profile meets the axis (low, high), and
    /// the mean distance of the profile to the axis.
    limits: [Option<f64>; 2],
    radius: f64,
    /// A blend: the two surfaces the ball touches, and the angle of its arc at the middle of
    /// the spine.
    supports: Vec<Surface>,
    angle: f64,
}

impl Profile {
    pub fn new(path: Path) -> Option<Profile> {
        let samples: Vec<(f64, P3)> = path.parameters().into_iter().map(|t| (t, path.eval(t))).collect();
        let span = samples.last()?.0 - samples.first()?.0;
        let length: f64 = samples.windows(2).map(|w| dist(w[0].1, w[1].1)).sum();
        if samples.len() < 2 || !span.is_finite() || span <= 0.0 || !length.is_finite() || length <= 0.0 {
            return None;
        }
        Some(Profile { path, step: span / (samples.len() - 1) as f64, speed: length / span, samples, limits: [None; 2], radius: 1.0, supports: Vec::new(), angle: 1.0 })
    }
}

/// Some direction that is not along `z`, for a frame about `z` whose angles start anywhere.
fn aside(z: P3) -> P3 {
    let z = unit(z);
    if z[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    }
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
    /// The curve of a swept or spun surface.
    pub profile: Option<Rc<Profile>>,
    /// How far the surface lies from the one all the above describes, along its natural
    /// normal: an offset surface. It has the parameters of that surface.
    pub offset: f64,
}

impl Surface {
    /// The surface of node `index`; `None` for the kinds not evaluated (blends, offsets).
    pub fn load(file: &File, index: u32) -> Option<Surface> {
        Surface::load_within(file, index, 0)
    }

    fn load_within(file: &File, index: u32, depth: usize) -> Option<Surface> {
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
            SWEPT_SURF => {
                let sweep = n.vector("sweep")?;
                let profile = Profile::new(Path::load(file, n.ptr("section"))?)?;
                let frame = Frame::new([0.0; 3], sweep, aside(sweep));
                return Some(Surface { kind: Kind::Swept, frame, positive: n.positive(), nurbs: None, profile: Some(Rc::new(profile)), offset: 0.0 });
            }
            SPUN_SURF => {
                let (base, axis) = (n.vector("base")?, n.vector("axis")?);
                let frame = Frame::new(base, axis, aside(axis));
                let mut profile = Profile::new(Path::load(file, n.ptr("profile"))?)?;
                // a null parameter (-3.14158e13) is no limit
                profile.limits = ["start_param", "end_param"].map(|f| n.real(f).filter(|v| v.is_finite() && v.abs() < 1e12));
                let radius = |p: P3| frame.local(p)[0].hypot(frame.local(p)[1]);
                profile.radius = profile.samples.iter().map(|s| radius(s.1)).sum::<f64>() / profile.samples.len() as f64;
                return Some(Surface { kind: Kind::Spun, frame, positive: n.positive(), nurbs: None, profile: Some(Rc::new(profile)), offset: 0.0 });
            }
            // the underlying surface, moved along its normal; with its sense
            OFFSET_SURF if depth < MAX_DEPTH => {
                let mut surface = Surface::load_within(file, n.ptr("surface"), depth + 1)?;
                let offset = n.real("offset").filter(|d| d.is_finite())?;
                surface.offset += if surface.positive { offset } else { -offset };
                surface.positive = n.positive();
                return Some(surface);
            }
            BLENDED_EDGE if depth < MAX_DEPTH => {
                let supports: Vec<Surface> = n.many("surface").iter().filter_map(|v| if let Value::Ptr(p) = v { Surface::load_within(file, *p, depth + 1) } else { None }).collect();
                // the radius of the ball. A blend with none is an edge the ball of another blend
                // rolls along (a cliff edge): the curve that is its spine
                let radius = n.many("range").iter().filter_map(|v| if let Value::Real(r) = v { Some(r.abs()) } else { None }).fold(0.0, f64::max);
                if !radius.is_finite() || (radius > 0.0 && supports.len() != 2) {
                    return None;
                }
                let mut profile = Profile::new(Path::load(file, n.ptr("spine"))?)?;
                profile.supports = if radius > 0.0 { supports } else { Vec::new() };
                profile.radius = radius;
                let frame = Frame::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
                let mut surface = Surface { kind: Kind::Blend, frame, positive: n.positive(), nurbs: None, profile: Some(Rc::new(profile.clone())), offset: 0.0 };
                // the angle of the arc, for the steps across it
                let middle = profile.samples[profile.samples.len() / 2].0;
                profile.angle = surface.arc(&profile, middle).map_or(TAU / 4.0, |arc| arc.4).max(1e-3);
                surface.profile = Some(Rc::new(profile));
                return Some(surface);
            }
            _ => return None,
        };
        Some(Surface { kind, frame, positive: n.positive(), nurbs: None, profile: None, offset: 0.0 })
    }

    /// The B-surface of node `index` (a `B_SURFACE`). The reference does not say in which order
    /// the control points come: with u running slowest, as the edges of 225 faces of a corpus
    /// file showed (none fitted the other order). `v_first` takes them the other way.
    pub fn load_nurbs(file: &File, index: u32, v_first: bool) -> Option<Surface> {
        let n = file.node(index).filter(|n| n.code() == B_SURFACE)?;
        let def = file.node(n.ptr("nurbs"))?;
        let reals = |node: u32, field: &str| -> Option<Vec<f64>> { Some(file.node(node)?.many(field).iter().filter_map(|v| if let Value::Real(r) = v { Some(*r) } else { None }).collect()) };
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
        Some(Surface { positive: n.positive(), ..Surface::of(Rc::new(nurbs)) })
    }

    /// The B-surface with definition `nurbs`.
    pub fn of(nurbs: Rc<NurbsSurface>) -> Surface {
        let frame = Frame::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
        Surface { kind: Kind::Nurbs, frame, positive: true, nurbs: Some(nurbs), profile: None, offset: 0.0 }
    }

    /// True for the surfaces whose parameters are searched from a guess, not solved.
    pub fn searched(&self) -> bool {
        self.nurbs.is_some() || self.profile.is_some()
    }

    /// The parameter that is an angle about the axis: the one without meaning on the axis.
    pub fn angular(&self) -> usize {
        usize::from(self.kind == Kind::Spun)
    }

    /// What a point and the point of the profile it comes from have in common: its place
    /// across the sweep direction, or its distance to and its height along the spin axis.
    fn reduced(&self, p: P3) -> P3 {
        let l = self.frame.local(p);
        match self.kind {
            Kind::Spun => [l[0].hypot(l[1]), l[2], 0.0],
            // (a point of a blend comes from the point of the spine nearest to it)
            Kind::Blend => p,
            _ => [l[0], l[1], 0.0],
        }
    }

    /// The arc of a blend at `u` of its spine: its centre, the unit direction to where it
    /// starts (on the first supporting surface), the unit direction square to that in its
    /// plane towards its end, its radius and its angle.
    fn arc(&self, profile: &Profile, u: f64) -> Option<(P3, P3, P3, f64, f64)> {
        let c = profile.path.eval(u);
        let foot = |s: &Surface| {
            let uv = s.invert(c);
            sub(s.eval(uv[0], uv[1]), c)
        };
        let (a, b) = (foot(profile.supports.first()?), foot(profile.supports.get(1)?));
        let r = dot(a, a).sqrt();
        if !(r.is_finite() && r > 0.0) {
            return None;
        }
        let e1 = mul(a, 1.0 / r);
        let across = sub(b, mul(e1, dot(b, e1)));
        let l = dot(across, across).sqrt();
        (l > 1e-12 * r).then(|| (c, e1, mul(across, 1.0 / l), r, l.atan2(dot(b, e1))))
    }

    /// (u, v) of `p` on a blend, given `u`.
    fn across_blend(&self, profile: &Profile, p: P3, u: f64) -> [f64; 2] {
        match self.arc(profile, u) {
            Some((c, e1, e2, _, angle)) => {
                let d = sub(p, c);
                [u, dot(d, e2).atan2(dot(d, e1)) / angle]
            }
            None => [u, 0.0],
        }
    }

    /// The parameter of the point of the profile that `p` comes from, searched from `guess`
    /// or else from the nearest of the samples.
    fn profile_parameter(&self, profile: &Profile, p: P3, guess: Option<f64>) -> f64 {
        let target = self.reduced(p);
        let at = |t: f64| self.reduced(profile.path.eval(t));
        let nearest = || profile.samples.iter().min_by(|a, b| dist(self.reduced(a.1), target).total_cmp(&dist(self.reduced(b.1), target))).map_or(0.0, |s| s.0);
        let mut t = guess.unwrap_or_else(nearest);
        let mut best = (dist(at(t), target), t);
        let h = profile.step * 1e-5;
        for _ in 0..24 {
            let d = mul(sub(at(t + h), at(t - h)), 0.5 / h);
            let along = dot(d, d);
            if along.is_nan() || along <= 1e-30 {
                break;
            }
            t += dot(d, sub(target, at(t))) / along;
            let miss = dist(at(t), target);
            if miss < best.0 {
                let gain = best.0 - miss;
                best = (miss, t);
                if gain < 1e-13 {
                    break;
                }
            } else {
                break;
            }
        }
        best.1
    }

    /// (u, v) of `p` on a swept or spun surface, given `u`.
    fn across_profile(&self, profile: &Profile, p: P3, u: f64) -> [f64; 2] {
        let (l, c) = (self.frame.local(p), self.frame.local(profile.path.eval(u)));
        match self.kind {
            Kind::Spun => {
                let turn = l[1].atan2(l[0]) - c[1].atan2(c[0]);
                [u, turn - TAU * (turn / TAU).round()]
            }
            _ => [u, l[2] - c[2]],
        }
    }

    pub fn eval(&self, u: f64, v: f64) -> P3 {
        let p = self.eval_base(u, v);
        if self.offset == 0.0 {
            return p;
        }
        add(p, mul(self.normal(u, v), self.offset))
    }

    /// The point at (u, v) of the surface without its offset.
    fn eval_base(&self, u: f64, v: f64) -> P3 {
        if let Some(n) = &self.nurbs {
            return n.eval(u, v);
        }
        if let (Kind::Blend, Some(profile)) = (self.kind, &self.profile) {
            return match self.arc(profile, u) {
                Some((c, e1, e2, r, angle)) => {
                    let (s, co) = (v * angle).sin_cos();
                    add(c, add(mul(e1, r * co), mul(e2, r * s)))
                }
                None => profile.path.eval(u),
            };
        }
        if let Some(profile) = &self.profile {
            let c = profile.path.eval(u);
            return match self.kind {
                Kind::Spun => {
                    let (l, (s, c)) = (self.frame.local(c), v.sin_cos());
                    self.frame.world([l[0] * c - l[1] * s, l[0] * s + l[1] * c, l[2]])
                }
                _ => add(c, mul(self.frame.z, v)),
            };
        }
        let (s, c) = u.sin_cos();
        self.frame.world(match self.kind {
            Kind::Plane => [u, v, 0.0],
            Kind::Cylinder { radius } => [radius * c, radius * s, v],
            Kind::Cone { radius, tan } => {
                let rho = radius + v * tan;
                [rho * c, rho * s, v]
            }
            Kind::Sphere { radius } => [radius * v.cos() * c, radius * v.cos() * s, radius * v.sin()],
            Kind::Torus { major, minor } => {
                let rho = major + minor * v.cos();
                [rho * c, rho * s, minor * v.sin()]
            }
            Kind::Nurbs | Kind::Swept | Kind::Spun | Kind::Blend => [0.0; 3],
        })
    }

    /// The natural normal at (u, v), unit length.
    pub fn normal(&self, u: f64, v: f64) -> P3 {
        if let Some(n) = &self.nurbs {
            return n.normal(u, v);
        }
        if let Some(profile) = &self.profile {
            let h = profile.step * 1e-5;
            let du = sub(self.eval_base(u + h, v), self.eval_base(u - h, v));
            let dv = match self.kind {
                Kind::Spun | Kind::Blend => sub(self.eval_base(u, v + 1e-6), self.eval_base(u, v - 1e-6)),
                _ => self.frame.z,
            };
            return unit(cross(du, dv));
        }
        let (s, c) = u.sin_cos();
        self.frame.direction(unit(match self.kind {
            Kind::Plane => [0.0, 0.0, 1.0],
            Kind::Cylinder { .. } => [c, s, 0.0],
            // dP/du × dP/dv = (r + v tan a)(radial - A tan a): outward where the radius is positive
            Kind::Cone { tan, .. } => [c, s, -tan],
            Kind::Sphere { .. } | Kind::Torus { .. } => [v.cos() * c, v.cos() * s, v.sin()],
            Kind::Nurbs | Kind::Swept | Kind::Spun | Kind::Blend => [0.0, 0.0, 1.0],
        }))
    }

    /// The parameters of a point on the surface. At a pole of a sphere or the apex of a cone `u`
    /// has no meaning (see [`Surface::singular`]).
    pub fn invert(&self, p: P3) -> [f64; 2] {
        if let Some(n) = &self.nurbs {
            return n.invert(p);
        }
        if let (Kind::Blend, Some(profile)) = (self.kind, &self.profile) {
            return self.across_blend(profile, p, self.profile_parameter(profile, p, None));
        }
        if let Some(profile) = &self.profile {
            return self.across_profile(profile, p, self.profile_parameter(profile, p, None));
        }
        let l = self.frame.local(p);
        let u = l[1].atan2(l[0]);
        match self.kind {
            Kind::Plane => [l[0], l[1]],
            Kind::Cylinder { .. } => [u, l[2]],
            Kind::Cone { .. } => [u, l[2]],
            // (by angles, so that it also holds off the sphere: for an offset surface)
            Kind::Sphere { .. } => [u, l[2].atan2(l[0].hypot(l[1]))],
            Kind::Torus { major, .. } => [u, l[2].atan2(l[0].hypot(l[1]) - major)],
            Kind::Nurbs | Kind::Swept | Kind::Spun | Kind::Blend => [0.0; 2],
        }
    }

    /// As [`Surface::invert`], for a point near one whose parameters are `guess`: a B-surface,
    /// a swept and a spun surface are searched from there, which follows a boundary from point
    /// to point.
    pub fn invert_near(&self, p: P3, guess: [f64; 2]) -> [f64; 2] {
        if let Some(n) = &self.nurbs {
            return n.invert_near(p, guess);
        }
        if let (Kind::Blend, Some(profile)) = (self.kind, &self.profile) {
            return self.across_blend(profile, p, self.profile_parameter(profile, p, Some(guess[0])));
        }
        if let Some(profile) = &self.profile {
            let mut uv = self.across_profile(profile, p, self.profile_parameter(profile, p, Some(guess[0])));
            if self.kind == Kind::Spun {
                // the turn nearest to that of the guess
                uv[1] += TAU * ((guess[1] - uv[1]) / TAU).round();
            }
            return uv;
        }
        self.invert(p)
    }

    /// True when the angle about the axis (see [`Surface::angular`]) means nothing at this
    /// point: on the axis of the surface.
    pub fn singular(&self, p: P3) -> bool {
        let l = self.frame.local(p);
        let scale = match self.kind {
            Kind::Plane | Kind::Torus { .. } | Kind::Nurbs | Kind::Swept | Kind::Blend => return false,
            Kind::Cylinder { radius } | Kind::Cone { radius, .. } | Kind::Sphere { radius } => radius.abs().max(l[2].abs()),
            Kind::Spun => self.profile.as_ref().map_or(1.0, |p| p.radius).max(l[2].abs()),
        };
        l[0].hypot(l[1]) <= 1e-9 * scale.max(1e-12)
    }

    /// The period of `u`, for the surfaces that close around their axis or along their curve.
    pub fn period_u(&self) -> Option<f64> {
        if let Some(profile) = &self.profile {
            return profile.path.period();
        }
        match &self.nurbs {
            Some(n) => n.closed[0].then(|| n.range(0).1 - n.range(0).0),
            None => (self.kind != Kind::Plane).then_some(TAU),
        }
    }

    /// The period of `v` (a torus, a spun surface).
    pub fn period_v(&self) -> Option<f64> {
        match &self.nurbs {
            Some(n) => n.closed[1].then(|| n.range(1).1 - n.range(1).0),
            None => matches!(self.kind, Kind::Torus { .. } | Kind::Spun).then_some(TAU),
        }
    }

    /// Where the surface meets its axis: the value of parameter `axis` there, towards larger
    /// (`up`) or smaller values of it. A pole of a sphere and the apex of a cone are in `v`
    /// (`axis` 1), the ends of the profile of a spun surface in `u` (`axis` 0).
    pub fn pole(&self, axis: usize, up: bool) -> Option<f64> {
        match self.kind {
            Kind::Sphere { .. } if axis == 1 => Some(if up { TAU / 4.0 } else { -TAU / 4.0 }),
            // the radius r + v tan a is 0 at v = -r / tan a
            Kind::Cone { radius, tan } if axis == 1 && tan != 0.0 => {
                let apex = -radius / tan;
                // which side of the cone the apex is on follows from the sign of tan
                ((apex > 0.0) == up || apex == 0.0).then_some(apex)
            }
            Kind::Spun if axis == 0 => self.profile.as_ref()?.limits[usize::from(up)],
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
            Kind::Swept => [self.profile.as_ref().map_or(1.0, |p| p.speed), 1.0],
            Kind::Spun => self.profile.as_ref().map_or([1.0, 1.0], |p| [p.speed, p.radius.max(1e-12)]),
            Kind::Blend => self.profile.as_ref().map_or([1.0, 1.0], |p| [p.speed, (p.radius * p.angle).max(1e-12)]),
        }
    }

    /// True when the surface bends along `v` as well as along `u`.
    pub fn curved_in_v(&self) -> bool {
        matches!(self.kind, Kind::Sphere { .. } | Kind::Torus { .. } | Kind::Nurbs | Kind::Spun | Kind::Blend)
    }

    /// The step in `u` and in `v` between points the surface is drawn with.
    pub fn step(&self) -> [f64; 2] {
        if let Some(profile) = &self.profile {
            // (across a blend v runs from 0 to 1 over the angle of its arc)
            return [profile.step, if self.kind == Kind::Blend { ARC_STEP / profile.angle } else { ARC_STEP }];
        }
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
        [Kind::Plane, Kind::Cylinder { radius: 0.3 }, Kind::Cone { radius: 0.4, tan: 0.5 }, Kind::Sphere { radius: 0.7 }, Kind::Torus { major: 1.2, minor: 0.25 }]
            .map(|kind| Surface { kind, frame, positive: true, nurbs: None, profile: None, offset: 0.0 })
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
        let s = Surface { kind: Kind::Nurbs, frame: all()[0].frame, positive: true, nurbs: Some(Rc::new(n)), profile: None, offset: 0.0 };
        let p = s.eval(0.4, 0.5);
        assert!((p[0].hypot(p[1]) - 1.0).abs() < 1e-12 && (p[2] - 1.0).abs() < 1e-12);
        assert!(dot(s.normal(0.4, 0.5), [p[0], p[1], 0.0]) > 0.999);
        let [u, v] = s.invert(p);
        assert!(dist(s.eval(u, v), p) < 1e-9);
        assert_eq!((s.period_u(), s.domain()), (None, Some([(0.0, 1.0), (0.0, 1.0)])));
        assert!(s.curved_in_v() && !s.singular(p));
    }

    /// The line from (1, 0, 0) to (2, 0, 1) and on, swept along y or spun about z.
    fn from_a_line(kind: Kind) -> Surface {
        let mut profile = Profile::new(Path::Line { point: [1.0, 0.0, 0.0], direction: [1.0, 0.0, 1.0] }).unwrap();
        let z = if kind == Kind::Spun { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
        if kind == Kind::Spun {
            // the line meets the axis at t = -1
            profile.limits = [Some(-1.0), None];
            profile.radius = 1.5;
        }
        Surface { kind, frame: Frame::new([0.0; 3], z, aside(z)), positive: true, nurbs: None, profile: Some(Rc::new(profile)), offset: 0.0 }
    }

    #[test]
    fn swept_and_spun_surfaces_follow_their_curve() {
        let swept = from_a_line(Kind::Swept);
        assert!(dist(swept.eval(0.5, 3.0), [1.5, 3.0, 0.5]) < 1e-12);
        let spun = from_a_line(Kind::Spun);
        // a quarter turn about z takes (2, 0, 1) to (0, 2, 1)
        assert!(dist(spun.eval(1.0, TAU / 4.0), [0.0, 2.0, 1.0]) < 1e-12);
        for s in [&swept, &spun] {
            for (u, v) in [(0.3, 0.4), (-0.5, 2.0), (4.0, -1.5)] {
                let p = s.eval(u, v);
                let [iu, iv] = s.invert(p);
                assert!((iu - u).abs() < 1e-9 && (iv - v).abs() < 1e-9, "{:?} {u} {v} -> {iu} {iv}", s.kind);
                let n = s.normal(u, v);
                assert!(dot(n, numeric_normal(s, u, v)) > 1.0 - 1e-6, "{:?}", s.kind);
            }
        }
        // a turn further is found from a guess there
        let [_, v] = spun.invert_near(spun.eval(0.5, 0.1), [0.5, TAU]);
        assert!((v - (TAU + 0.1)).abs() < 1e-9, "{v}");
        // the cone this line makes has its apex where the line meets the axis
        assert!(spun.singular(spun.eval(-1.0, 1.0)) && !spun.singular(spun.eval(0.0, 1.0)));
        assert_eq!((spun.pole(0, false), spun.pole(0, true), spun.pole(1, false)), (Some(-1.0), None, None));
        assert_eq!((spun.period_u(), spun.period_v(), swept.period_v()), (None, Some(TAU), None));
    }

    #[test]
    fn blend_between_two_planes() {
        // the floor z = 0 and the wall x = 0, a ball of radius 1 rolled along y in their corner:
        // its centre runs along the line through (1, 0, 1)
        let plane = |normal: P3, x: P3| Surface { kind: Kind::Plane, frame: Frame::new([0.0; 3], normal, x), positive: true, nurbs: None, profile: None, offset: 0.0 };
        let mut profile = Profile::new(Path::Line { point: [1.0, 0.0, 1.0], direction: [0.0, 1.0, 0.0] }).unwrap();
        profile.supports = vec![plane([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]), plane([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])];
        profile.radius = 1.0;
        profile.angle = TAU / 4.0;
        let blend = Surface { kind: Kind::Blend, frame: Frame::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]), positive: true, nurbs: None, profile: Some(Rc::new(profile)), offset: 0.0 };
        // from the floor (v = 0) to the wall (v = 1), a quarter circle about the spine
        assert!(dist(blend.eval(2.0, 0.0), [1.0, 2.0, 0.0]) < 1e-12 && dist(blend.eval(2.0, 1.0), [0.0, 2.0, 1.0]) < 1e-12);
        let middle = blend.eval(0.5, 0.5);
        assert!(dist(middle, [1.0 - 0.5f64.sqrt(), 0.5, 1.0 - 0.5f64.sqrt()]) < 1e-12, "{middle:?}");
        for (u, v) in [(0.3, 0.2), (-4.0, 0.9), (7.5, 0.5)] {
            let [iu, iv] = blend.invert(blend.eval(u, v));
            assert!((iu - u).abs() < 1e-9 && (iv - v).abs() < 1e-9, "{u} {v} -> {iu} {iv}");
            assert!(dot(blend.normal(u, v), numeric_normal(&blend, u, v)) > 1.0 - 1e-6);
        }
        assert_eq!((blend.period_u(), blend.period_v(), blend.curved_in_v()), (None, None, true));
    }

    #[test]
    fn an_offset_surface_lies_along_the_normal() {
        // a cylinder of radius 0.3 moved 0.2 outward is one of radius 0.5, with the same parameters
        let mut s = all()[1].clone();
        s.offset = 0.2;
        let p = s.eval(1.0, 0.7);
        let l = s.frame.local(p);
        assert!((l[0].hypot(l[1]) - 0.5).abs() < 1e-12 && (l[2] - 0.7).abs() < 1e-12);
        let [u, v] = s.invert(p);
        assert!((u - 1.0).abs() < 1e-12 && (v - 0.7).abs() < 1e-12);
        // and a sphere moved inward
        let mut sphere = all()[3].clone();
        sphere.offset = -0.2;
        let [u, v] = sphere.invert(sphere.eval(0.4, 0.9));
        assert!((u - 0.4).abs() < 1e-12 && (v - 0.9).abs() < 1e-12);
    }

    #[test]
    fn poles_and_apex() {
        let s = all();
        assert!(s[3].singular(s[3].eval(1.0, TAU / 4.0)));
        assert!(!s[3].singular(s[3].eval(1.0, 0.3)));
        // cone radius 0.4, tan 0.5: apex at v = -0.8
        assert_eq!(s[2].pole(1, false), Some(-0.8));
        assert_eq!(s[2].pole(1, true), None);
        assert!(s[2].singular(s[2].eval(2.0, -0.8)));
    }
}
