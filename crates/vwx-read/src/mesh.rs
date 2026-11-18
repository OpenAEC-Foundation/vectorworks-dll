//! Viewer meshes of a document: one mesh per layer and class, each object pickable.
//!
//! What is drawn so far (`docs/ARCHITECTURE.md` has the list of what is not):
//!
//! - extrudes and multiple extrudes of any profile (from the points Vectorworks keeps for the
//!   profile, type 42), and sweeps;
//! - 3D polygons, meshes and primitive solids (sphere, hemisphere, cone);
//! - CSG solids, walls and NURBS surfaces from the Parasolid body the
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

use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use vwx_model::{Info, Mesh, Scene, View};

use crate::raw::{self, kind, Raw};
use crate::shapes::{self, Affine, P2, P3};
use crate::styles;
use crate::tri;
use vwx_core::container::Record;

/// Millimetres to metres.
const SCALE: f64 = 0.001;
/// Deepest nesting of symbols in symbols that is followed (a definition that places itself
/// would never end).
const MAX_DEPTH: usize = 24;
/// The kind of building part a plug-in object is, by the name of its plug-in (English names,
/// as files have them whatever the language of Vectorworks).
fn plug_in_category(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if has(&["door"]) {
        "door"
    } else if has(&["window"]) {
        "window"
    } else if has(&["stair"]) {
        "stair"
    } else if has(&["slab"]) {
        "slab"
    } else if has(&["roof"]) {
        "roof"
    } else if has(&["space"]) {
        "space"
    } else if has(&["column", "pilaster"]) {
        "column"
    } else if has(&["structuralmember", "framing member", "beam"]) {
        "beam"
    } else if has(&["railing", "handrail", "fence"]) {
        "railing"
    } else {
        "plug-in object"
    }
}

/// Colour of lines whose pen is not known.
const PEN_COLOR: [u8; 3] = [30, 30, 30];
/// Fill of objects whose colour is not known.
const DEFAULT_COLOR: [u8; 3] = [200, 200, 204];

/// The meshes of a document.
pub fn scene(raw: &Raw, info: Info) -> Scene {
    scene_and_drawn(raw, info).0
}

/// The meshes of a document, and for each record whether drawing it gave triangles or line
/// segments (what it holds included): the measure of `crate::coverage`.
pub fn scene_and_drawn(raw: &Raw, info: Info) -> (Scene, Vec<bool>) {
    let mut builder = Builder {
        raw,
        batches: BTreeMap::new(),
        elevation: 0.0,
        bodies: HashMap::new(),
        drawn: vec![false; raw.records.len()],
        redraw: false,
        paper: 1.0,
        textures: crate::textures::Colours::default(),
        inherited: None,
    };
    for (i, r) in raw.records.iter().enumerate() {
        // a sheet layer is paper, not part of the model; a private layer is Vectorworks' own
        if !raw::is_design_layer(r) {
            continue;
        }
        // 3D objects hold their height in the document; 2D shapes lie on their layer
        builder.elevation = raw::layer_elevation(r.bytes);
        // dashes and page-based hatches are lengths on paper
        builder.paper = styles::layer_scale(r.bytes);
        for &child in &raw.children[i] {
            // what is set into a wall (a door, a window, a symbol) is an object of its own, in
            // its own class; the rest of what the wall holds is the wall
            let inserts: Vec<usize> = if raw.records[child].kind() == kind::WALL {
                raw.children[child].iter().copied().filter(|&c| matches!(raw.records[c].kind(), kind::PLUG_IN_OBJECT | kind::SYMBOL)).collect()
            } else {
                Vec::new()
            };
            let key = (i, raw.records[child].class_id());
            let start = builder.mark(key);
            if inserts.is_empty() {
                builder.walk(child, &Affine::IDENTITY, false, key, 0);
            } else {
                for &part in raw.children[child].iter().filter(|c| !inserts.contains(c)) {
                    builder.walk(part, &Affine::IDENTITY, true, key, 1);
                }
                if !builder.drawn.is_empty() && builder.mark(key) != start {
                    builder.drawn[child] = true;
                }
            }
            builder.element(child, i, key, start);
            for insert in inserts {
                let key = (i, raw.records[insert].class_id());
                let start = builder.mark(key);
                builder.walk(insert, &Affine::IDENTITY, true, key, 1);
                builder.element(insert, i, key, start);
            }
        }
    }
    // a library file places nothing on its layers: its symbols are what there is to see
    if builder.batches.values().all(|b| b.mesh.indices.is_empty() && b.mesh.lines.len() == b.markers as usize) {
        builder.paper = 1.0;
        builder.library();
    }
    let mut meshes: Vec<Mesh> = builder.batches.into_values().map(|b| b.mesh).filter(|m| !m.indices.is_empty() || !m.lines.is_empty()).collect();
    // a document may be saved with the classes of its model invisible (its sheets show the
    // model through viewports with their own settings, and so do its saved views): then
    // little or nothing would show. When more than half of what is drawn would start hidden,
    // all of it shows
    let size = |m: &Mesh| m.indices.len() + m.lines.len();
    let (hidden, all): (usize, usize) = meshes.iter().fold((0, 0), |(h, a), m| (h + if m.hidden { size(m) } else { 0 }, a + size(m)));
    if 2 * hidden > all {
        meshes.iter_mut().for_each(|m| m.hidden = false);
    }
    (Scene { info, meshes, views: views(raw), texts: texts(raw), sheets: sheets(raw), ..Scene::default() }, builder.drawn)
}

/// The texts on the design layers, on the elevation of their layer.
fn texts(raw: &Raw) -> Vec<vwx_model::Text> {
    let mut out = Vec::new();
    for (i, layer) in raw.records.iter().enumerate() {
        if !raw::is_design_layer(layer) {
            continue;
        }
        let name = raw::layer_name(layer.bytes).unwrap_or_default();
        texts_in(raw, i, &Affine::translation([0.0, 0.0, raw::layer_elevation(layer.bytes)]), &name, 0, &mut out);
    }
    out
}

/// Adds the texts among what record `i` holds: its own, and those inside its groups and
/// plug-in objects (a label, a title block), placed as their object is.
fn texts_in(raw: &Raw, i: usize, place: &Affine, layer: &str, depth: usize, out: &mut Vec<vwx_model::Text>) {
    if depth >= MAX_DEPTH {
        return;
    }
    for &c in &raw.children[i] {
        let r = &raw.records[c];
        match r.kind() {
            kind::TEXT => {
                let Some(t) = shapes::text(r) else { continue };
                // the top left corner of its box, where its object puts it
                let at = place.apply([t.at[0], t.at[1], 0.0]);
                // (as large as its placement makes it: an annotation of a viewport is drawn at
                // the scale of the viewport)
                let k = (place.rows[0][0].powi(2) + place.rows[0][1].powi(2) + place.rows[0][2].powi(2)).sqrt();
                out.push(vwx_model::Text {
                    id: c as u64,
                    layer: layer.to_string(),
                    text: t.text,
                    at: at.map(|v| (v * SCALE) as f32),
                    width: (t.width * k * SCALE) as f32,
                    height: (t.height * k * SCALE) as f32,
                    size: t.size as f32,
                    lines: t.lines,
                });
            }
            // (a dimension holds its lines and, in a group, its text, where they are drawn)
            kind::GROUP | kind::DIMENSION => texts_in(raw, c, place, layer, depth + 1, out),
            kind::PLUG_IN_OBJECT => {
                let place = shapes::plug_in_placement(r).map_or(*place, |p| p.then(place));
                texts_in(raw, c, &place, layer, depth + 1, out);
            }
            _ => {}
        }
    }
}

/// True for a group of a viewport that keeps a part of the drawing the viewport shows: the
/// byte at 0x55 of the group is 1 (the three such groups of a section viewport and the one of
/// an elevation in the corpus project; 0 in the groups of annotations and of the crop).
fn keeps_drawing(group: &vwx_core::container::Record) -> bool {
    group.bytes.get(0x55) == Some(&1)
}

/// True for a group that holds what cuts the hole of its object out of a wall: among its
/// user data is a record tagged `gchw` or `ggcw` (each window and door of the corpus project
/// has one of each, with an extrude of 6 m through the wall). It is not a part of the object.
pub(crate) fn cuts_wall(raw: &Raw, group: usize) -> bool {
    raw.children[group].iter().map(|&t| &raw.records[t]).any(|t| t.kind() == kind::USER_DATA && matches!(t.bytes.get(0x50..0x54), Some(b"gchw" | b"ggcw")))
}

