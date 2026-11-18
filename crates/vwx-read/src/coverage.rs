//! Coverage: for every object on a design layer (on the layer itself, and inside groups, placed
//! symbols and plug-in objects), whether [`crate::read`] draws it, so that nothing of a model is
//! left out without notice.
//!
//! An object counts as drawn when drawing it gave triangles or line segments. Objects that are
//! not drawn on their own are sorted by why (a group, symbol or plug-in object that is not drawn
//! takes the status of what it holds):
//!
//! - **part**: a part of a solid that is drawn as one (the profile of an extrude, an operand of a
//!   CSG solid, a boundary of a NURBS surface);
//! - **plan**: a 2D shape of an object that is drawn in 3D: its plan view, left out beside its
//!   3D form;
//! - **text**: a text, which the scene keeps as a text ([`vwx_model::Scene::texts`]);
//! - **cutter**: what cuts the hole of a door or window out of its wall (`gchw`, `ggcw`);
//! - **empty**: a group, symbol or plug-in object with no object inside;
//! - **invisible**: a shape with neither a pen nor a fill (of its own or of its class);
//! - **missing**: none of these: the object is left out.

use std::collections::{HashMap, HashSet};

use crate::raw::{kind, Raw};

/// Why an object is or is not drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Status {
    Drawn,
    Part,
    Plan,
    Text,
    Cutter,
    Empty,
    Invisible,
    Missing,
}

impl Status {
    pub const ALL: [Status; 8] = [Status::Drawn, Status::Part, Status::Plan, Status::Text, Status::Cutter, Status::Empty, Status::Invisible, Status::Missing];

    pub fn name(self) -> &'static str {
        match self {
            Status::Drawn => "drawn",
            Status::Part => "part",
            Status::Plan => "plan",
            Status::Text => "text",
            Status::Cutter => "cutter",
            Status::Empty => "empty",
            Status::Invisible => "invisible",
            Status::Missing => "missing",
        }
    }
}

/// One object of a design layer.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Its record (the object id of the scene).
    pub record: usize,
    pub kind: u8,
    /// Its kind as `24 extrude`, a plug-in object as `86 plug-in: Door`.
    pub label: String,
    pub status: Status,
    /// On an invisible layer or in an invisible class (it is drawn all the same, in a mesh that
    /// starts hidden).
    pub hidden: bool,
    /// The label of the object it is in (`31 layer` for an object on a layer itself).
    pub parent: String,
}

/// Records that are data of their object rather than objects: those whose byte at 0x2f has 2
/// in its low four bits (0x12, 0x22, 0x02: user data, records, solid bodies, the points of a profile, previews, attributes; see
/// `docs/FORMAT.md`), those with nothing after the record header (type 128), and the end of a
/// list, the faces of a mesh and the mesh of a solid.
pub fn is_data(r: &vwx_core::container::Record) -> bool {
    r.bytes.get(0x2f).is_some_and(|b| b & 0x0f == 2)
        || r.bytes.len() <= vwx_core::container::RECORD_HEADER
        || matches!(r.kind(), kind::END | kind::USER_DATA | kind::RECORD | kind::BODY | kind::PROFILE_GRID | kind::MESH_FACE | kind::POLY_MESH | kind::VIEW)
}

/// Objects whose children are the parts of one solid, drawn as a whole.
fn makes_solid(k: u8) -> bool {
    matches!(k, kind::EXTRUDE | kind::SWEEP | kind::MULTIPLE_EXTRUDE | kind::CSG_SOLID | kind::SOLID | kind::MESH | kind::NURBS_SURFACE)
}

/// 2D kinds: line, rectangle, oval, polygon, arc, freehand, text, rounded rectangle, bitmap,
/// locus, polyline, dimension, hatch.
fn is_2d(k: u8) -> bool {
    matches!(k, 2 | 3 | 4 | 5 | 6 | 8 | 10 | 13 | 14 | 17 | 21 | 63 | 66)
}

/// True when record `i` is 2D through and through: a 2D shape, or a group or symbol of them.
fn flat(raw: &Raw, i: usize, depth: usize) -> bool {
    let r = &raw.records[i];
    match r.kind() {
        k if is_2d(k) => true,
        _ if depth > 16 => false,
        kind::GROUP => raw.children[i].iter().filter(|&&c| !is_data(&raw.records[c])).all(|&c| flat(raw, c, depth + 1)),
        kind::SYMBOL => definition(raw, i).is_some_and(|d| raw.children[d].iter().filter(|&&c| !is_data(&raw.records[c])).all(|&c| flat(raw, c, depth + 1))),
        _ => false,
    }
}

/// The symbol definition a placed symbol puts.
fn definition(raw: &Raw, i: usize) -> Option<usize> {
    let d = *raw.by_ref.get(&crate::shapes::instance(&raw.records[i])?.definition)?;
    (raw.records[d].kind() == kind::SYMBOL_DEFINITION).then_some(d)
}

