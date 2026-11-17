//! Neutral model of a Vectorworks document, and the meshes a viewer draws.
//!
//! Units: metres; Z up; right-handed, as Vectorworks shows a model in 3D. The reader converts
//! from the document's internal units, so callers never see them.
//!
//! The model grows with the reader. Objects that have no typed struct yet are still reachable as
//! an [`Object`] with their raw fields, so that every object of a file can be listed and queried
//! before it is fully decoded.

use serde::{Deserialize, Serialize};

/// Facts about a document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Info {
    /// The file format version as stored in the file (e.g. a Vectorworks year), once known.
    pub version: Option<String>,
    /// The application that saved the file, when the file says.
    pub application: Option<String>,
    /// Size of the file in bytes.
    pub size: u64,
    /// Number of objects found.
    pub objects: usize,
}

/// A design layer or sheet layer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    /// Sheet layer (drawing sheet) rather than design layer.
    pub sheet: bool,
    /// Elevation of the layer (metres).
    pub elevation: f64,
    /// Wall height of the layer (metres), when set.
    pub height: Option<f64>,
    pub visible: bool,
}

/// A class (Vectorworks' second organisation next to layers).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Class {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    /// Fill colour (sRGB, 0..1), when the class sets one.
    pub fill: Option<[f32; 3]>,
}

/// A field value of an object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Real(f64),
    Text(String),
    Point([f64; 3]),
    Ref(u64),
    List(Vec<Value>),
}

/// Any object of the document: its kind and its fields, decoded or raw.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Object {
    pub id: u64,
    /// Object kind, e.g. `line`, `rectangle`, `extrude`, `wall`, `symbol`; `unknown:<tag>` for
    /// kinds not decoded yet.
    pub kind: String,
    pub name: Option<String>,
    pub layer: Option<u64>,
    pub class: Option<u64>,
    pub parent: Option<u64>,
    pub fields: Vec<(String, Value)>,
}

/// A document as read.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Document {
    pub info: Info,
    pub layers: Vec<Layer>,
    pub classes: Vec<Class>,
    pub objects: Vec<Object>,
    /// The line types (dash patterns) of the document.
    #[serde(default)]
    pub line_types: Vec<LineType>,
    /// The hatches of the document.
    #[serde(default)]
    pub hatches: Vec<Hatch>,
    /// The gradients of the document.
    #[serde(default)]
    pub gradients: Vec<Gradient>,
    /// The tile fills of the document.
    #[serde(default)]
    pub tiles: Vec<Tile>,
    /// The image fills of the document.
    #[serde(default)]
    pub image_fills: Vec<ImageFill>,
}

/// A triangle mesh for a viewer: one per layer/class/material batch.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mesh {
    /// Name to show, e.g. `Design Layer-1 / Walls`.
    pub name: String,
    /// Base colour (sRGB, 0..1).
    pub color: [f32; 3],
    /// xyz per vertex, metres, Z up.
    pub positions: Vec<f32>,
    /// xyz per vertex, unit length.
    pub normals: Vec<f32>,
    /// Optional colour per vertex (sRGB RGBA8); overrides `color`.
    pub colors: Option<Vec<[u8; 4]>>,
    /// Triangle list.
    pub indices: Vec<u32>,
    /// Pickable objects: object id, first index and index count in `indices`, by first index.
    pub elements: Vec<(u64, u32, u32)>,
    /// Object properties as flat JSON objects, by object id.
    pub props: Vec<(u64, String)>,
    /// The layer and the class what it holds is in, by name (empty: none).
    pub layer: String,
    pub class: String,
    /// Hidden in the file (an invisible layer or class): starts switched off.
    pub hidden: bool,
    /// Line segments (lines, arcs, the outlines of shapes without a fill): pairs of vertex
    /// numbers. Their vertices are not used by `indices`.
    pub lines: Vec<u32>,
    /// The objects among the line segments: (object id, first index into `lines`, count).
    pub line_elements: Vec<(u64, u32, u32)>,
}

/// What a viewer needs of a document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scene {
    pub info: Info,
    pub meshes: Vec<Mesh>,
    /// The views saved in the document.
    pub views: Vec<View>,
    /// The texts on the design layers.
    pub texts: Vec<Text>,
    /// The sheet layers: the drawing sheets of the document.
    pub sheets: Vec<Sheet>,
}

