//! Objects read so that nothing of a model is left out (see `crate::coverage`): loci and the
//! marker they are drawn with, 3D polygons as lines.
//!
//! Lengths are millimetres; 2D points are returned as [x, y].

use vwx_core::container::Record;
use vwx_core::le;

use crate::shapes::{P2, P3};

/// Half the length of each arm of the cross a locus is drawn with (millimetres). Vectorworks
/// draws a locus as a cross of a fixed size on screen; a model has no screen, so the cross has
/// a fixed size in the model.
pub const LOCUS_ARM: f64 = 50.0;

fn real(r: &Record, at: usize) -> Option<f64> {
    le::f64(r.bytes, at).filter(|v| v.is_finite())
}

/// The point of a locus (type 17): (y, x) at 0x68 (sample 18; sample 401: a locus made at
/// (100, 200) has 200 at 0x68 and 100 at 0x70).
pub fn locus(r: &Record) -> Option<P2> {
    Some([real(r, 0x70)?, real(r, 0x68)?])
}

/// The point of a 3D locus (type 9, 126 bytes): x, y, z at 0x66 (samples 402 and 403: loci made
/// at (1000, 2000, 3000) and (-500, 250, -75) have those three numbers there, in that order). Its
/// height is in the document, as that of other 3D objects.
pub fn locus_3d(r: &Record) -> Option<P3> {
    Some([real(r, 0x66)?, real(r, 0x6e)?, real(r, 0x76)?])
}

/// The line segments of the cross a locus at `at` is drawn with: along x and y, and with `up`
/// also along z.
pub fn cross(at: P3, up: bool) -> Vec<[P3; 2]> {
    let arm = |k: usize| {
        let (mut a, mut b) = (at, at);
        a[k] -= LOCUS_ARM;
        b[k] += LOCUS_ARM;
        [a, b]
    };
    (0..if up { 3 } else { 2 }).map(arm).collect()
}