/// True for the group of a viewport that holds its crop: it starts with user data tagged
/// `porc` (sample 246).
fn is_crop(raw: &Raw, group: usize) -> bool {
    raw.children[group].first().is_some_and(|&t| raw.records[t].kind() == kind::USER_DATA && raw.records[t].bytes.get(0x50..0x54) == Some(b"porc"))
}

/// What a viewport keeps of the drawing it shows, as line segments on its sheet (metres): the
/// shapes (lines, polygons, polylines) of its groups that keep it, in the millimetres of the
/// view. A section viewport keeps the outlines of what it cuts (its group
/// tagged `hcsc`), the lines beyond the cut (`ncsp`) and the outline of the whole (`hcac`); an
/// elevation the outline of what it shows. A point of the view lies on the sheet at the
/// viewport's origin plus the point divided by the scale (`Section-1` of the corpus project:
/// its kept drawing, 8.6 m by 7.9 m at 1 : 50, fills the box of the viewport so placed).
fn kept_drawing(raw: &Raw, viewport: usize, v: &shapes::Viewport) -> (Vec<[[f32; 2]; 2]>, Vec<vwx_model::Fill>) {
    let on_sheet = |p: P2| [((v.origin[0] + p[0] / v.scale) * SCALE) as f32, ((v.origin[1] + p[1] / v.scale) * SCALE) as f32];
    let (mut lines, mut fills) = (Vec::new(), Vec::new());
    for &group in raw.children[viewport].iter().filter(|&&g| raw.records[g].kind() == kind::GROUP) {
        // (its other groups are its annotations and its crop)
        if !keeps_drawing(&raw.records[group]) {
            continue;
        }
        // its shapes, and the shapes those hold (the parts of what is cut)
        let mut todo: Vec<(usize, usize)> = raw.children[group].iter().rev().map(|&c| (c, 0)).collect();
        while let Some((c, depth)) = todo.pop() {
            let r = &raw.records[c];
            if depth < MAX_DEPTH && r.kind() != kind::USER_DATA {
                todo.extend(raw.children[c].iter().rev().map(|&k| (k, depth + 1)));
            }
            if !matches!(r.kind(), kind::LINE | kind::POLYGON | kind::POLYLINE | kind::RECTANGLE | kind::ARC | kind::OVAL) {
                continue;
            }
            let class = raw.by_ref.get(&r.class_id()).map(|&k| &raw.records[k]).filter(|k| k.kind() == kind::CLASS);
            let outline = shapes::outline(r).filter(|o| o.iter().flatten().all(|x| x.is_finite()));
            // what is cut is filled in the colour of its fill, or hatched
            if let Some(outline) = outline.as_ref().filter(|_| r.kind() != kind::LINE && shapes::closed(r) && shapes::filled(r, class)) {
                let color = shapes::fill_index(r, class).and_then(|k| raw.palette.color(k)).unwrap_or(DEFAULT_COLOR);
                let pen = shapes::pen_index(r, class).and_then(|k| raw.palette.color(k)).unwrap_or(PEN_COLOR);
                let filling = styles::filling(raw, c, class, outline, v.scale, color, pen);
                if let Some(color) = filling.color {
                    fills.push(vwx_model::Fill { id: c as u64, outline: outline.iter().map(|p| on_sheet(*p)).collect(), color });
                }
                lines.extend(filling.lines.iter().map(|([a, b], _)| [on_sheet(*a), on_sheet(*b)]));
            }
            if !shapes::stroked(r, class) {
                continue;
            }
            // its lines, in dashes when its pen is a line type of dashes
            let pattern = styles::dashes(raw, r, class, v.scale);
            let mut draw = |points: &[P2], closed: bool| match pattern.as_ref().and_then(|p| styles::dash(points, closed, p)) {
                Some(dashes) => lines.extend(dashes.iter().map(|[a, b]| [on_sheet(*a), on_sheet(*b)])),
                None => {
                    lines.extend(points.windows(2).map(|w| [on_sheet(w[0]), on_sheet(w[1])]));
                    if closed {
                        lines.push([on_sheet(points[points.len() - 1]), on_sheet(points[0])]);
                    }
                }
            };
            if let Some(vertices) = (r.kind() == kind::POLYLINE).then(|| shapes::polyline(r)).flatten() {
                // a polyline: its sides that are drawn (those to its holes are not)
                let sides: Vec<[P2; 2]> = shapes::polyline_segments(&vertices, shapes::closed(r)).into_iter().filter(|s| s.iter().flatten().all(|c| c.is_finite())).collect();
                for line in styles::chains(&sides) {
                    draw(&line, false);
                }
                continue;
            }
            let points: Vec<P2> = match r.kind() {
                kind::LINE => shapes::line(r).map(|l| l.to_vec()).unwrap_or_default(),
                _ => outline.clone().or_else(|| shapes::arc(r)).unwrap_or_default(),
            };
            if points.len() < 2 || points.iter().flatten().any(|c| !c.is_finite()) {
                continue;
            }
            draw(&points, r.kind() != kind::LINE && shapes::closed(r) && outline.is_some() && points.len() > 2);
        }
    }
    (lines, fills)
}

/// The sheet layers of a document: what is drawn on each (lines, arcs, outlines; texts).
fn sheets(raw: &Raw) -> Vec<vwx_model::Sheet> {
    let mut out = Vec::new();
    // the design layers: reference number and name
    let design_layers: Vec<(u32, String)> =
        raw.records.iter().filter(|r| r.kind() == kind::LAYER && !raw::layer_is_sheet(r.bytes)).map(|r| (r.ref_id(), raw::layer_name(r.bytes).unwrap_or_default())).collect();
    for (i, layer) in raw.records.iter().enumerate() {
        if layer.kind() != kind::LAYER || !raw::layer_is_sheet(layer.bytes) {
            continue;
        }
        let mut names = raw::layer_texts(layer.bytes).into_iter();
        let (number, title) = (names.next().unwrap_or_default(), names.next().unwrap_or_default());
        // drawn as a layer is, then only its line work kept, flat
        let mut builder = Builder {
            raw,
            batches: BTreeMap::new(),
            elevation: 0.0,
            bodies: HashMap::new(),
            drawn: Vec::new(),
            redraw: false,
            paper: 1.0,
            textures: crate::textures::Colours::default(),
            inherited: None,
        };
        for &child in &raw.children[i] {
            builder.walk(child, &Affine::IDENTITY, false, (i, 0), 0);
        }
        let mut texts = Vec::new();
        texts_in(raw, i, &Affine::IDENTITY, &number, 0, &mut texts);
        // the annotations of its viewports (labels, dimensions, notes): drawn in the
        // millimetres of the sheet times the scale of the viewport
        for &c in raw.children[i].iter().filter(|&&c| raw.records[c].kind() == kind::VIEWPORT) {
            let Some(v) = shapes::viewport(&raw.records[c]) else { continue };
            let k = 1.0 / v.scale;
            let on_sheet = Affine { rows: [[k, 0.0, 0.0], [0.0, k, 0.0], [0.0, 0.0, k]], offset: [0.0; 3] };
            builder.paper = v.scale;
            for &group in raw.children[c].iter().filter(|&&g| raw.records[g].kind() == kind::GROUP && !keeps_drawing(&raw.records[g]) && !is_crop(raw, g)) {
                builder.walk(group, &on_sheet, false, (i, 0), 0);
                texts_in(raw, group, &on_sheet, &number, 0, &mut texts);
            }
            builder.paper = 1.0;
        }
        let mut lines = Vec::new();
        for batch in builder.batches.values() {
            let point = |k: u32| [batch.mesh.positions[3 * k as usize], batch.mesh.positions[3 * k as usize + 1]];
            lines.extend(batch.mesh.lines.as_chunks::<2>().0.iter().map(|pair| [point(pair[0]), point(pair[1])]));
        }
        let mut viewports = Vec::new();
        let mut fills = Vec::new();
        for &c in raw.children[i].iter().filter(|&&c| raw.records[c].kind() == kind::VIEWPORT) {
            let Some(v) = shapes::viewport(&raw.records[c]) else { continue };
            let (kept, filled) = kept_drawing(raw, c, &v);
            lines.extend(kept);
            fills.extend(filled);
            viewports.push(vwx_model::Viewport {
                id: c as u64,
                name: raw.name(c).unwrap_or_default().to_string(),
                min: v.min.map(|x| (x * SCALE) as f32),
                max: v.max.map(|x| (x * SCALE) as f32),
                scale: v.scale as f32,
                origin: v.origin.map(|x| (x * SCALE) as f32),
                forward: v.axes[2].map(|x| -x as f32),
                up: v.axes[1].map(|x| x as f32),
                // a section viewport keeps what it cuts in a group tagged `hcsc`
                section: raw.children[c]
                    .iter()
                    .filter(|&&g| raw.records[g].kind() == kind::GROUP)
                    .any(|&g| raw.children[g].first().is_some_and(|&t| raw.records[t].kind() == kind::USER_DATA && raw.records[t].bytes.get(0x50..0x54) == Some(b"hcsc"))),
                // it shows the design layers it names as visible or greyed, and no others
                hidden_layers: design_layers.iter().filter(|(number, _)| !v.layers.iter().any(|(n, visibility)| n == number && *visibility != -1)).map(|(_, name)| name.clone()).collect(),
                hidden_classes: v.classes.iter().filter(|(_, visibility)| *visibility == -1).filter_map(|(n, _)| raw.names.get(n).cloned()).collect(),
            });
        }
        out.push(vwx_model::Sheet { id: i as u64, number, title, lines, fills, texts, viewports });
    }
    out
}

