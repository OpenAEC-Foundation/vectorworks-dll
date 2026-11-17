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
    /// The named resources of the document, as its resource browser lists them.
    #[serde(default)]
    pub resources: Vec<Resource>,
}

/// A named resource: a symbol definition, a style, a text style, a script, a record format, a
/// folder that holds others, a font; also the description of a class or layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    /// The object it is, as in [`Object::id`] (0 for what has no object of its own).
    pub id: u64,
    /// Its kind, as [`Object::kind`] names it (`symbol definition`, `text style`, `document
    /// script`, `symbol folder`, `font`, `class`, `layer`, ..).
    pub kind: String,
    pub name: String,
    /// The folders it is in, the outermost first; for a script or a saved view the palette.
    #[serde(default)]
    pub folder: Vec<String>,
    /// What else is known of it: the `text` of a script, the `size` of a text style in points,
    /// the `description` of a class or layer, the `font` number.
    #[serde(default)]
    pub fields: Vec<(String, Value)>,
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
