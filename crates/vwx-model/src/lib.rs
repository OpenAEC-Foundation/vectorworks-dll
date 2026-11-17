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
    /// The worksheets (spreadsheets) of the document, with where they are placed on drawings.
    #[serde(default)]
    pub worksheets: Vec<Worksheet>,
}

/// A worksheet: a spreadsheet of the document. Rows and columns count from 1, as in the
/// scripting interface; row 0 and column 0 do not hold cells here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Worksheet {
    pub id: u64,
    pub name: String,
    pub rows: u32,
    pub columns: u32,
    /// The cells that hold something (text, a number or a formula), by row then column.
    pub cells: Vec<Cell>,
    /// Where drawings show it as an image.
    #[serde(default)]
    pub placements: Vec<WorksheetPlacement>,
    /// Width of each column in pixels (72 per inch), column 1 first.
    #[serde(default)]
    pub column_widths: Vec<u32>,
    /// Height of each row in pixels, row 1 first.
    #[serde(default)]
    pub row_heights: Vec<u32>,
    /// The window of the worksheet on screen: top, left, bottom, right in pixels (zeros: never
    /// shown).
    #[serde(default)]
    pub window: [u32; 4],
    /// The rows that list objects by a criteria (database rows) with the rows they computed.
    #[serde(default)]
    pub database_rows: Vec<DatabaseRow>,
}

/// A worksheet cell.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub row: u32,
    pub column: u32,
    /// What the cell shows: its text, or its value as text.
    pub text: String,
    /// The formula of the cell with its leading `=`, as `=A1+B1`; `None` when the cell
    /// holds plain text or a number, or a formula the reader cannot write out.
    pub formula: Option<String>,
    /// The value of a number or of a formula that works out to one.
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    /// Size of the letters in points.
    #[serde(default)]
    pub size: u32,
    /// `general`, `left`, `center` or `right`.
    #[serde(default)]
    pub align: String,
    /// Number format as the scripting interface numbers it (0 general, 1 fixed, 2 with commas,
    /// 3 scientific, 4 fractional, 5 dimension, ..) and its accuracy.
    #[serde(default)]
    pub number_format: u32,
    #[serde(default)]
    pub accuracy: u32,
    /// Palette index of the colour of the text.
    #[serde(default)]
    pub text_color: u32,
    /// The merged range this cell is part of: top row, left column, bottom row, right column.
    #[serde(default)]
    pub merged: Option<[u32; 4]>,
}

/// A database row of a worksheet: its criteria and the rows Vectorworks worked out from it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DatabaseRow {
    /// The row of the worksheet.
    pub row: u32,
    /// The criteria, written as the formula `=DATABASE(..)`.
    pub criteria: Option<String>,
    /// The rows computed, each the cells that hold something (their `row` is that of the
    /// database row).
    pub subrows: Vec<Vec<Cell>>,
}

/// A worksheet shown on a drawing: an image of it on a layer or sheet, in metres.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorksheetPlacement {
    pub id: u64,
    /// The layer (design or sheet) it is on: its name, and its id in the document.
    pub layer: String,
    pub layer_id: Option<u64>,
    /// Top left corner on the layer (x right, y up), and the size.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
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