/// The saved views of a document: a type 49 record each, named in the name list, with the way
/// it looks at the model in its type 43 child.
fn views(raw: &Raw) -> Vec<View> {
    let mut out = Vec::new();
    for (i, r) in raw.records.iter().enumerate() {
        if r.kind() != kind::SAVED_VIEW {
            continue;
        }
        let Some(name) = raw.names.get(&r.ref_id()) else { continue };
        let Some([_, up, eye]) = raw.children[i].iter().map(|&c| &raw.records[c]).find(|c| c.kind() == kind::VIEW).and_then(shapes::view_axes) else {
            continue;
        };
        out.push(View { name: name.clone(), forward: eye.map(|c| -c as f32), up: up.map(|c| c as f32) });
    }
    out
}

/// A symbol definition drawn for the library view: its record, its first index and first vertex
/// in the mesh, and its lowest and highest corner in metres.
type Shown = (usize, usize, usize, [f32; 3], [f32; 3], usize);

/// The batch that holds the symbol definitions of a library file.
const LIBRARY: (usize, u32) = (usize::MAX, 0);

struct Batch {
    mesh: Mesh,
    /// Of `mesh.lines`, the indexes that draw markers (loci): not a form of their object.
    markers: u32,
}

struct Builder<'a, 'b> {
    raw: &'b Raw<'a>,
    /// By (layer record, class reference number).
    batches: BTreeMap<(usize, u32), Batch>,
    /// Elevation of the layer being drawn (millimetres).
    elevation: f64,
    /// The triangles of the Parasolid bodies read so far, by record: an object inside a symbol
    /// is drawn once for every place the symbol is put.
    bodies: HashMap<usize, Option<Rc<vwx_xt::Mesh>>>,
    /// Per record: true once drawing it gave triangles or line segments (empty: not kept).
    drawn: Vec<bool>,
    /// True while the 2D shapes of an object without a 3D form are drawn: its markers were
    /// drawn already with its 3D form.
    redraw: bool,
    /// Millimetres of the drawing per millimetre of paper (the scale of the layer or viewport
    /// drawn): dashes and page-based hatches are lengths on paper.
    paper: f64,
    /// The colours of the textures, as objects need them.
    textures: crate::textures::Colours,
    /// The colour of the texture of the plug-in object whose parts are drawn: what a part
    /// without a texture of its own or of its class shows.
    inherited: Option<[u8; 3]>,
}

impl Builder<'_, '_> {
    fn batch(&mut self, key: (usize, u32)) -> &mut Batch {
        let raw = self.raw;
        self.batches.entry(key).or_insert_with(|| {
            let layer = match key {
                LIBRARY => "Symbols".into(),
                _ => raw::layer_name(raw.records[key.0].bytes).unwrap_or_else(|| "Layer".into()),
            };
            let layer_name = layer.clone();
            let name = match raw.names.get(&key.1) {
                Some(class) => format!("{layer} / {class}"),
                None => layer,
            };
            // what is in an invisible class starts switched off
            let hidden = raw.by_ref.get(&key.1).is_some_and(|&class| raw::class_hidden(raw.records[class].bytes));
            // and so does what is on an invisible layer
            let hidden = hidden || raw.records.get(key.0).is_some_and(|layer| raw::layer_hidden(layer.bytes));
            let (layer, class) = (if key == LIBRARY { String::new() } else { layer_name.clone() }, raw.names.get(&key.1).cloned().unwrap_or_default());
            Batch { mesh: Mesh { name, color: DEFAULT_COLOR.map(|c| c as f32 / 255.0), colors: Some(Vec::new()), hidden, layer, class, ..Mesh::default() }, markers: 0 }
        })
    }

