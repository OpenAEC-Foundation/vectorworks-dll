//! Viewer meshes of a document: one mesh per layer and class, each object pickable.
//!
//! What is drawn so far (`docs/ARCHITECTURE.md` has the list of what is not):
//!
//! - extrudes of rectangles, ovals, circles and polygons;
//! - 3D polygons and meshes;
//! - CSG solids, walls, sweeps, multiple extrudes and NURBS surfaces from the Parasolid body the
//!   file keeps for them (`vwx-xt`); faces on surfaces that are not evaluated yet (B-surfaces,
//!   blends) are filled flat;
//! - rectangles, ovals, circles and polygons drawn on a layer itself, as flat faces at the
//!   layer's elevation;
//! - placed symbols (the content of their definition), groups, and whatever of the above sits
//!   inside other objects. Inside a symbol or another object 2D shapes are its plan view and are
//!   left out.
//!
//! A document with nothing drawn on its layers (a symbol library) shows its symbol definitions
//! side by side instead, each pickable under its name.
//!
//! Output: metres, Z up.

use std::collections::BTreeMap;

use vwx_model::{Info, Mesh, Scene};

use crate::raw::{self, kind, Raw};
use crate::shapes::{self, Affine, P2, P3};
use crate::tri;

/// Millimetres to metres.
const SCALE: f64 = 0.001;
/// Deepest nesting of symbols in symbols that is followed (a definition that places itself
/// would never end).
const MAX_DEPTH: usize = 24;
/// Fill of objects whose colour is not known.
const DEFAULT_COLOR: [u8; 3] = [200, 200, 204];

/// The meshes of a document.
pub fn scene(raw: &Raw, info: Info) -> Scene {
    let mut builder = Builder { raw, batches: BTreeMap::new(), elevation: 0.0 };
    for (i, r) in raw.records.iter().enumerate() {
        if r.kind() != kind::LAYER {
            continue;
        }
        // 3D objects hold their height in the document; 2D shapes lie on their layer
        builder.elevation = raw::layer_elevation(r.bytes);
        for &child in &raw.children[i] {
            let key = (i, raw.records[child].class_id());
            let start = builder.batch(key).mesh.indices.len() as u32;
            builder.walk(child, &Affine::IDENTITY, false, key, 0);
            let count = builder.batch(key).mesh.indices.len() as u32 - start;
            if count > 0 {
                let props = builder.props(child, i);
                let mesh = &mut builder.batch(key).mesh;
                mesh.elements.push((child as u64, start, count));
                mesh.props.push((child as u64, props));
            }
        }
    }
    // a library file places nothing on its layers: its symbols are what there is to see
    if builder.batches.values().all(|b| b.mesh.indices.is_empty()) {
        builder.library();
    }
    let meshes = builder.batches.into_values().map(|b| b.mesh).filter(|m| !m.indices.is_empty()).collect();
    Scene { info, meshes }
}

/// A symbol definition drawn for the library view: its record, its first index and first vertex
/// in the mesh, and its lowest and highest corner in metres.
type Shown = (usize, usize, usize, [f32; 3], [f32; 3]);

/// The batch that holds the symbol definitions of a library file.
const LIBRARY: (usize, u32) = (usize::MAX, 0);

struct Batch {
    mesh: Mesh,
}

struct Builder<'a, 'b> {
    raw: &'b Raw<'a>,
    /// By (layer record, class reference number).
    batches: BTreeMap<(usize, u32), Batch>,
    /// Elevation of the layer being drawn (millimetres).
    elevation: f64,
}

impl Builder<'_, '_> {
    fn batch(&mut self, key: (usize, u32)) -> &mut Batch {
        let raw = self.raw;
        self.batches.entry(key).or_insert_with(|| {
            let layer = match key {
                LIBRARY => "Symbols".into(),
                _ => raw::layer_name(raw.records[key.0].bytes).unwrap_or_else(|| "Layer".into()),
            };
            let name = match raw.names.get(&key.1) {
                Some(class) => format!("{layer} / {class}"),
                None => layer,
            };
            Batch { mesh: Mesh { name, color: DEFAULT_COLOR.map(|c| c as f32 / 255.0), colors: Some(Vec::new()), ..Mesh::default() } }
        })
    }

