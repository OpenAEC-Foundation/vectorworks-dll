//! Small vector helpers.

pub type P2 = [f64; 2];
pub type P3 = [f64; 3];

pub fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn mul(a: P3, s: f64) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn len(a: P3) -> f64 {
    dot(a, a).sqrt()
}

pub fn dist(a: P3, b: P3) -> f64 {
    len(sub(a, b))
}

/// `a` at unit length; `a` itself when it has none.
pub fn unit(a: P3) -> P3 {
    let l = len(a);
    if l > 0.0 {
        mul(a, 1.0 / l)
    } else {
        a
    }
}

/// An origin with three orthonormal axes: `z` the main axis (a normal or a rotation axis), `x`
/// the reference direction, `y = z × x`.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub origin: P3,
    pub x: P3,
    pub y: P3,
    pub z: P3,
}

impl Frame {
    pub fn new(origin: P3, z: P3, x: P3) -> Frame {
        let z = unit(z);
        // the reference direction made square to the axis, in case the file's is not quite
        let x = unit(sub(x, mul(z, dot(x, z))));
        Frame { origin, x, y: cross(z, x), z }
    }

    /// The coordinates of `p` in the frame.
    pub fn local(&self, p: P3) -> P3 {
        let d = sub(p, self.origin);
        [dot(d, self.x), dot(d, self.y), dot(d, self.z)]
    }

    /// The point with coordinates `l` in the frame.
    pub fn world(&self, l: P3) -> P3 {
        add(self.origin, self.direction(l))
    }

    /// The direction with components `l` in the frame.
    pub fn direction(&self, l: P3) -> P3 {
        add(add(mul(self.x, l[0]), mul(self.y, l[1])), mul(self.z, l[2]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trip() {
        let f = Frame::new([1.0, 2.0, 3.0], [0.0, 0.0, 2.0], [1.0, 0.0, 0.5]);
        assert_eq!((f.x, f.y, f.z), ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]));
        let p = [4.0, -1.0, 7.0];
        assert_eq!(f.world(f.local(p)), p);
    }
}