    /// Draws every symbol definition of the document, side by side in rows, each pickable under
    /// its name. A definition with nothing in 3D shows its plan view.
    fn library(&mut self) {
        let raw = self.raw;
        self.elevation = 0.0;
        let mut items: Vec<Shown> = Vec::new();
        for d in (0..raw.records.len()).filter(|&i| raw.records[i].kind() == kind::SYMBOL_DEFINITION) {
            let (index, vertex, line) = {
                let mesh = &self.batch(LIBRARY).mesh;
                (mesh.indices.len(), mesh.positions.len() / 3, mesh.lines.len())
            };
            for nested in [true, false] {
                for &c in &raw.children[d] {
                    self.walk(c, &Affine::IDENTITY, nested, LIBRARY, 1);
                }
                if self.batch(LIBRARY).mesh.indices.len() > index || self.batch(LIBRARY).mesh.lines.len() > line {
                    break;
                }
            }
            let mesh = &self.batch(LIBRARY).mesh;
            if mesh.indices.len() == index && mesh.lines.len() == line {
                continue;
            }
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in mesh.positions[vertex * 3..].as_chunks::<3>().0 {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            items.push((d, index, vertex, lo, hi, line));
        }
        if items.is_empty() {
            return;
        }
        // rows of about equal length and depth, a gap of a tenth of the average size between
        let size = |lo: &[f32; 3], hi: &[f32; 3], k: usize| (hi[k] - lo[k]).max(0.0);
        let mean = items.iter().map(|(_, _, _, lo, hi, _)| size(lo, hi, 0).max(size(lo, hi, 1))).sum::<f32>() / items.len() as f32;
        let gap = (mean * 0.1).max(0.05);
        let area: f32 = items.iter().map(|(_, _, _, lo, hi, _)| (size(lo, hi, 0) + gap) * (size(lo, hi, 1) + gap)).sum();
        let widest = items.iter().map(|(_, _, _, lo, hi, _)| size(lo, hi, 0)).fold(0.0, f32::max);
        let row_length = (area.sqrt() * 1.3).max(widest);
        let (mut x, mut y, mut depth) = (0.0f32, 0.0f32, 0.0f32);
        let ends: Vec<usize> = items.iter().skip(1).map(|item| item.2).collect();
        let mesh = &mut self.batch(LIBRARY).mesh;
        let last = mesh.positions.len() / 3;
        for (n, (d, index, vertex, lo, hi, line)) in items.iter().enumerate() {
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
            if count > 0 {
                mesh.elements.push((*d as u64, *index as u32, count as u32));
            }
            let lines = ends.get(n).map_or(mesh.lines.len(), |_| items[n + 1].5) - line;
            if lines > 0 {
                mesh.line_elements.push((*d as u64, *line as u32, lines as u32));
            }
            mesh.props.push((*d as u64, serde_json::Value::Object(map).to_string()));
        }
    }

    /// Makes what was drawn in batch `key` since index `start` the pickable object `i` of
    /// layer `layer`, with its properties.
    fn element(&mut self, i: usize, layer: usize, key: (usize, u32), start: (u32, u32)) {
        let now = self.mark(key);
        let (triangles, lines) = (now.0 - start.0, now.1 - start.1);
        if triangles + lines == 0 {
            return;
        }
        let props = self.props(i, layer);
        let mesh = &mut self.batch(key).mesh;
        if triangles > 0 {
            mesh.elements.push((i as u64, start.0, triangles));
        }
        if lines > 0 {
            mesh.line_elements.push((i as u64, start.1, lines));
        }
        mesh.props.push((i as u64, props));
    }

    /// How far batch `key` is drawn: its counts of triangle and of line indexes.
    fn mark(&mut self, key: (usize, u32)) -> (u32, u32) {
        let mesh = &self.batch(key).mesh;
        (mesh.indices.len() as u32, mesh.lines.len() as u32)
    }

    /// As [`Self::mark`], markers left out: what gives an object a form.
    fn form(&mut self, key: (usize, u32)) -> (u32, u32) {
        let batch = self.batch(key);
        (batch.mesh.indices.len() as u32, batch.mesh.lines.len() as u32 - batch.markers)
    }

    /// Draws a locus at `at` (placed, millimetres) as a cross: along z too when `up`.
    fn marker(&mut self, key: (usize, u32), at: P3, color: [u8; 3], up: bool) {
        let before = self.mark(key).1;
        for arm in crate::objects::cross(at, up) {
            self.stroke(key, &arm, color, false);
        }
        let added = self.mark(key).1 - before;
        self.batch(key).markers += added;
    }

    /// Adds the line through `points` (millimetres, placed already), back to its first point
    /// when `closed`.
    fn stroke(&mut self, key: (usize, u32), points: &[P3], color: [u8; 3], closed: bool) {
        if points.len() < 2 || points.iter().flatten().any(|c| !c.is_finite()) {
            return;
        }
        let mesh = &mut self.batch(key).mesh;
        let base = (mesh.positions.len() / 3) as u32;
        let n = points.len() as u32;
        for p in points {
            mesh.positions.extend(p.map(|c| (c * SCALE) as f32));
            mesh.normals.extend([0.0; 3]);
        }
        if let Some(colors) = mesh.colors.as_mut() {
            colors.extend(std::iter::repeat_n([color[0], color[1], color[2], 255], points.len()));
        }
        mesh.lines.extend((0..n - 1).flat_map(|k| [base + k, base + k + 1]));
        if closed && n > 2 {
            mesh.lines.extend([base + n - 1, base]);
        }
    }

    /// Draws the line through `points` (in the coordinates of object `r`, at height `z`, placed
    /// by `place`) with the pen of `r`: in dashes when it has a line type of dashes.
    #[allow(clippy::too_many_arguments)]
    fn pen_line(&mut self, r: &Record, class: Option<&Record>, key: (usize, u32), points: &[P2], closed: bool, place: &Affine, z: f64, color: [u8; 3]) {
        let at = |p: &P2| place.apply([p[0], p[1], z]);
        match styles::dashes(self.raw, r, class, self.paper).and_then(|pattern| styles::dash(points, closed, &pattern)) {
            Some(dashes) => {
                for [a, b] in dashes {
                    self.stroke(key, &[at(&a), at(&b)], color, false);
                }
            }
            None => self.stroke(key, &points.iter().map(at).collect::<Vec<P3>>(), color, closed),
        }
    }

    /// Draws the fill of 2D object `i` inside `outline` (its own coordinates, at height `z`,
    /// placed by `place`): a face in its colour, and the lines of its hatch or tiles.
    #[allow(clippy::too_many_arguments)]
    fn fill(&mut self, i: usize, class: Option<&Record>, key: (usize, u32), outline: &[P2], place: &Affine, z: f64, color: [u8; 3], pen: [u8; 3]) {
        let filling = styles::filling(self.raw, i, class, outline, self.paper, color, pen);
        let at = |p: &P2| place.apply([p[0], p[1], z]);
        if let Some(color) = filling.color {
            self.face(key, &outline.iter().map(at).collect::<Vec<P3>>(), color, place.mirrors());
        }
        for ([a, b], color) in filling.lines {
            self.stroke(key, &[at(&a), at(&b)], color, false);
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
        // what a plug-in object is, by the name of its plug-in
        if r.kind() == kind::PLUG_IN_OBJECT {
            if let Some(name) = crate::records::plug_in(self.raw, i) {
                map.insert("Object".into(), name.clone().into());
                map.insert("category".into(), plug_in_category(name).into());
            }
        }
        // the records attached to it, each value under the name of its record and field
        for (record, values) in crate::records::of_object(self.raw, i) {
            // the IFC entity Vectorworks exports it as
            if let Some(entity) = crate::records::ifc_entity(&record) {
                map.entry("IFC").or_insert_with(|| entity.into());
            }
            // (all of them: the record of a plug-in object holds its settings, such as the
            // name, number and area of a space or the width of a door)
            let record = crate::records::label(&record);
            for (field, value) in crate::records::pairs(&values) {
                let value = match value {
                    crate::records::Value::Integer(v) => v.into(),
                    crate::records::Value::Boolean(v) => v.into(),
                    crate::records::Value::Number(v) | crate::records::Value::Date(v) => v.into(),
                    crate::records::Value::Reference(v) => v.into(),
                    crate::records::Value::Colour(v) => v.into(),
                    crate::records::Value::Text(v) if v.is_empty() => continue,
                    crate::records::Value::Text(v) => v.into(),
                };
                map.insert(format!("{record}.{field}"), value);
            }
        }
        serde_json::Value::Object(map).to_string()
    }

    /// Draws record `i` and what is inside it. `nested` is true inside a symbol or another
    /// object, where 2D shapes are the plan view.
    fn walk(&mut self, i: usize, place: &Affine, nested: bool, key: (usize, u32), depth: usize) {
        let before = self.mark(key);
        self.walk_one(i, place, nested, key, depth);
        if !self.drawn.is_empty() && self.mark(key) != before {
            self.drawn[i] = true;
        }
    }

    fn walk_one(&mut self, i: usize, place: &Affine, nested: bool, key: (usize, u32), depth: usize) {
        let raw = self.raw;
        let r = &raw.records[i];
        // its class, for what it takes from there
        let class = raw.by_ref.get(&r.class_id()).map(|&c| &raw.records[c]).filter(|c| c.kind() == kind::CLASS);
        // the colour of its texture (or its class's), as a rendered view shows it, else that of
        // its fill
        let color = self.textures.of_object(raw, r, class).or(self.inherited).or_else(|| shapes::fill_index(r, class).and_then(|k| raw.palette.color(k))).unwrap_or(DEFAULT_COLOR);
        match r.kind() {
            kind::END | kind::USER_DATA => {}
            // a viewport shows the model on a sheet: what it holds is laid out by it, which is
            // not read yet
            kind::VIEWPORT => {}
            // the children of a CSG solid are its operands; the result is its Parasolid body
            kind::CSG_SOLID => {
                self.body(i, place, key, color, true);
            }
            // a NURBS surface, inside its boundaries (its children, in its parameters)
            kind::NURBS_SURFACE => {
                if self.body(i, place, key, color, true) {
                    return;
                }
                let Some(surface) = shapes::nurbs_surface(r) else { return };
                let loops: Vec<Vec<P2>> = raw.children[i].iter().filter(|&&c| raw.records[c].kind() == kind::NURBS_CURVE).filter_map(|&c| shapes::trim_loop(&raw.records[c])).collect();
                let patch = vwx_xt::trimmed(&Rc::new(surface), &loops);
                self.solid(key, color, &patch, place);
            }
            kind::EXTRUDE => {
                // the exact body when the file has one and all of it is read
                if self.body(i, place, key, color, false) {
                    return;
                }
                let Some(e) = shapes::extrude(r) else { return };
                let place = e.placement.then(place);
                for &c in &raw.children[i] {
                    // the profile as Vectorworks stepped it, at the bottom and at the top
                    if let Some(rings) = self.grid_of(c) {
                        self.loft(key, &rings, &place, color, false, true);
                    } else if let Some(outline) = shapes::outline(&raw.records[c]) {
                        self.prism(key, &outline, e.bottom, e.top, &place, color);
                    }
                }
            }
            kind::SOLID => {
                // a sphere, hemisphere or cone: the mesh Vectorworks keeps for it
                let Some(&child) = raw.children[i].iter().find(|&&c| raw.records[c].kind() == kind::POLY_MESH) else {
                    return;
                };
                let Some((points, faces)) = shapes::poly_mesh(&raw.records[child]) else {
                    return;
                };
                for face in faces {
                    let corners: Vec<P3> = face.iter().map(|&k| place.apply(points[k])).collect();
                    self.face(key, &corners, color, place.mirrors());
                }
            }
            kind::MULTIPLE_EXTRUDE => {
                if self.body(i, place, key, color, false) {
                    return;
                }
                // one ring per profile, in the object's own coordinates
                let (Some(rings), Some((placement, _))) = (self.grid_of(i), shapes::solid_frame(r)) else {
                    return;
                };
                self.loft(key, &rings, &placement.then(place), color, false, true);
            }
            kind::SWEEP => {
                if self.body(i, place, key, color, false) {
                    return;
                }
                let Some(sweep) = shapes::sweep(r) else {
                    return;
                };
                let place = sweep.placement.then(place);
                for &c in &raw.children[i] {
                    // the rings Vectorworks keeps for the profile, else the profile turned here
                    let (rings, closed) = match self.grid_of(c) {
                        Some(mut rings) => {
                            rings.dedup();
                            let closed = rings.len() > 2 && same_ring(&rings[0], &rings[rings.len() - 1]);
                            if closed {
                                rings.pop();
                            }
                            (rings, closed)
                        }
                        None => match shapes::outline(&raw.records[c]).filter(|p| p.len() >= 3) {
                            Some(profile) => sweep.rings(&profile),
                            None => continue,
                        },
                    };
                    self.loft(key, &rings, &place, color, closed, !closed);
                }
            }
            kind::POLYGON_3D => {
                // a face when it has a fill and an area, else its outline when it has a pen (a 3D
                // polygon of two points is a 3D line); with neither it is not seen
                let Some(points) = shapes::polygon_3d(r).filter(|p| p.len() >= 2) else { return };
                let world: Vec<P3> = points.iter().map(|&p| place.apply(p)).collect();
                if world.len() >= 3 && shapes::filled(r, class) && newell(&world).is_some() {
                    self.face(key, &world, color, false);
                } else if shapes::stroked(r, class) && !self.redraw {
                    let pen = shapes::pen_index(r, class).and_then(|k| raw.palette.color(k)).unwrap_or(PEN_COLOR);
                    self.stroke(key, &world, pen, crate::objects::polygon_3d_closed(r));
                }
            }
            // a NURBS curve on its own is a line in space (one inside a NURBS surface bounds it,
            // and is read with it)
            kind::NURBS_CURVE => {
                if self.redraw || !shapes::stroked(r, class) {
                    return;
                }
                let Some(points) = shapes::nurbs_curve(r) else { return };
                let world: Vec<P3> = points.iter().map(|&p| place.apply(p)).collect();
                let pen = shapes::pen_index(r, class).and_then(|k| raw.palette.color(k)).unwrap_or(PEN_COLOR);
                self.stroke(key, &world, pen, false);
            }
            // a locus is drawn as a small cross: a 3D locus wherever it is, a 2D locus where
            // the other 2D shapes are drawn
            kind::LOCUS_3D => {
                if self.redraw {
                    return;
                }
                if let Some(at) = crate::objects::locus_3d(r) {
                    let pen = shapes::pen_index(r, class).and_then(|k| raw.palette.color(k)).unwrap_or(PEN_COLOR);
                    self.marker(key, place.apply(at), pen, true);
                }
            }
            kind::LOCUS => {
                if nested {
                    return;
                }
                if let Some(p) = crate::objects::locus(r) {
                    let pen = shapes::pen_index(r, class).and_then(|k| raw.palette.color(k)).unwrap_or(PEN_COLOR);
                    self.marker(key, place.apply([p[0], p[1], self.elevation]), pen, false);
                }
            }
            kind::MESH => {
                let Some(points) = shapes::mesh_points(r) else {
                    return;
                };
                let faces: Vec<Vec<P3>> = raw.children[i]
                    .iter()
                    .map(|&c| &raw.records[c])
                    .filter(|face| face.kind() == kind::MESH_FACE)
                    .filter_map(|face| shapes::mesh_face(face).and_then(|f| f.iter().map(|&k| points.get(k).map(|&p| place.apply(p))).collect::<Option<Vec<P3>>>()))
                    .filter(|c| c.len() >= 3)
                    .collect();
                // Vectorworks draws a mesh from both sides and keeps the faces of some turned
                // inward (the surface of a site model runs clockwise seen from above): a mesh
                // is turned to face outward, or up when it is open (found from the placed corners,
                // so a mirroring placement is taken into account)
                let flip = facing_inward(&faces);
                for corners in faces {
                    self.face(key, &corners, color, flip);
                }
            }
            kind::RECTANGLE | kind::ROUNDED_RECTANGLE | kind::OVAL | kind::POLYGON | kind::POLYLINE | kind::ARC => {
                if nested {
                    return;
                }
                let z = self.elevation;
                let pen = shapes::pen_index(r, class).and_then(|k| raw.palette.color(k)).unwrap_or(PEN_COLOR);
                if let Some(outline) = shapes::outline(r) {
                    let outline = counter_clockwise(outline);
                    // a shape with a fill is a face (with the lines of its hatch), one without
                    // is its outline
                    if shapes::filled(r, class) {
                        self.fill(i, class, key, &outline, place, z, color, pen);
                    } else if shapes::stroked(r, class) {
                        match (r.kind() == kind::POLYLINE).then(|| shapes::polyline(r)).flatten() {
                            // a polyline: its sides that are drawn
                            Some(vertices) => {
                                for line in styles::chains(&shapes::polyline_segments(&vertices, shapes::closed(r))) {
                                    self.pen_line(r, class, key, &line, false, place, z, pen);
                                }
                            }
                            None => self.pen_line(r, class, key, &outline, shapes::closed(r), place, z, pen),
                        }
                    }
                } else if let Some(points) = shapes::two_points(r).filter(|_| shapes::stroked(r, class)) {
                    self.pen_line(r, class, key, &points, false, place, z, pen);
                } else if let Some(points) = (r.kind() == kind::ARC && shapes::stroked(r, class)).then(|| shapes::arc(r)).flatten() {
                    self.pen_line(r, class, key, &points, false, place, z, pen);
                }
            }
            kind::LINE => {
                if nested || !shapes::stroked(r, class) {
                    return;
                }
                if let Some([a, b]) = shapes::line(r) {
                    let z = self.elevation;
                    let pen = shapes::pen_index(r, class).and_then(|k| raw.palette.color(k)).unwrap_or(PEN_COLOR);
                    self.pen_line(r, class, key, &[a, b], false, place, z, pen);
                }
            }
            kind::SYMBOL => {
                let Some(instance) = shapes::instance(r) else {
                    return;
                };
                let Some(&definition) = raw.by_ref.get(&instance.definition) else {
                    return;
                };
                if depth >= MAX_DEPTH || raw.records[definition].kind() != kind::SYMBOL_DEFINITION {
                    return;
                }
                let place = instance.placement.then(place);
                let before = self.form(key);
                for &c in &raw.children[definition] {
                    self.walk(c, &place, true, key, depth + 1);
                }
                // a symbol without a 3D form is its 2D shapes
                if !nested && self.form(key) == before {
                    let again = std::mem::replace(&mut self.redraw, true);
                    for &c in &raw.children[definition] {
                        self.walk(c, &place, false, key, depth + 1);
                    }
                    self.redraw = again;
                }
            }
            // (what cuts the hole of a window or door out of its wall is not drawn)
            kind::GROUP if cuts_wall(raw, i) => {}
            kind::GROUP => {
                for &c in &raw.children[i] {
                    self.walk(c, place, nested, key, depth);
                }
            }
            // any other object: what it holds is part of it. Its 2D shapes are its plan view,
            // left out beside its 3D form; an object without one (a label, a title block) is
            // those shapes
            _ => {
                // a plug-in object holds its parts in its own coordinates
                let placed = (r.kind() == kind::PLUG_IN_OBJECT).then(|| shapes::plug_in_placement(r)).flatten().map(|p| p.then(place));
                let place = placed.as_ref().unwrap_or(place);
                // its parts show its texture where they have none of their own (a site model
                // keeps the texture of its surface so)
                let outer = self.inherited;
                if r.kind() == kind::PLUG_IN_OBJECT {
                    self.inherited = self.textures.of_object(raw, r, None).or(outer);
                }
                let before = self.form(key);
                let shown = site_surfaces(raw, i);
                for &c in raw.children[i].iter().filter(|c| shown.as_ref().is_none_or(|s| s.contains(c))) {
                    self.walk(c, place, true, key, depth);
                }
                if !nested && self.form(key) == before {
                    let again = std::mem::replace(&mut self.redraw, true);
                    for &c in &raw.children[i] {
                        self.walk(c, place, false, key, depth);
                    }
                    self.redraw = again;
                }
                self.inherited = outer;
            }
        }
    }

    /// Draws the cached solid body of object `i` (its child of type 114). With `approximate`
    /// false a body with faces that are not evaluated yet is left out. True when drawn.
    fn body(&mut self, i: usize, place: &Affine, key: (usize, u32), color: [u8; 3], approximate: bool) -> bool {
        let raw = self.raw;
        let Some(&child) = raw.children[i].iter().find(|&&c| raw.records[c].kind() == kind::BODY) else {
            return false;
        };
        let Some(body) = self.bodies.entry(child).or_insert_with(|| shapes::body(&raw.records[child]).map(Rc::new)).clone() else {
            return false;
        };
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
        self.solid(key, color, &body, &place);
        true
    }

    /// Adds the triangles of `body` (millimetres), placed by `place`.
    fn solid(&mut self, key: (usize, u32), color: [u8; 3], body: &vwx_xt::Mesh, place: &Affine) {
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
    }

    /// The grid of points (type 42) among the children of record `i`.
    fn grid_of(&self, i: usize) -> Option<Vec<Vec<P3>>> {
        let raw = self.raw;
        raw.children[i].iter().find(|&&c| raw.records[c].kind() == kind::PROFILE_GRID).and_then(|&c| shapes::grid(&raw.records[c]))
    }

    /// The surface through rings of points, each ring a closed profile with its points in the
    /// same order: the faces between ring and ring, with `closed` also from the last ring back
    /// to the first, and with `caps` the first and last ring filled.
    fn loft(&mut self, key: (usize, u32), rings: &[Vec<P3>], place: &Affine, color: [u8; 3], closed: bool, caps: bool) {
        let n = rings.first().map_or(0, Vec::len);
        if rings.len() < 2 || n < 3 || rings.iter().any(|r| r.len() != n) {
            return;
        }
        let world: Vec<Vec<P3>> = rings.iter().map(|ring| ring.iter().map(|p| place.apply(*p)).collect()).collect();
        // which way the profile turns, seen along the way the rings go: decides which side of
        // the faces is the outside
        let Some(normal) = newell(&world[0]) else {
            return;
        };
        let forward: f64 = (0..n).map(|k| (0..3).map(|a| (world[1][k][a] - world[0][k][a]) * normal[a]).sum::<f64>()).sum();
        let outward = forward >= 0.0;
        let pairs = rings.len() - usize::from(!closed);
        for r in 0..pairs {
            let (a, b) = (&world[r], &world[(r + 1) % rings.len()]);
            for k in 0..n {
                let j = (k + 1) % n;
                if a[k] == a[j] && b[k] == b[j] {
                    continue;
                }
                let quad = if outward { [a[k], a[j], b[j], b[k]] } else { [a[k], b[k], b[j], a[j]] };
                // two triangles: a quad between turned rings is not flat
                self.face(key, &[quad[0], quad[1], quad[2]], color, false);
                self.face(key, &[quad[0], quad[2], quad[3]], color, false);
            }
        }
        if caps {
            let first: Vec<P3> = if outward { world[0].iter().rev().copied().collect() } else { world[0].clone() };
            let last = &world[rings.len() - 1];
            let last: Vec<P3> = if outward { last.clone() } else { last.iter().rev().copied().collect() };
            self.face(key, &first, color, false);
            self.face(key, &last, color, false);
        }
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
        let Some(normal) = newell(corners) else {
            return;
        };
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

/// True when two rings are the same points, to a thousandth of their size.
/// What a site model (plug-in object `DTM6`) shows in 3D: its surfaces only. It keeps its
/// proposed surface as its first mesh and its existing one as its second (the garden design of
/// `Site Plan.sta`: 33 164 points with the texture of its lawn, then 2 462 without one, above
/// the proposed one where the ground is cut), and its setting `3D Display` says which it shows
/// (`kTINDisplayProposedOnly` there, as in `Planting Plan.sta`, which keeps one mesh only).
/// Its other parts are helpers Vectorworks does not draw in 3D: its contours, its points, and
/// its boundary extruded from 0 to the highest point, whose flat top covered the whole site.
/// `None` for any other object: all of it shows.
fn site_surfaces(raw: &Raw, i: usize) -> Option<Vec<usize>> {
    if raw.records[i].kind() != kind::PLUG_IN_OBJECT || crate::records::plug_in(raw, i).map(String::as_str) != Some("DTM6") {
        return None;
    }
    let meshes: Vec<usize> = raw.children[i].iter().copied().filter(|&c| raw.records[c].kind() == kind::MESH).collect();
    if meshes.is_empty() {
        return None;
    }
    let display = crate::records::of_object(raw, i).into_iter().flat_map(|(_, values)| values).find_map(|(name, value)| match (name.as_str(), value) {
        ("3D Display", crate::records::Value::Text(v)) => Some(v),
        _ => None,
    });
    Some(match (display.as_deref(), meshes.len()) {
        (Some("kTINDisplayProposedOnly"), 2) => vec![meshes[0]],
        (Some("kTINDisplayExistingOnly"), 2) => vec![meshes[1]],
        _ => meshes,
    })
}

/// True when the faces of a mesh face inward: a closed mesh (every edge shared by two faces)
/// whose volume comes out negative, or an open one whose faces point down more than up
/// (weighted by their area).
fn facing_inward(faces: &[Vec<P3>]) -> bool {
    let mut edges: HashMap<(u64, u64), i32> = HashMap::new();
    let key = |p: &P3| p.iter().fold(0u64, |h, c| h.wrapping_mul(0x100000001b3).wrapping_add(c.to_bits()));
    let (mut volume, mut up) = (0.0, 0.0);
    for face in faces {
        for k in 0..face.len() {
            let (a, b) = (key(&face[k]), key(&face[(k + 1) % face.len()]));
            *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
        // as a fan of triangles from its first corner
        for k in 1..face.len() - 1 {
            let (a, b, c) = (face[0], face[k], face[k + 1]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            volume += a[0] * n[0] + a[1] * n[1] + a[2] * n[2];
            up += n[2];
        }
    }
    let closed = !edges.is_empty() && edges.values().all(|&n| n == 2);
    if closed {
        volume < 0.0
    } else {
        up < 0.0
    }
}

fn same_ring(a: &[P3], b: &[P3]) -> bool {
    let size = a.iter().flatten().fold(1.0f64, |m, c| m.max(c.abs()));
    a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (0..3).all(|k| (p[k] - q[k]).abs() <= 1e-6 * size))
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
pub(crate) mod tests {
    use super::*;
    use crate::shapes::tests::record;
    use vwx_core::container::{Unpacked, RECORD_HEADER};
    use vwx_core::Header;

    /// A drawn object with a solid pen.
    pub(crate) fn pen(mut r: Vec<u8>) -> Vec<u8> {
        r[0x50] = 2;
        r
    }

    /// Sets the parent address (0x26) and the reference and class numbers of a record.
    pub(crate) fn tagged(mut r: Vec<u8>, parent: u64, reference: u32, class: u32) -> Vec<u8> {
        r[0x26..0x2e].copy_from_slice(&parent.to_le_bytes());
        r[0x1a..0x1e].copy_from_slice(&reference.to_le_bytes());
        r[0x20..0x24].copy_from_slice(&class.to_le_bytes());
        r
    }

    pub(crate) fn layer(name: &str, elevation: f64) -> Vec<u8> {
        let text: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let mut r = record(kind::LAYER, 0x2a2, &[(0x20e, elevation)]);
        r.extend((text.len() as u32).to_le_bytes());
        r.extend(&text);
        r.extend([0u8; 8]);
        let n = r.len() as u64;
        r[0..8].copy_from_slice(&n.to_le_bytes());
        r
    }

    pub(crate) fn names(list: &[(u32, &str)]) -> Vec<u8> {
        let mut r = record(kind::NAMES, RECORD_HEADER + 12, &[]);
        r[RECORD_HEADER + 8..RECORD_HEADER + 12].copy_from_slice(&(list.len() as u32).to_le_bytes());
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

    pub(crate) fn unpacked(records: Vec<Vec<u8>>) -> Unpacked {
        let mut head = vec![0u8; 0x94];
        head[8..12].copy_from_slice(&0x94u32.to_be_bytes());
        head[14..32].copy_from_slice(b"VW32.0.0 VW32.0.0\0");
        Unpacked { header: Header::parse(&head).unwrap(), document: Vec::new(), body: records.concat(), blocks: 1 }
    }

    /// The box of sample 20: Rect(0, 500, 1000, 0) extruded 0..3000.
    fn box_records(parent: u64, own: u64) -> Vec<Vec<u8>> {
        let extrude = record(kind::EXTRUDE, 324, &[(0xcc, 1.0), (0xec, 1.0), (0x10c, 1.0), (0x138, 3000.0)]);
        let rect = pen(record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]));
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
    fn records_of_an_object_are_among_its_properties() {
        // a rectangle with the record of sample 90 attached: format 41, "Gegevens", field "Naam"
        let rect = pen(record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]));
        let records = vec![
            tagged(names(&[(41, "Gegevens")]), 1, 0, 0),
            tagged(crate::records::tests::format_record(&[("Naam", 4, 0)]), 1, 41, 0),
            tagged(layer("L", 0.0), 1, 1, 0),
            tagged(rect, 2, 0, 0),
            tagged(crate::records::tests::sample_90(), 3, 0, 0),
            tagged(record(kind::END, 88, &[]), 3, 0, 0),
            tagged(record(kind::END, 88, &[]), 2, 0, 0),
        ];
        let u = unpacked(records);
        let scene = scene(&Raw::new(&u).unwrap(), Info::default());
        assert!(scene.meshes[0].props[0].1.contains(r#""Gegevens.Naam":"Balk-1""#), "{}", scene.meshes[0].props[0].1);
    }

    #[test]
    fn saved_view_as_sample_140() {
        // "Aanzicht A": the model turned 60 degrees about x and 45 about z
        let (h, q) = (0.5f64.sqrt(), 0.125f64.sqrt());
        let axes = [h, h, 0.0, -q, q, 0.75f64.sqrt(), 0.375f64.sqrt(), -(0.375f64.sqrt()), 0.5];
        let reals: Vec<(usize, f64)> = axes.iter().enumerate().map(|(k, v)| (0x9c + 8 * k, *v)).collect();
        let records = vec![
            tagged(names(&[(52, "Aanzicht A")]), 1, 0, 0),
            tagged(record(kind::SAVED_VIEW, 89, &[]), 1, 52, 0),
            tagged(record(kind::VIEW, 380, &reals), 2, 0, 0),
            tagged(record(kind::END, 88, &[]), 2, 0, 0),
        ];
        let u = unpacked(records);
        let raw = Raw::new(&u).unwrap();
        let scene = scene(&raw, Info::default());
        assert_eq!(scene.views.len(), 1);
        let view = &scene.views[0];
        assert_eq!(view.name, "Aanzicht A");
        // looking down at 30 degrees, to the north-west
        let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-6);
        assert!(near(view.forward, [-0.6123724, 0.6123724, -0.5]) && near(view.up, [-0.35355338, 0.35355338, 0.8660254]), "{view:?}");
    }

    #[test]
    fn what_is_in_an_invisible_class_starts_hidden() {
        // class 7 with the visibility byte of samples 130 (invisible) and 131 (greyed)
        for (visibility, hidden) in [(0xffu8, true), (2, false), (0, false)] {
            let mut class = record(kind::CLASS, 188, &[]);
            class[0xb6] = visibility;
            let rect = pen(record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]));
            // and a rectangle in no class beside it
            let plain = pen(record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]));
            let records = vec![tagged(class, 1, 7, 0), tagged(layer("L", 0.0), 1, 1, 0), tagged(rect, 2, 0, 7), tagged(plain, 2, 0, 0), tagged(record(kind::END, 88, &[]), 2, 0, 0)];
            let u = unpacked(records);
            let raw = Raw::new(&u).unwrap();
            let scene = scene(&raw, Info::default());
            assert_eq!((scene.meshes.len(), scene.meshes[1].hidden, scene.meshes[0].hidden), (2, hidden, false), "{visibility:#x}");
        }
        // when most of what there is to see would start hidden, all of it shows
        let mut class = record(kind::CLASS, 188, &[]);
        class[0xb6] = 0xff;
        let rect = pen(record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]));
        let records = vec![tagged(class, 1, 7, 0), tagged(layer("L", 0.0), 1, 1, 0), tagged(rect, 2, 0, 7), tagged(record(kind::END, 88, &[]), 2, 0, 0)];
        let u = unpacked(records);
        let scene = scene(&Raw::new(&u).unwrap(), Info::default());
        assert!(!scene.meshes[0].hidden);
    }

    #[test]
    fn lines_and_shapes_without_a_fill_are_line_work() {
        // a line from (0, 0) to (1000, 0) and a rectangle without a fill, on a layer at 3000
        let line = pen(record(kind::LINE, 140, &[(0x80, 1000.0)]));
        let rect = pen(record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]));
        let records = vec![tagged(layer("L", 3000.0), 1, 1, 0), tagged(line, 2, 0, 0), tagged(rect, 2, 0, 0), tagged(record(kind::END, 88, &[]), 2, 0, 0)];
        let u = unpacked(records);
        let scene = scene(&Raw::new(&u).unwrap(), Info::default());
        let m = &scene.meshes[0];
        // one segment and the four sides of the rectangle; nothing filled
        assert_eq!((m.indices.len(), m.lines.len(), m.line_elements.len(), m.props.len()), (0, 10, 2, 2));
        assert_eq!(m.line_elements[1], (2, 2, 8));
        assert_eq!(&m.positions[..6], &[0.0, 0.0, 3.0, 1.0, 0.0, 3.0]);
        assert_eq!(m.normals.len(), m.positions.len());
    }

    #[test]
    fn what_is_on_an_invisible_layer_starts_hidden() {
        let rect = || {
            let mut r = pen(record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]));
            r[0x64] = 1;
            r
        };
        // as the second layer of sample 151
        let mut off = layer("Tweede laag", 0.0);
        off[0xd8..0xda].copy_from_slice(&[0xff, 0xff]);
        let records = vec![
            tagged(layer("L", 0.0), 1, 1, 0),
            tagged(rect(), 2, 0, 0),
            tagged(record(kind::END, 88, &[]), 2, 0, 0),
            tagged(off, 1, 2, 0),
            tagged(rect(), 3, 0, 0),
            tagged(record(kind::END, 88, &[]), 3, 0, 0),
        ];
        let u = unpacked(records);
        let scene = scene(&Raw::new(&u).unwrap(), Info::default());
        assert_eq!(scene.meshes.iter().map(|m| (m.name.as_str(), m.hidden)).collect::<Vec<_>>(), [("L", false), ("Tweede laag", true)]);
    }

    #[test]
    fn a_plug_in_object_places_what_it_holds() {
        // as a window of the corpus: put at (2000, 8050), its line from (0, 0) to (1000, 0)
        let object = record(kind::PLUG_IN_OBJECT, 433, &[(0xf0, 1.0), (0x110, 1.0), (0x130, 1.0), (0x138, 2000.0), (0x140, 8050.0)]);
        let line = pen(record(kind::LINE, 140, &[(0x80, 1000.0)]));
        let records = vec![tagged(layer("L", 0.0), 1, 1, 0), tagged(object, 2, 0, 0), tagged(line, 3, 0, 0), tagged(record(kind::END, 88, &[]), 3, 0, 0), tagged(record(kind::END, 88, &[]), 2, 0, 0)];
        let u = unpacked(records);
        let scene = scene(&Raw::new(&u).unwrap(), Info::default());
        assert_eq!(&scene.meshes[0].positions[..6], &[2.0, 8.05, 0.0, 3.0, 8.05, 0.0]);
    }

    #[test]
    fn the_plan_holds_the_2d_shapes_of_objects_that_are_3d_too() {
        // a window at (2000, 0) holding a line of 1000 (its plan) and an extrude (its 3D form)
        let object = record(kind::PLUG_IN_OBJECT, 433, &[(0xf0, 1.0), (0x110, 1.0), (0x130, 1.0), (0x138, 2000.0)]);
        let records = vec![
            tagged(layer("L", 0.0), 1, 1, 0),
            tagged(object, 2, 0, 0),
            tagged(pen(record(kind::LINE, 140, &[(0x80, 1000.0)])), 3, 0, 0),
            tagged(record(kind::EXTRUDE, 324, &[]), 3, 0, 0),
            tagged(record(kind::END, 88, &[]), 3, 0, 0),
            tagged(record(kind::END, 88, &[]), 2, 0, 0),
        ];
        let u = unpacked(records);
        let plans = crate::plan::plans(&Raw::new(&u).unwrap());
        assert_eq!(plans.len(), 1);
        assert_eq!((plans[0].layer.as_str(), plans[0].lines.as_slice()), ("L", &[[[2.0, 0.0], [3.0, 0.0]]][..]));
        assert_eq!(plans[0].elements, [(1, 0, 1)]);
    }

    #[test]
    fn meshes_are_turned_to_face_up_or_out() {
        // an open square clockwise seen from above faces down: it is turned; one the other way
        // round is not
        let clockwise = vec![vec![[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 0.0]]];
        assert!(facing_inward(&clockwise));
        let counter: Vec<Vec<P3>> = clockwise.iter().map(|f| f.iter().rev().copied().collect()).collect();
        assert!(!facing_inward(&counter));
        // a closed tetrahedron with its faces outward is not turned, inward it is
        let [a, b, c, d] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let outward = vec![vec![a, c, b], vec![a, b, d], vec![a, d, c], vec![b, c, d]];
        assert!(!facing_inward(&outward));
        let inward: Vec<Vec<P3>> = outward.iter().map(|f| f.iter().rev().copied().collect()).collect();
        assert!(facing_inward(&inward));
    }

    #[test]
    fn private_layers_are_not_drawn() {
        // a line on a layer of the document and one on a private layer of Vectorworks
        let records = vec![
            tagged(layer("L", 0.0), 1, 1, 0),
            tagged(pen(record(kind::LINE, 140, &[(0x80, 1000.0)])), 2, 0, 0),
            tagged(record(kind::END, 88, &[]), 2, 0, 0),
            tagged(layer("__NNA_PrivateLayer", 0.0), 1, 2, 0),
            tagged(pen(record(kind::LINE, 140, &[(0x80, 1000.0)])), 3, 0, 0),
            tagged(record(kind::END, 88, &[]), 3, 0, 0),
        ];
        let u = unpacked(records);
        let raw = Raw::new(&u).unwrap();
        let scene = scene(&raw, Info::default());
        assert_eq!(scene.meshes.len(), 1);
        assert_eq!(scene.meshes[0].layer, "L");
        assert_eq!(crate::plan::plans(&raw).len(), 1);
    }

    #[test]
    fn what_cuts_the_hole_of_a_window_is_not_drawn() {
        // a window with a line, and a group tagged `gchw` with the shape that cuts its wall
        let object = record(kind::PLUG_IN_OBJECT, 433, &[(0xf0, 1.0), (0x110, 1.0), (0x130, 1.0)]);
        let mut tag = record(kind::USER_DATA, 84, &[]);
        tag[0x50..0x54].copy_from_slice(b"gchw");
        let records = vec![
            tagged(layer("L", 0.0), 1, 1, 0),
            tagged(object, 2, 0, 0),
            tagged(pen(record(kind::LINE, 140, &[(0x80, 1000.0)])), 3, 0, 0),
            tagged(record(kind::GROUP, 204, &[]), 3, 0, 0),
            tagged(tag, 4, 0, 0),
            tagged(pen(record(kind::LINE, 140, &[(0x80, 6000.0)])), 4, 0, 0),
            tagged(record(kind::END, 88, &[]), 4, 0, 0),
            tagged(record(kind::END, 88, &[]), 3, 0, 0),
            tagged(record(kind::END, 88, &[]), 2, 0, 0),
        ];
        let u = unpacked(records);
        let scene = scene(&Raw::new(&u).unwrap(), Info::default());
        assert_eq!(scene.meshes[0].positions, [0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_sheet_layer_gives_a_sheet_with_its_lines_and_texts() {
        // sheet 001 "Plans": a line, and a title block (a plug-in object at (100, 50)) with a text
        let mut sheet = layer("001", 0.0);
        sheet[0x24c] = 2;
        // its title, after its name
        let end = sheet.len() - 8;
        sheet.truncate(end);
        let title: Vec<u8> = "Plans".encode_utf16().flat_map(u16::to_le_bytes).collect();
        sheet.extend((title.len() as u32).to_le_bytes());
        sheet.extend(&title);
        let n = sheet.len() as u64;
        sheet[0..8].copy_from_slice(&n.to_le_bytes());
        let line = pen(record(kind::LINE, 140, &[(0x80, 420.0)]));
        let mut kept = record(kind::GROUP, 204, &[]);
        kept[0x55] = 1;
        let mut cut = record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]);
        cut[0x64] = 1;
        let block = record(kind::PLUG_IN_OBJECT, 433, &[(0xf0, 1.0), (0x110, 1.0), (0x130, 1.0), (0x138, 100.0), (0x140, 50.0)]);
        let mut text = record(kind::TEXT, 0x19c, &[(0x68, 10.0), (0x70, 5.0), (0x78, 6.0), (0x80, 45.0), (0x108, 12.0)]);
        text[0xa6] = 5;
        text[0x100] = 1;
        text.extend("Plans".encode_utf16().flat_map(u16::to_le_bytes));
        let n = text.len() as u64;
        text[0..8].copy_from_slice(&n.to_le_bytes());
        let records = vec![
            tagged(names(&[(77, "GF Plan")]), 1, 0, 0),
            tagged(sheet, 1, 1, 0),
            tagged(line, 2, 0, 0),
            tagged(block, 2, 0, 0),
            tagged(text, 3, 0, 0),
            tagged(record(kind::END, 88, &[]), 3, 0, 0),
            // a plan of 1 : 50 as the first of the corpus, the origin of the model at (-200, 100)
            tagged(
                record(kind::VIEWPORT, 1082, &[(0x68, 275.0), (0x70, -369.0), (0x78, 27.0), (0x80, 172.0), (0xd4, -10000.0), (0xdc, 5000.0), (0x23e, 1.0), (0x25e, 1.0), (0x27e, 1.0), (0x2e4, 50.0)]),
                2,
                77,
                0,
            ),
            // with a group that keeps a line of its drawing: 5 m long, in the millimetres of the view
            tagged(kept, 4, 0, 0),
            tagged(pen(record(kind::LINE, 140, &[(0x80, 5000.0)])), 5, 0, 0),
            // and what it cuts: a rectangle of 1000 x 500 with a fill and no pen
            tagged(cut, 5, 0, 0),
            tagged(record(kind::END, 88, &[]), 5, 0, 0),
            // and a group of annotations: a line of 2.5 m in the millimetres of the sheet times 50
            tagged(record(kind::GROUP, 204, &[]), 4, 0, 0),
            tagged(pen(record(kind::LINE, 140, &[(0x80, 2500.0)])), 6, 0, 0),
            tagged(record(kind::END, 88, &[]), 6, 0, 0),
            tagged(record(kind::END, 88, &[]), 4, 0, 0),
            tagged(record(kind::END, 88, &[]), 2, 0, 0),
        ];
        let u = unpacked(records);
        let scene = scene(&Raw::new(&u).unwrap(), Info::default());
        assert!(scene.meshes.is_empty());
        let sheet = &scene.sheets[0];
        assert_eq!((sheet.number.as_str(), sheet.title.as_str()), ("001", "Plans"));
        let v = &sheet.viewports[0];
        assert_eq!((v.name.as_str(), v.min, v.max, v.scale, v.forward), ("GF Plan", [-0.369, 0.027], [0.172, 0.275], 50.0, [0.0, 0.0, -1.0]));
        assert_eq!(v.origin, [-0.2, 0.1]);
        // it names no layers: it shows none of the design layers (here there are none)
        assert!(v.hidden_layers.is_empty() && v.hidden_classes.is_empty());
        // its own line, and the line its viewport keeps: 100 mm long on the sheet, from the origin of the view
        // then the annotation of the viewport (50 mm on the sheet), then the line the viewport keeps
        // (100 mm, from the origin of the view)
        assert_eq!(sheet.lines, [[[0.0, 0.0], [0.42, 0.0]], [[0.0, 0.0], [0.05, 0.0]], [[-0.2, 0.1], [-0.1, 0.1]]]);
        // what the viewport cuts, filled: 20 mm by 10 mm on the sheet, from the origin of the view
        assert_eq!(sheet.fills.len(), 1);
        assert_eq!(sheet.fills[0].outline, [[-0.2, 0.1], [-0.18, 0.1], [-0.18, 0.11], [-0.2, 0.11]]);
        // the text of the title block, where the block is
        assert_eq!((sheet.texts[0].text.as_str(), sheet.texts[0].at), ("Plans", [0.105, 0.06, 0.0]));
    }

    #[test]
    fn sheet_layers_are_not_part_of_the_model() {
        let rect = || {
            let mut r = pen(record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]));
            r[0x64] = 1;
            r
        };
        let mut sheet = layer("001", 0.0);
        sheet[0x24c] = 2;
        let mut design = layer("L", 0.0);
        design[0x24c] = 1;
        let records = vec![
            tagged(design, 1, 1, 0),
            tagged(rect(), 2, 0, 0),
            tagged(record(kind::END, 88, &[]), 2, 0, 0),
            tagged(sheet, 1, 2, 0),
            tagged(rect(), 3, 0, 0),
            tagged(record(kind::END, 88, &[]), 3, 0, 0),
        ];
        let u = unpacked(records);
        let scene = scene(&Raw::new(&u).unwrap(), Info::default());
        assert_eq!(scene.meshes.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["L"]);
    }

    #[test]
    fn rectangle_on_a_layer_is_a_flat_face() {
        let mut rect = pen(record(kind::RECTANGLE, 188, &[(0xa4, 1.0), (0xac, 1000.0), (0xb4, 500.0)]));
        // with a solid fill
        rect[0x64] = 1;
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