    /// Draws every symbol definition of the document, side by side in rows, each pickable under
    /// its name. A definition with nothing in 3D shows its plan view.
    fn library(&mut self) {
        let raw = self.raw;
        self.elevation = 0.0;
        let mut items: Vec<Shown> = Vec::new();
        for d in (0..raw.records.len()).filter(|&i| raw.records[i].kind() == kind::SYMBOL_DEFINITION) {
            let (index, vertex) = {
                let mesh = &self.batch(LIBRARY).mesh;
                (mesh.indices.len(), mesh.positions.len() / 3)
            };
            for nested in [true, false] {
                for &c in &raw.children[d] {
                    self.walk(c, &Affine::IDENTITY, nested, LIBRARY, 1);
                }
                if self.batch(LIBRARY).mesh.indices.len() > index {
                    break;
                }
            }
            let mesh = &self.batch(LIBRARY).mesh;
            if mesh.indices.len() == index {
                continue;
            }
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in mesh.positions[vertex * 3..].as_chunks::<3>().0 {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            items.push((d, index, vertex, lo, hi));
        }
        if items.is_empty() {
            return;
        }
        // rows of about equal length and depth, a gap of a tenth of the average size between
        let size = |lo: &[f32; 3], hi: &[f32; 3], k: usize| (hi[k] - lo[k]).max(0.0);
        let mean = items.iter().map(|(_, _, _, lo, hi)| size(lo, hi, 0).max(size(lo, hi, 1))).sum::<f32>() / items.len() as f32;
        let gap = (mean * 0.1).max(0.05);
        let area: f32 = items.iter().map(|(_, _, _, lo, hi)| (size(lo, hi, 0) + gap) * (size(lo, hi, 1) + gap)).sum();
        let widest = items.iter().map(|(_, _, _, lo, hi)| size(lo, hi, 0)).fold(0.0, f32::max);
        let row_length = (area.sqrt() * 1.3).max(widest);
        let (mut x, mut y, mut depth) = (0.0f32, 0.0f32, 0.0f32);
        let ends: Vec<usize> = items.iter().skip(1).map(|item| item.2).collect();
        let mesh = &mut self.batch(LIBRARY).mesh;
        let last = mesh.positions.len() / 3;
        for (n, (d, index, vertex, lo, hi)) in items.iter().enumerate() {
            let (w, deep) = (size(lo, hi, 0), size(lo, hi, 1));
            if x > 0.0 && x + w > row_length {
                x = 0.0;
                y -= depth + gap;
                depth = 0.0;
            }
            // the symbol keeps its height; its lowest corner in plan goes to (x, y - depth)
            let shift = [x - lo[0], y - hi[1]];
            let end = ends.get(n).copied().unwrap_or(last);
            for p in mesh.positions[vertex * 3..end * 3].as_chunks_mut::<3>().0 {
                p[0] += shift[0];
                p[1] += shift[1];
            }
            x += w + gap;
            depth = depth.max(deep);
            let count = ends.get(n).map_or(mesh.indices.len(), |_| items[n + 1].1) - index;
            let mut map = serde_json::Map::new();
            if let Some(name) = raw.name(*d) {
                map.insert("Name".into(), name.into());
            }
            map.insert("category".into(), crate::kinds::name(kind::SYMBOL_DEFINITION).into());
            mesh.elements.push((*d as u64, *index as u32, count as u32));
            mesh.props.push((*d as u64, serde_json::Value::Object(map).to_string()));
        }
    }

    /// The properties of an object as a flat JSON object.
    fn props(&self, i: usize, layer: usize) -> String {
        let r = &self.raw.records[i];
        let mut map = serde_json::Map::new();
        if let Some(name) = self.raw.name(i) {
            map.insert("Name".into(), name.into());
        }
        map.insert("category".into(), crate::kinds::name(r.kind()).into());
        if let Some(layer) = raw::layer_name(self.raw.records[layer].bytes) {
            map.insert("Layer".into(), layer.into());
        }
        if let Some(class) = self.raw.names.get(&r.class_id()) {
            map.insert("Class".into(), class.clone().into());
        }
        if r.kind() == kind::SYMBOL {
            if let Some(symbol) = shapes::instance(r).and_then(|s| self.raw.names.get(&s.definition)) {
                map.insert("Symbol".into(), symbol.clone().into());
            }
        }
        serde_json::Value::Object(map).to_string()
    }

    /// Draws record `i` and what is inside it. `nested` is true inside a symbol or another
    /// object, where 2D shapes are the plan view.
    fn walk(&mut self, i: usize, place: &Affine, nested: bool, key: (usize, u32), depth: usize) {
        let raw = self.raw;
        let r = &raw.records[i];
        let color = shapes::fill_index(r).and_then(|k| raw.palette.color(k)).unwrap_or(DEFAULT_COLOR);
        match r.kind() {
            kind::END | kind::USER_DATA => {}
            // the children of a CSG solid are its operands; the result is its Parasolid body
            kind::CSG_SOLID | kind::SWEEP | kind::MULTIPLE_EXTRUDE | kind::NURBS_SURFACE => {
                self.body(i, place, key, color, true);
            }
            kind::EXTRUDE => {
                // the exact body when the file has one and all of it is read
                if self.body(i, place, key, color, false) {
                    return;
                }
                let Some(e) = shapes::extrude(r) else { return };
                let place = e.placement.then(place);
                for &c in &raw.children[i] {
                    if let Some(outline) = shapes::outline(&raw.records[c]) {
                        self.prism(key, &outline, e.bottom, e.top, &place, color);
                    }
                }
            }
            kind::POLYGON_3D => {
                if let Some(points) = shapes::polygon_3d(r).filter(|p| p.len() >= 3) {
                    let world: Vec<P3> = points.iter().map(|&p| place.apply(p)).collect();
                    self.face(key, &world, color, false);
                }
            }
            kind::MESH => {
                let Some(points) = shapes::mesh_points(r) else { return };
                for &c in &raw.children[i] {
                    let face = &raw.records[c];
                    if face.kind() != kind::MESH_FACE {
                        continue;
                    }
                    let corners: Option<Vec<P3>> = shapes::mesh_face(face).and_then(|f| f.iter().map(|&k| points.get(k).map(|&p| place.apply(p))).collect());
                    if let Some(corners) = corners.filter(|c| c.len() >= 3) {
                        self.face(key, &corners, color, place.mirrors());
                    }
                }
            }
            kind::RECTANGLE | kind::OVAL | kind::POLYGON | kind::ARC => {
                if nested {
                    return;
                }
                if let Some(outline) = shapes::outline(r) {
                    let outline = counter_clockwise(outline);
                    let z = self.elevation;
                    let world: Vec<P3> = outline.iter().map(|p| place.apply([p[0], p[1], z])).collect();
                    self.face(key, &world, color, place.mirrors());
                }
            }
            kind::SYMBOL => {
                let Some(instance) = shapes::instance(r) else { return };
                let Some(&definition) = raw.by_ref.get(&instance.definition) else { return };
                if depth >= MAX_DEPTH || raw.records[definition].kind() != kind::SYMBOL_DEFINITION {
                    return;
                }
                let place = instance.placement.then(place);
                for &c in &raw.children[definition] {
                    self.walk(c, &place, true, key, depth + 1);
                }
            }
            kind::GROUP => {
                for &c in &raw.children[i] {
                    self.walk(c, place, nested, key, depth);
                }
            }
            // any other object: what it holds is part of it
            _ => {
                for &c in &raw.children[i] {
                    self.walk(c, place, true, key, depth);
                }
            }
        }
    }

    /// Draws the cached solid body of object `i` (its child of type 114). With `approximate`
    /// false a body with faces that are not evaluated yet is left out. True when drawn.
    fn body(&mut self, i: usize, place: &Affine, key: (usize, u32), color: [u8; 3], approximate: bool) -> bool {
        let raw = self.raw;
        let Some(&child) = raw.children[i].iter().find(|&&c| raw.records[c].kind() == kind::BODY) else { return false };
        let Some(body) = shapes::body(&raw.records[child]) else { return false };
        if !approximate && body.approximate > 0 {
            return false;
        }
        // the body is in the coordinates of the object when it fills the object's own box, else
        // in those the object is placed in
        let own = shapes::solid_frame(&raw.records[i]).filter(|(_, bounds)| {
            let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            for p in &body.positions {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            let size = (0..3).map(|k| hi[k] - lo[k]).fold(1.0, f64::max);
            (0..3).all(|k| (lo[k] - bounds[0][k]).abs() < 1e-3 * size && (hi[k] - bounds[1][k]).abs() < 1e-3 * size)
        });
        let place = match own {
            Some((placement, _)) => placement.then(place),
            None => *place,
        };
        let flip = place.mirrors();
        let mesh = &mut self.batch(key).mesh;
        let base = (mesh.positions.len() / 3) as u32;
        for (p, n) in body.positions.iter().zip(&body.normals) {
            mesh.positions.extend(place.apply(*p).map(|c| (c * SCALE) as f32));
            let n = place.direction(*n);
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12) * if flip { -1.0 } else { 1.0 };
            mesh.normals.extend(n.map(|c| (c / len) as f32));
        }
        if let Some(colors) = mesh.colors.as_mut() {
            colors.extend(std::iter::repeat_n([color[0], color[1], color[2], 255], body.positions.len()));
        }
        for t in &body.triangles {
            let t = if flip { [t[0], t[2], t[1]] } else { *t };
            mesh.indices.extend(t.map(|k| base + k));
        }
        true
    }

    /// The prism of a closed outline in the local xy plane between two heights.
    fn prism(&mut self, key: (usize, u32), outline: &[P2], bottom: f64, top: f64, place: &Affine, color: [u8; 3]) {
        let outline = counter_clockwise(outline.to_vec());
        let (z0, z1) = if bottom <= top { (bottom, top) } else { (top, bottom) };
        if outline.len() < 3 || z0 == z1 {
            return;
        }
        let flip = place.mirrors();
        let at = |p: P2, z: f64| place.apply([p[0], p[1], z]);
        let low: Vec<P3> = outline.iter().rev().map(|&p| at(p, z0)).collect();
        let high: Vec<P3> = outline.iter().map(|&p| at(p, z1)).collect();
        self.face(key, &low, color, flip);
        self.face(key, &high, color, flip);
        for k in 0..outline.len() {
            let (a, b) = (outline[k], outline[(k + 1) % outline.len()]);
            if a != b {
                self.face(key, &[at(a, z0), at(b, z0), at(b, z1), at(a, z1)], color, flip);
            }
        }
    }

    /// One flat face from its corners in order (millimetres); `flip` turns it over.
    fn face(&mut self, key: (usize, u32), corners: &[P3], color: [u8; 3], flip: bool) {
        let Some(normal) = newell(corners) else { return };
        // the triangles are found in the plane of the face: drop the axis the normal is closest to
        let drop = (0..3).max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs())).unwrap_or(2);
        let (u, v) = ((drop + 1) % 3, (drop + 2) % 3);
        let flat: Vec<P2> = corners.iter().map(|p| if normal[drop] >= 0.0 { [p[u], p[v]] } else { [p[v], p[u]] }).collect();
        let triangles = tri::triangulate(&flat);
        let sign = if flip { -1.0 } else { 1.0 };
        let mesh = &mut self.batch(key).mesh;
        let base = (mesh.positions.len() / 3) as u32;
        for p in corners {
            mesh.positions.extend(p.map(|c| (c * SCALE) as f32));
            mesh.normals.extend(normal.map(|c| (c * sign) as f32));
        }
        if let Some(colors) = mesh.colors.as_mut() {
            colors.extend(std::iter::repeat_n([color[0], color[1], color[2], 255], corners.len()));
        }
        for t in triangles {
            let t = if flip { [t[0], t[2], t[1]] } else { t };
            mesh.indices.extend(t.map(|k| base + k as u32));
        }
    }
}

