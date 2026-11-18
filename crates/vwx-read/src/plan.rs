//! The plan of each design layer: the 2D line work Vectorworks shows in the top/plan view. Beside
//! the 2D shapes drawn on the layer itself, objects that are 3D as well (walls, doors, windows,
//! symbols, other plug-in objects) keep 2D shapes for their plan view; the viewer meshes leave
//! those out beside the 3D form, the plan shows them.
//!
//! Output: metres, x right and y up, as seen from above; each segment with the object of the
//! layer it belongs to.

use vwx_model::Plan;

use crate::raw::{self, kind, Raw};
use crate::shapes::{self, Affine, P2};

/// Millimetres to metres.
const SCALE: f64 = 0.001;
/// How deep objects may nest in each other (symbols in symbols, groups in groups).
const MAX_DEPTH: usize = 16;
/// Slab, roof edge, roof and roof element: objects with their own 2D shapes among their parts.
const OTHER_HOLDERS: [u8; 4] = [71, 82, 83, 87];

/// The plans of the design layers that have 2D line work, in the order of the layers.
pub fn plans(raw: &Raw) -> Vec<Plan> {
    let mut out = Vec::new();
    for (i, layer) in raw.records.iter().enumerate() {
        if !raw::is_design_layer(layer) {
            continue;
        }
        let mut plan = Plan { layer: raw::layer_name(layer.bytes).unwrap_or_default(), ..Plan::default() };
        for &child in &raw.children[i] {
            // what is set into a wall (a door, a window) is an object of its own
            let inserts: Vec<usize> = if raw.records[child].kind() == kind::WALL {
                raw.children[child].iter().copied().filter(|&c| matches!(raw.records[c].kind(), kind::PLUG_IN_OBJECT | kind::SYMBOL)).collect()
            } else {
                Vec::new()
            };
            let start = plan.lines.len();
            if inserts.is_empty() {
                walk(raw, child, &Affine::IDENTITY, 0, &mut plan.lines);
            } else {
                for &part in raw.children[child].iter().filter(|c| !inserts.contains(c)) {
                    walk(raw, part, &Affine::IDENTITY, 1, &mut plan.lines);
                }
            }
            element(&mut plan, child, start);
            for insert in inserts {
                let start = plan.lines.len();
                walk(raw, insert, &Affine::IDENTITY, 1, &mut plan.lines);
                element(&mut plan, insert, start);
            }
        }
        if !plan.lines.is_empty() {
            out.push(plan);
        }
    }
    out
}

/// Records the segments from `start` on as those of object `i`.
fn element(plan: &mut Plan, i: usize, start: usize) {
    if plan.lines.len() > start {
        plan.elements.push((i as u64, start as u32, (plan.lines.len() - start) as u32));
    }
}

/// True for a group that holds what cuts the hole of a window or door out of its wall (user data
/// tagged `gchw` or `ggcw`): no part of the plan.
fn cuts_wall(raw: &Raw, group: usize) -> bool {
    raw.children[group].iter().map(|&t| &raw.records[t]).any(|t| t.kind() == kind::USER_DATA && matches!(t.bytes.get(0x50..0x54), Some(b"gchw" | b"ggcw")))
}

/// Adds the 2D line work of record `i` (and of what it holds) placed by `place`.
fn walk(raw: &Raw, i: usize, place: &Affine, depth: usize, out: &mut Vec<[[f32; 2]; 2]>) {
    if depth >= MAX_DEPTH {
        return;
    }
    let r = &raw.records[i];
    match r.kind() {
        kind::LINE | kind::POLYGON | kind::POLYLINE | kind::RECTANGLE | kind::ROUNDED_RECTANGLE | kind::ARC | kind::OVAL => {
            let class = raw.by_ref.get(&r.class_id()).map(|&c| &raw.records[c]).filter(|c| c.kind() == kind::CLASS);
            if shapes::stroked(r, class) {
                segments(r, place, out);
            }
        }
        kind::GROUP if cuts_wall(raw, i) => {}
        kind::GROUP | kind::WALL => {
            for &c in &raw.children[i] {
                walk(raw, c, place, depth + 1, out);
            }
        }
        kind::SYMBOL => {
            let Some(instance) = shapes::instance(r) else { return };
            let Some(&definition) = raw.by_ref.get(&instance.definition) else { return };
            if raw.records[definition].kind() != kind::SYMBOL_DEFINITION {
                return;
            }
            let place = instance.placement.then(place);
            for &c in &raw.children[definition] {
                walk(raw, c, &place, depth + 1, out);
            }
        }
        kind::PLUG_IN_OBJECT => {
            let place = shapes::plug_in_placement(r).map_or(*place, |p| p.then(place));
            for &c in &raw.children[i] {
                walk(raw, c, &place, depth + 1, out);
            }
        }
        k if OTHER_HOLDERS.contains(&k) => {
            for &c in &raw.children[i] {
                walk(raw, c, place, depth + 1, out);
            }
        }
        // 3D objects (their profiles are no plan), texts, records, user data
        _ => {}
    }
}

/// The segments of a 2D shape, placed, in metres.
fn segments(r: &vwx_core::container::Record, place: &Affine, out: &mut Vec<[[f32; 2]; 2]>) {
    let at = |p: P2| {
        let q = place.apply([p[0], p[1], 0.0]);
        [(q[0] * SCALE) as f32, (q[1] * SCALE) as f32]
    };
    let finite = |s: &[P2; 2]| s.iter().flatten().all(|c| c.is_finite());
    if let Some(vertices) = (r.kind() == kind::POLYLINE).then(|| shapes::polyline(r)).flatten() {
        // a polyline: its sides that are drawn (those to its holes are not)
        out.extend(shapes::polyline_segments(&vertices, shapes::closed(r)).iter().filter(|s| finite(s)).map(|[a, b]| [at(*a), at(*b)]));
        return;
    }
    let outline = shapes::outline(r);
    let points: Vec<P2> = match r.kind() {
        kind::LINE => shapes::line(r).map(|l| l.to_vec()).unwrap_or_default(),
        _ => outline.clone().or_else(|| shapes::two_points(r)).or_else(|| shapes::arc(r)).unwrap_or_default(),
    };
    if points.len() < 2 || points.iter().flatten().any(|c| !c.is_finite()) {
        return;
    }
    out.extend(points.windows(2).map(|w| [at(w[0]), at(w[1])]));
    if r.kind() != kind::LINE && shapes::closed(r) && outline.is_some() && points.len() > 2 {
        out.push([at(points[points.len() - 1]), at(points[0])]);
    }
}
