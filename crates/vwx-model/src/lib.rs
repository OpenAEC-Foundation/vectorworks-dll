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
    /// The scale of the layer, 1 : `scale` (0 when not read).
    #[serde(default)]
    pub scale: f64,
    /// The level type the layer is linked to ("Top of Slab"), empty when none.
    #[serde(default)]
    pub level: String,
    /// The story it belongs to, by name, when it does.
    #[serde(default)]
    pub story: Option<String>,
    /// Greyed (shown dimmed) rather than shown normally.
    #[serde(default)]
    pub greyed: bool,
    /// Thickness of the layer (metres), where the layer has one.
    #[serde(default)]
    pub thickness: Option<f64>,
    /// Opacity of what is on the layer, 0..1 (1: fully opaque), when the layer sets one.
    #[serde(default)]
    pub opacity: Option<f64>,
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
    #[serde(default)]
    pub settings: DocumentSettings,
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
    /// The plan of each design layer with 2D line work: what the top/plan view shows.
    #[serde(default)]
    pub plans: Vec<Plan>,
    /// The units the document is set in, its origin, georeference and project data.
    #[serde(default)]
    pub units: Units,
    #[serde(default)]
    pub origin: Origin,
    #[serde(default)]
    pub georeference: Option<Georeference>,
    /// The stories of the building, lowest first, with their levels and layers.
    #[serde(default)]
    pub stories: Vec<Story>,
    /// The level types a layer can be linked to ("Top of Slab", "Finish Floor").
    #[serde(default)]
    pub level_types: Vec<String>,
    /// The levels a new story gets.
    #[serde(default)]
    pub level_templates: Vec<LevelTemplate>,
    #[serde(default)]
    pub project: ProjectInfo,
    #[serde(default)]
    pub page: Page,
}

/// Everything a document says about itself besides its objects: what [`Scene`] carries at its
/// top level, for [`Document`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentSettings {
    pub units: Units,
    pub origin: Origin,
    pub georeference: Option<Georeference>,
    pub stories: Vec<Story>,
    pub level_types: Vec<String>,
    pub level_templates: Vec<LevelTemplate>,
    pub project: ProjectInfo,
    pub page: Page,
}

/// How a length is written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitFormat {
    /// 1.25 m
    #[default]
    Decimal,
    /// 1 1/4 inch
    Fractional,
    /// 3' 4.5"
    DecimalFeetInches,
    /// 3' 4 1/2"
    FractionalFeetInches,
}

/// One set of units (the document has a primary and a secondary one).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UnitSystem {
    /// Name of the length unit as the document gives it ("Millimeter", "Feet & Inches").
    pub name: String,
    /// The mark shown after a length ("mm").
    pub mark: String,
    pub format: UnitFormat,
    /// Drawing units per inch: 25.4 for millimetres, 1 for inches.
    pub units_per_inch: f64,
    /// Metres in one drawing unit (0.001 for millimetres, 0.0254 for inches).
    pub metres_per_unit: f64,
    /// Decimals shown for lengths, or the fraction precision as the document counts it.
    pub display_precision: u32,
    /// The same for dimensions.
    pub dimension_precision: u32,
    pub leading_zero: bool,
    pub trailing_zeros: bool,
    pub show_mark: bool,
    pub thousands_separator: bool,
    pub area_name: String,
    pub area_mark: String,
    /// Area units in one square inch.
    pub area_units_per_square_inch: f64,
    pub area_precision: u32,
    pub volume_name: String,
    pub volume_mark: String,
    /// Volume units in one cubic inch.
    pub volume_units_per_cubic_inch: f64,
    pub volume_precision: u32,
}

/// How an angle is written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AngleUnit {
    #[default]
    Degrees,
    Radians,
    Gradians,
}

/// The units of a document. Coordinates inside the file are millimetres whatever these are; the
/// reader has converted them to metres already, so these only say how to *show* values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Units {
    pub primary: UnitSystem,
    pub secondary: UnitSystem,
    pub angle: AngleUnit,
}

/// The user origin: where the coordinates the user sees start, in the coordinates of the model
/// (metres, x right, y up).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Origin {
    pub x: f64,
    pub y: f64,
}

/// A georeference: where on Earth the origin of the model lies.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Georeference {
    /// Degrees, positive north / east.
    pub latitude: f64,
    pub longitude: f64,
    /// The angle between the y axis of the model and true north, in degrees.
    pub angle_to_north: f64,
    /// Elevation of the project above the datum (metres).
    pub project_elevation: f64,
    /// The coordinate system as Well Known Text, as the document keeps it.
    pub wkt: String,
    /// The EPSG code of the coordinate system, when the text names one.
    pub epsg: Option<u32>,
    /// Name of the coordinate system ("Amersfoort / RD New").
    pub crs_name: String,
}

/// A level of a story: a layer's place in it (a floor, a ceiling).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Level {
    /// The level type: "Finish Floor", "Top of Slab".
    pub name: String,
    /// Elevation above the story (metres).
    pub elevation: f64,
    /// The layer of the level; empty for a level without one.
    pub layer: String,
}

/// A story of a building.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Story {
    pub name: String,
    /// What is added to the name of its layers.
    pub suffix: String,
    /// Elevation of the story (metres).
    pub elevation: f64,
    pub levels: Vec<Level>,
    /// The names of the design layers that belong to it.
    pub layers: Vec<String>,
}

/// A level that a new story is given.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LevelTemplate {
    /// Name of the layer made for it; empty: the level has no layer.
    pub layer: String,
    /// 1 : `scale` for that layer.
    pub scale: f64,
    pub level_type: String,
    /// Elevation in the story, and the height of walls on its layer (metres).
    pub elevation: f64,
    pub wall_height: f64,
}

/// The project data of the title block (name, number, address, ...).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub number: String,
    pub address: String,
    pub client: String,
    pub date: String,
    pub file_name: String,
    /// Every field of the record the data comes from, in the document's own words.
    pub fields: Vec<(String, String)>,
}

/// The paper the document is set up for, and how many pages its drawing area has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Page {
    /// The printable area of one page in metres on paper.
    pub width: f64,
    pub height: f64,
    /// The drawing area in pages across and down.
    pub columns: u16,
    pub rows: u16,
}

/// The plan of a design layer: its 2D line work as seen from above, the plan views of its
/// objects included (walls, doors, windows, symbols), in metres, x right and y up.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// The name of the layer.
    pub layer: String,
    /// Line segments, each from one point to another.
    pub lines: Vec<[[f32; 2]; 2]>,
    /// The objects among the segments: (object id, first segment, count).
    pub elements: Vec<(u64, u32, u32)>,
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