/// True for a 3D polygon (type 25) whose last point is joined to its first: the byte at 0x8d, as
/// for a polygon (sample 407: 1 for the squares; 0 for a polygon of two points, a 3D line).
pub fn polygon_3d_closed(r: &Record) -> bool {
    r.bytes.get(0x8d) != Some(&0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::raw::kind;
    use crate::shapes::tests::record;

    /// A 3D locus at `p`, with a pen.
    pub(crate) fn locus_3d_record(p: P3) -> Vec<u8> {
        let mut r = record(kind::LOCUS_3D, 126, &[(0x66, p[0]), (0x6e, p[1]), (0x76, p[2])]);
        r[0x50] = 2;
        r
    }

    #[test]
    fn loci_as_samples_401_to_403() {
        let r = locus_3d_record([-500.0, 250.0, -75.0]);
        let rec = Record { offset: 0, bytes: &r };
        assert_eq!(locus_3d(&rec), Some([-500.0, 250.0, -75.0]));
        let r = record(kind::LOCUS, 120, &[(0x68, 200.0), (0x70, 100.0)]);
        assert_eq!(locus(&Record { offset: 0, bytes: &r }), Some([100.0, 200.0]));
        assert_eq!(cross([0.0; 3], true).len(), 3);
        assert_eq!(cross([0.0; 3], false)[1], [[0.0, -LOCUS_ARM, 0.0], [0.0, LOCUS_ARM, 0.0]]);
    }

    use crate::coverage::Status;
    use crate::mesh::tests::{layer, names, pen, tagged, unpacked};
    use crate::raw::Raw;
    use vwx_model::{Info, Scene};

    /// The scene of `records` and the status of each object of its design layers by record.
    fn read(records: Vec<Vec<u8>>) -> (Scene, Vec<(usize, Status)>) {
        let u = unpacked(records);
        let raw = Raw::new(&u).unwrap();
        let (scene, drawn) = crate::mesh::scene_and_drawn(&raw, Info::default());
        let texts = scene.texts.iter().map(|t| t.id as usize).collect();
        let statuses = crate::coverage::entries(&raw, &drawn, &texts).into_iter().map(|e| (e.record, e.status)).collect();
        (scene, statuses)
    }

    #[test]
    fn a_3d_locus_is_a_cross_of_three_lines() {
        let records = vec![tagged(layer("L", 0.0), 1, 1, 0), tagged(locus_3d_record([1000.0, 2000.0, 3000.0]), 2, 0, 0), tagged(record(kind::END, 88, &[]), 2, 0, 0)];
        let (scene, statuses) = read(records);
        let m = &scene.meshes[0];
        assert_eq!((m.indices.len(), m.lines.len(), m.line_elements.len()), (0, 6, 1));
        // the arm along z, about the point
        assert_eq!(&m.positions[12..18], &[1.0, 2.0, 2.95, 1.0, 2.0, 3.05]);
        assert_eq!(statuses, [(1, Status::Drawn)]);
    }

    #[test]
    fn a_2d_locus_is_a_flat_cross_on_its_layer() {
        let locus = pen(record(kind::LOCUS, 120, &[(0x68, 200.0), (0x70, 100.0)]));
        let records = vec![tagged(layer("L", 3000.0), 1, 1, 0), tagged(locus, 2, 0, 0), tagged(record(kind::END, 88, &[]), 2, 0, 0)];
        let (scene, _) = read(records);
        let m = &scene.meshes[0];
        assert_eq!(m.lines.len(), 4);
        assert_eq!(&m.positions[..6], &[0.05, 0.2, 3.0, 0.15, 0.2, 3.0]);
    }

    #[test]
    fn a_symbol_with_a_3d_locus_still_shows_its_2d_shapes() {
        // definition 30 holds a 3D locus (its insertion point) and a rectangle without a fill
        let rect = pen(record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]));
        let mut symbol = record(kind::SYMBOL, 280, &[(0x96, 1.0), (0xb6, 1.0), (0xd6, 1.0)]);
        symbol[0x8e..0x92].copy_from_slice(&30u32.to_le_bytes());
        let records = vec![
            tagged(names(&[(30, "Punt")]), 1, 0, 0),
            tagged(record(kind::SYMBOL_DEFINITION, 372, &[]), 1, 30, 0),
            tagged(locus_3d_record([0.0; 3]), 9, 0, 0),
            tagged(rect, 9, 0, 0),
            tagged(record(kind::END, 88, &[]), 9, 0, 0),
            tagged(layer("L", 0.0), 1, 1, 0),
            tagged(symbol, 20, 0, 0),
            tagged(record(kind::END, 88, &[]), 20, 0, 0),
        ];
        let (scene, statuses) = read(records);
        // the cross once (3 segments) and the four sides of the rectangle
        assert_eq!(scene.meshes[0].lines.len(), 2 * (3 + 4));
        assert!(statuses.iter().all(|s| s.1 == Status::Drawn), "{statuses:?}");
    }

    #[test]
    fn a_3d_polygon_without_a_fill_is_its_outline() {
        // a 3D line (two points) with a pen, and a square with neither pen nor fill
        let line = pen(record(kind::POLYGON_3D, 0xd2 + 48, &[(0xd2 + 24, 3000.0), (0xd2 + 40, 1000.0)]));
        let mut line = line;
        line[0xd0] = 2;
        let mut square = record(kind::POLYGON_3D, 0xd2 + 96, &[(0xd2 + 24, 1000.0), (0xd2 + 48, 1000.0), (0xd2 + 56, 1000.0), (0xd2 + 80, 1000.0)]);
        square[0xd0] = 4;
        square[0x8d] = 1;
        let records = vec![tagged(layer("L", 0.0), 1, 1, 0), tagged(line, 2, 0, 0), tagged(square, 2, 0, 0), tagged(record(kind::END, 88, &[]), 2, 0, 0)];
        let (scene, statuses) = read(records);
        let m = &scene.meshes[0];
        assert_eq!((m.indices.len(), m.lines.len()), (0, 2));
        assert_eq!(&m.positions[3..6], &[3.0, 0.0, 1.0]);
        assert_eq!(statuses, [(1, Status::Drawn), (2, Status::Invisible)]);
    }
}