fn counter_clockwise(mut outline: Vec<P2>) -> Vec<P2> {
    if tri::area2(&outline) < 0.0 {
        outline.reverse();
    }
    outline
}

/// The unit normal of a polygon (Newell), `None` for one without area.
fn newell(p: &[P3]) -> Option<P3> {
    let mut n = [0.0f64; 3];
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    (len > 1e-9).then(|| n.map(|c| c / len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::tests::record;
    use vwx_core::container::{Unpacked, RECORD_HEADER};
    use vwx_core::Header;

    /// Sets the parent address (0x26) and the reference and class numbers of a record.
    fn tagged(mut r: Vec<u8>, parent: u64, reference: u32, class: u32) -> Vec<u8> {
        r[0x26..0x2e].copy_from_slice(&parent.to_le_bytes());
        r[0x1a..0x1e].copy_from_slice(&reference.to_le_bytes());
        r[0x20..0x24].copy_from_slice(&class.to_le_bytes());
        r
    }

    fn layer(name: &str, elevation: f64) -> Vec<u8> {
        let text: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let mut r = record(kind::LAYER, 0x2a2, &[(0x20e, elevation)]);
        r.extend((text.len() as u32).to_le_bytes());
        r.extend(&text);
        r.extend([0u8; 8]);
        let n = r.len() as u64;
        r[0..8].copy_from_slice(&n.to_le_bytes());
        r
    }

    fn names(list: &[(u32, &str)]) -> Vec<u8> {
        let mut r = record(kind::NAMES, RECORD_HEADER + 20, &[]);
        for (id, name) in list {
            let text: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
            r.extend(id.to_le_bytes());
            r.extend((text.len() as u32).to_le_bytes());
            r.extend(text);
        }
        let n = r.len() as u64;
        r[0..8].copy_from_slice(&n.to_le_bytes());
        r
    }

    fn unpacked(records: Vec<Vec<u8>>) -> Unpacked {
        let mut head = vec![0u8; 0x94];
        head[8..12].copy_from_slice(&0x94u32.to_be_bytes());
        head[14..32].copy_from_slice(b"VW32.0.0 VW32.0.0\0");
        Unpacked { header: Header::parse(&head).unwrap(), document: Vec::new(), body: records.concat(), blocks: 1 }
    }

    /// The box of sample 20: Rect(0, 500, 1000, 0) extruded 0..3000.
    fn box_records(parent: u64, own: u64) -> Vec<Vec<u8>> {
        let extrude = record(kind::EXTRUDE, 324, &[(0xcc, 1.0), (0xec, 1.0), (0x10c, 1.0), (0x138, 3000.0)]);
        let rect = record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]);
        vec![tagged(extrude, parent, 0, 27), tagged(rect, own, 0, 0), tagged(record(kind::END, 88, &[]), own, 0, 0)]
    }

    fn bounds(mesh: &Mesh) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in mesh.positions.chunks(3) {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (lo, hi)
    }

    #[test]
    fn extruded_rectangle_on_a_raised_layer() {
        let mut records = vec![tagged(names(&[(27, "Muren")]), 1, 0, 0), tagged(layer("Verdieping 1", 3000.0), 1, 42, 0)];
        let mut on_layer = box_records(2, 3);
        // an object made on a raised layer holds that height in its own placement (sample 105)
        on_layer[0][0x124..0x12c].copy_from_slice(&3000f64.to_le_bytes());
        records.extend(on_layer);
        records.push(tagged(record(kind::END, 88, &[]), 2, 0, 0));
        let u = unpacked(records);
        let raw = Raw::new(&u).unwrap();
        let scene = scene(&raw, Info::default());
        assert_eq!(scene.meshes.len(), 1);
        let m = &scene.meshes[0];
        assert_eq!(m.name, "Verdieping 1 / Muren");
        assert_eq!(m.indices.len() / 3, 12);
        assert_eq!(m.elements, vec![(2, 0, 36)]);
        assert_eq!(bounds(m), ([0.0, 0.0, 3.0], [1.0, 0.5, 6.0]));
        assert!(m.props[0].1.contains(r#""category":"extrude""#) && m.props[0].1.contains(r#""Class":"Muren""#), "{}", m.props[0].1);
        // every normal points out of the box
        for (p, n) in m.positions.chunks(3).zip(m.normals.chunks(3)) {
            let out = [(p[0] - 0.5) * n[0], (p[1] - 0.25) * n[1], (p[2] - 4.5) * n[2]];
            assert!(out.iter().sum::<f32>() > 0.0);
        }
    }

    #[test]
    fn placed_symbol_draws_its_definition() {
        // definition 30 (address 9) holds the box; placed at (3000, 1000) turned 90 degrees
        let mut records = vec![tagged(names(&[(30, "Blok")]), 1, 0, 0), tagged(record(kind::SYMBOL_DEFINITION, 372, &[]), 1, 30, 0)];
        records.extend(box_records(9, 10));
        records.push(tagged(record(kind::END, 88, &[]), 9, 0, 0));
        records.push(tagged(layer("Ontwerplaag-1", 0.0), 1, 1, 0));
        let mut symbol = record(kind::SYMBOL, 280, &[(0x9e, 1.0), (0xae, -1.0), (0xd6, 1.0), (0xde, 3000.0), (0xe6, 1000.0), (0x100, 1.0), (0x108, 1.0), (0x110, 1.0)]);
        symbol[0x8e..0x92].copy_from_slice(&30u32.to_le_bytes());
        records.push(tagged(symbol, 20, 0, 0));
        records.push(tagged(record(kind::END, 88, &[]), 20, 0, 0));
        let u = unpacked(records);
        let raw = Raw::new(&u).unwrap();
        let scene = scene(&raw, Info::default());
        let m = &scene.meshes[0];
        assert_eq!(m.indices.len() / 3, 12);
        assert_eq!(bounds(m), ([2.5, 1.0, 0.0], [3.0, 2.0, 3.0]));
        assert!(m.props[0].1.contains(r#""Symbol":"Blok""#), "{}", m.props[0].1);
    }

    #[test]
    fn a_library_file_shows_its_symbols_side_by_side() {
        // two definitions holding the same box, nothing on the layer
        let mut records = vec![tagged(names(&[(30, "Blok"), (31, "Kast")]), 1, 0, 0)];
        for (reference, address) in [(30, 9), (31, 12)] {
            records.push(tagged(record(kind::SYMBOL_DEFINITION, 372, &[]), 1, reference, 0));
            records.extend(box_records(address, address + 1));
            records.push(tagged(record(kind::END, 88, &[]), address, 0, 0));
        }
        records.push(tagged(layer("Ontwerplaag-1", 0.0), 1, 1, 0));
        let u = unpacked(records);
        let raw = Raw::new(&u).unwrap();
        let scene = scene(&raw, Info::default());
        assert_eq!(scene.meshes.len(), 1);
        let m = &scene.meshes[0];
        assert_eq!(m.name, "Symbols");
        assert_eq!(m.indices.len() / 3, 24);
        assert_eq!(m.elements.iter().map(|e| (e.1, e.2)).collect::<Vec<_>>(), [(0, 36), (36, 36)]);
        assert!(m.props[0].1.contains(r#""Name":"Blok""#) && m.props[1].1.contains(r#""Name":"Kast""#), "{:?}", m.props);
        // boxes of 1 x 0.5 m: two rows (rows are about as long as they are deep together), the
        // second a gap of 0.1 m behind the first, each at its own height
        let (lo, hi) = bounds(m);
        assert_eq!((lo[0], hi[0], hi[1], lo[2], hi[2]), (0.0, 1.0, 0.0, 0.0, 3.0));
        assert!((lo[1] + 1.1).abs() < 1e-6, "{lo:?}");
    }

    #[test]
    fn rectangle_on_a_layer_is_a_flat_face() {
        let rect = record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]);
        let records = vec![tagged(layer("L", 3000.0), 1, 1, 0), tagged(rect, 2, 0, 0), tagged(record(kind::END, 88, &[]), 2, 0, 0)];
        let u = unpacked(records);
        let raw = Raw::new(&u).unwrap();
        let scene = scene(&raw, Info::default());
        assert_eq!(scene.meshes[0].indices.len() / 3, 2);
        assert_eq!(&scene.meshes[0].normals[..3], &[0.0, 0.0, 1.0]);
        // on the elevation of its layer
        assert_eq!(scene.meshes[0].positions[2], 3.0);
    }
}
