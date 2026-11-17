//! Viewer meshes from a document: one mesh per layer and class, objects pickable by id.
//!
//! Holds the geometry helpers the object readers share (extruded profiles first, as most
//! Vectorworks 3D objects and BIM objects are extrusions of a 2D profile).

use vwx_model::{Document, Mesh, Scene};

/// The meshes of `doc`.
pub fn scene(doc: &Document) -> Scene {
    Scene { info: doc.info.clone(), meshes: Vec::new() }
}

/// Adds the prism of a closed, simple, counter-clockwise polygon `outline` (xy, metres) between
/// heights `z0 < z1` to `mesh`, as object `id`: a bottom and a top fan and the side quads, flat
/// shaded. Convex outlines only; concave ones wait for a triangulator.
pub fn add_prism(mesh: &mut Mesh, id: u64, outline: &[[f64; 2]], z0: f64, z1: f64) {
    let n = outline.len();
    if n < 3 {
        return;
    }
    let first = mesh.indices.len() as u32;
    let mut face = |pts: &[[f64; 3]], normal: [f32; 3]| {
        let base = (mesh.positions.len() / 3) as u32;
        for p in pts {
            mesh.positions.extend(p.map(|c| c as f32));
            mesh.normals.extend(normal);
        }
        for k in 1..pts.len() as u32 - 1 {
            mesh.indices.extend([base, base + k, base + k + 1]);
        }
    };
    let bottom: Vec<[f64; 3]> = outline.iter().rev().map(|p| [p[0], p[1], z0]).collect();
    let top: Vec<[f64; 3]> = outline.iter().map(|p| [p[0], p[1], z1]).collect();
    face(&bottom, [0.0, 0.0, -1.0]);
    face(&top, [0.0, 0.0, 1.0]);
    for k in 0..n {
        let (a, b) = (outline[k], outline[(k + 1) % n]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            continue;
        }
        let normal = [(dy / len) as f32, (-dx / len) as f32, 0.0];
        face(&[[a[0], a[1], z0], [b[0], b[1], z0], [b[0], b[1], z1], [a[0], a[1], z1]], normal);
    }
    mesh.elements.push((id, first, mesh.indices.len() as u32 - first));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prism_of_a_square_is_a_closed_box() {
        let mut m = Mesh::default();
        add_prism(&mut m, 9, &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], 0.0, 2.0);
        assert_eq!(m.indices.len() / 3, 12);
        assert_eq!(m.elements, vec![(9, 0, 36)]);
        assert_eq!(m.positions.len(), m.normals.len());
        // every side normal points away from the centre
        for v in 8..24 {
            let (p, n) = (&m.positions[v * 3..v * 3 + 2], &m.normals[v * 3..v * 3 + 2]);
            assert!((p[0] - 0.5) * n[0] + (p[1] - 0.5) * n[1] > 0.0);
        }
    }
}