/// The label an object is counted under.
pub fn label(raw: &Raw, i: usize) -> String {
    let k = raw.records[i].kind();
    if k == kind::PLUG_IN_OBJECT {
        return format!("{k} plug-in: {}", crate::records::plug_in(raw, i).map_or("?", String::as_str));
    }
    match crate::kinds::name(k) {
        "?" => format!("{k} type {k}"),
        name => format!("{k} {name}"),
    }
}

/// What the walk carries down: the nearest object around (not a group) and whether it is drawn,
/// whether we are inside a solid or a wall-hole cutter.
#[derive(Clone, Copy)]
struct Around {
    object_drawn: Option<bool>,
    solid: bool,
    cutter: bool,
    hidden: bool,
}

/// The coverage of a document read into `raw`, with `drawn` per record as the scene gives it
/// ([`crate::mesh::scene_and_drawn`]) and the records of the texts the scene keeps.
pub fn entries(raw: &Raw, drawn: &[bool], texts: &HashSet<usize>) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut seen = HashMap::new();
    for (i, r) in raw.records.iter().enumerate() {
        if !crate::raw::is_design_layer(r) {
            continue;
        }
        let around = Around { object_drawn: None, solid: false, cutter: false, hidden: crate::raw::layer_hidden(r.bytes) };
        visit(raw, i, around, drawn, texts, &mut seen, &mut out, 0);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn visit(raw: &Raw, i: usize, around: Around, drawn: &[bool], texts: &HashSet<usize>, seen: &mut HashMap<usize, Option<Status>>, out: &mut Vec<Entry>, depth: usize) {
    if depth > 32 {
        return;
    }
    for &c in &raw.children[i] {
        let r = &raw.records[c];
        let k = r.kind();
        if is_data(r) {
            continue;
        }
        let class = raw.by_ref.get(&r.class_id()).map(|&c| &raw.records[c]).filter(|c| c.kind() == kind::CLASS);
        let class_hidden = class.is_some_and(|class| crate::raw::class_hidden(class.bytes));
        let cutter = around.cutter || (k == kind::GROUP && crate::mesh::cuts_wall(raw, c));
        let is_drawn = drawn.get(c).copied().unwrap_or(false);
        let status = if is_drawn {
            Status::Drawn
        } else if cutter {
            Status::Cutter
        } else if around.solid {
            Status::Part
        } else if k == kind::TEXT && texts.contains(&c) {
            Status::Text
        } else if around.object_drawn == Some(true) && flat(raw, c, 0) {
            Status::Plan
        } else if matches!(k, 2 | 3 | 4 | 5 | 6 | 8 | 13 | 21 | kind::POLYGON_3D) && !crate::shapes::stroked(r, class) && !crate::shapes::filled(r, class) {
            Status::Invisible
        } else {
            Status::Missing
        };
        let hidden = around.hidden || class_hidden;
        let at = out.len();
        out.push(Entry { record: c, kind: k, label: label(raw, c), status, hidden, parent: label(raw, i) });
        let inner =
            Around { object_drawn: if k == kind::GROUP { around.object_drawn } else { Some(is_drawn || around.object_drawn == Some(true)) }, solid: around.solid || makes_solid(k), cutter, hidden };
        visit(raw, c, inner, drawn, texts, seen, out, depth + 1);
        let mut inside = summary(&out[at + 1..]);
        // what a placed symbol shows: its definition, counted once
        if k == kind::SYMBOL {
            if let Some(d) = definition(raw, c) {
                if !seen.contains_key(&d) {
                    seen.insert(d, None);
                    let start = out.len();
                    visit(raw, d, inner, drawn, texts, seen, out, depth + 1);
                    seen.insert(d, summary(&out[start..]));
                }
                inside = seen.get(&d).copied().flatten();
            }
        }
        // a group, symbol or plug-in object that is not drawn takes the status of what it holds:
        // a label of texts is text, a group of a plan view is plan
        if status == Status::Missing && matches!(k, kind::GROUP | kind::SYMBOL | kind::PLUG_IN_OBJECT) {
            out[at].status = inside.unwrap_or(Status::Missing);
        }
    }
}

/// The status a container takes from the objects inside it: none when something inside is
/// missing; empty when there is nothing; else text when a text is among them, then plan, part
/// or cutter.
fn summary(inside: &[Entry]) -> Option<Status> {
    if inside.iter().any(|e| e.status == Status::Missing || e.status == Status::Drawn) {
        return None;
    }
    [Status::Text, Status::Plan, Status::Part, Status::Cutter].into_iter().find(|s| inside.iter().any(|e| e.status == *s)).or(Some(Status::Empty))
}

/// The coverage of a file: every object of its design layers with its status.
pub fn coverage(data: &[u8]) -> crate::Result<Vec<Entry>> {
    let unpacked = crate::unpack(data)?;
    let raw = Raw::new(&unpacked)?;
    let (scene, drawn) = crate::mesh::scene_and_drawn(&raw, vwx_model::Info::default());
    let texts: HashSet<usize> = scene.texts.iter().map(|t| t.id as usize).collect();
    Ok(entries(&raw, &drawn, &texts))
}
