//! Triangles of a simple polygon (ear clipping), convex or not.

type P2 = [f64; 2];

fn cross(o: P2, a: P2, b: P2) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

/// Twice the signed area: positive for a counter-clockwise polygon.
pub fn area2(poly: &[P2]) -> f64 {
    (0..poly.len()).map(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        a[0] * b[1] - b[0] * a[1]
    }).sum()
}

fn inside(p: P2, a: P2, b: P2, c: P2) -> bool {
    cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0
}

/// The triangles of `poly` as indexes into it, counter-clockwise whatever the direction of the
/// polygon. A polygon that crosses itself still gives triangles, of the right count.
pub fn triangulate(poly: &[P2]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let mut idx: Vec<usize> = (0..n).collect();
    if area2(poly) < 0.0 {
        idx.reverse();
    }
    let mut out = Vec::with_capacity(n - 2);
    while idx.len() > 3 {
        let m = idx.len();
        let ear = (0..m).find(|&k| {
            let (a, b, c) = (poly[idx[(k + m - 1) % m]], poly[idx[k]], poly[idx[(k + 1) % m]]);
            // convex corner with no other point of the polygon in it
            cross(a, b, c) > 0.0 && !idx.iter().any(|&j| {
                let p = poly[j];
                p != a && p != b && p != c && inside(p, a, b, c)
            })
        });
        // no ear (a degenerate or crossing polygon): cut any corner, so the count stays right
        let k = ear.unwrap_or(0);
        out.push([idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]]);
        idx.remove(k);
    }
    out.push([idx[0], idx[1], idx[2]]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(poly: &[P2], tris: &[[usize; 3]]) -> f64 {
        tris.iter().map(|t| cross(poly[t[0]], poly[t[1]], poly[t[2]]) / 2.0).sum()
    }

    #[test]
    fn square_either_direction() {
        let sq = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let cw: Vec<P2> = sq.iter().rev().copied().collect();
        for poly in [&sq[..], &cw[..]] {
            let t = triangulate(poly);
            assert_eq!(t.len(), 2);
            assert!((area(poly, &t) - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn concave_l_shape() {
        let l = [[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0], [1.0, 2.0], [0.0, 2.0]];
        let t = triangulate(&l);
        assert_eq!(t.len(), 4);
        assert!((area(&l, &t) - 3.0).abs() < 1e-12);
        assert!(t.iter().all(|t| cross(l[t[0]], l[t[1]], l[t[2]]) > 0.0));
    }
}
