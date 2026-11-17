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