/// A sheet layer: what is drawn on the paper itself (its title block, notes), in metres on
/// paper, x right and y up. The views of the model it shows (its viewports) are not read yet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub id: u64,
    /// The name of the layer, by custom the number of the sheet.
    pub number: String,
    pub title: String,
    /// Line segments, each from one point to another.
    pub lines: Vec<[[f32; 2]; 2]>,
    /// Regions filled in one colour, under the lines: what a section viewport cuts.
    pub fills: Vec<Fill>,
    pub texts: Vec<Text>,
    /// The views of the model placed on it.
    pub viewports: Vec<Viewport>,
}

/// A region of a sheet filled in one colour (red, green, blue): its outline, and the object
/// it belongs to.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    pub id: u64,
    pub outline: Vec<[f32; 2]>,
    pub color: [u8; 3],
}

/// A viewport: a view of the model on a sheet (metres on paper; directions in the model, Z up).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub id: u64,
    pub name: String,
    /// The box it takes on the sheet: its lower left and upper right corner.
    pub min: [f32; 2],
    pub max: [f32; 2],
    /// 1 : `scale`.
    pub scale: f32,
    /// Where the origin of the model lies on the sheet.
    pub origin: [f32; 2],
    /// From the eye into the model, and up on the sheet.
    pub forward: [f32; 3],
    pub up: [f32; 3],
    /// True for a section viewport: the sheet has the drawing of the cut it keeps (among the
    /// lines of the sheet), which a view of the whole model would not be.
    pub section: bool,
    /// The design layers it does not show, and the classes it does not show, by name.
    pub hidden_layers: Vec<String>,
    pub hidden_classes: Vec<String>,
}

/// A text of the document (metres, Z up).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Text {
    /// The object it is.
    pub id: u64,
    /// The name of its layer.
    pub layer: String,
    pub text: String,
    /// The top left corner of its box, and the width and height of the box.
    pub at: [f32; 3],
    pub width: f32,
    pub height: f32,
    /// The size of its letters in points on paper.
    pub size: f32,
    pub lines: u16,
}

/// A saved view: the way the model is looked at (Z up; unit vectors).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct View {
    pub name: String,
    /// From the eye into the model.
    pub forward: [f32; 3],
    /// Up on the screen.
    pub up: [f32; 3],
}

/// A line type: the dashes lines drawn with it have. Lengths in metres on paper (a line on a
/// design layer at 1 : 50 has dashes 50 times as long in the model).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LineType {
    /// Its reference number: objects name it by minus this number.
    pub id: u64,
    pub name: String,
    /// Dash, gap, dash, gap, ...; empty for a line type of geometry.
    pub dashes: Vec<f64>,
    /// The dashes grow with the weight of the line.
    pub scale_with_thickness: bool,
    /// Drawn with geometry repeated along the line (not decoded: such lines are drawn solid).
    pub complex: bool,
}

/// A hatch: families of parallel dashed lines filling a shape.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hatch {
    /// Its reference number: objects name it by minus this number.
    pub id: u64,
    pub name: String,
    /// Its lengths are on paper (they scale with the layer) rather than in the model.
    pub page: bool,
    pub rotate_in_wall: bool,
    pub rotate_in_symbol: bool,
    /// Its background is not drawn.
    pub transparent: bool,
    /// The colour of its background (sRGB).
    pub background: [u8; 3],
    pub levels: Vec<HatchLevel>,
}

/// One family of parallel lines of a hatch, in metres (on paper for a page-based hatch), x
/// right and y up in the frame of the filled object's fill.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HatchLevel {
    /// A point a dash starts at.
    pub start: [f64; 2],
    /// The direction of the lines, as long as a dash and its gap.
    pub repeat: [f64; 2],
    /// From one line to the next.
    pub offset: [f64; 2],
    /// The part of `repeat` drawn (1: solid lines).
    pub dash: f64,
    pub color: [u8; 3],
    /// Line weight in thousandths of an inch.
    pub weight: u16,
}

/// A gradient fill: colours at places from 0 to 1.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub id: u64,
    pub name: String,
    pub stops: Vec<GradientStop>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub position: f64,
    /// Where between this stop and the next their colours mix half and half (0 to 1).
    pub midpoint: f64,
    pub color: [u8; 3],
    /// Opacity in percent.
    pub opacity: u16,
}

/// A tile fill: geometry repeated on a grid (metres).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub id: u64,
    pub name: String,
    /// The two steps from one tile to its neighbours.
    pub repeat: [f64; 2],
    pub offset: [f64; 2],
    pub background: [u8; 3],
    /// The line segments of one tile.
    pub lines: Vec<[[f64; 2]; 2]>,
}

/// An image fill: the image file as stored (a PNG or JPEG file).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImageFill {
    pub id: u64,
    pub name: String,
    /// `png`, `jpeg`, ...
    pub format: String,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<u8>,
}
