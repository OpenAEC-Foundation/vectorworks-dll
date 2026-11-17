//! A file as records with their tree, and the document-wide tables: names, classes, layers.

use std::collections::HashMap;

use vwx_core::container::{self, Record, Unpacked};
use vwx_core::{le, Result};

/// Object types the reader names (scripting API numbering).
pub mod kind {
    pub const END: u8 = 0;
    pub const LINE: u8 = 2;
    pub const RECTANGLE: u8 = 3;
    pub const OVAL: u8 = 4;
    pub const POLYGON: u8 = 5;
    pub const ARC: u8 = 6;
    pub const GROUP: u8 = 11;
    pub const SYMBOL: u8 = 15;
    pub const SYMBOL_DEFINITION: u8 = 16;
    pub const EXTRUDE: u8 = 24;
    pub const POLYGON_3D: u8 = 25;
    pub const LAYER: u8 = 31;
    pub const MESH: u8 = 40;
    pub const MESH_FACE: u8 = 41;
    /// The points Vectorworks worked out for a profile inside a 3D object.
    pub const PROFILE_GRID: u8 = 42;
    pub const NAMES: u8 = 55;
    pub const USER_DATA: u8 = 76;
    pub const SWEEP: u8 = 34;
    pub const MULTIPLE_EXTRUDE: u8 = 38;
    pub const CSG_SOLID: u8 = 84;
    pub const NURBS_SURFACE: u8 = 113;
    /// The cached solid body of its parent: wire frame and Parasolid data.
    pub const BODY: u8 = 114;
    /// The mesh of a primitive solid, a child of it.
    pub const POLY_MESH: u8 = 85;
    pub const CLASS: u8 = 94;
    /// A primitive solid: sphere, hemisphere, cone.
    pub const SOLID: u8 = 95;
}

/// The records of a file with their tree.
pub struct Raw<'a> {
    pub records: Vec<Record<'a>>,
    pub parents: Vec<Option<usize>>,
    /// The children of each record, in file order.
    pub children: Vec<Vec<usize>>,
    /// Names by reference number (classes, symbol definitions, record formats, named objects).
    pub names: HashMap<u32, String>,
    /// The record of each reference number that a record carries.
    pub by_ref: HashMap<u32, usize>,
    /// The colours of the document.
    pub palette: crate::palette::Palette,
}

impl<'a> Raw<'a> {
    pub fn new(unpacked: &'a Unpacked) -> Result<Raw<'a>> {
        let records = container::records(&unpacked.body)?;
        let parents = container::parents(&records);
        let mut children = vec![Vec::new(); records.len()];
        let mut by_ref = HashMap::new();
        let mut names = HashMap::new();
        let mut palette = None;
        for (i, r) in records.iter().enumerate() {
            if let Some(p) = parents[i] {
                children[p].push(i);
            }
            match r.kind() {
                kind::NAMES => names.extend(name_list(r.bytes)),
                kind::USER_DATA => {
                    // the palette is user data of the document itself
                    if palette.is_none() && parents[i].is_none() {
                        palette = crate::dmdt::Data::parse(r).filter(|d| &d.key == b"DCDN").and_then(|d| crate::palette::Palette::from_data(&d));
                    }
                }
                kind::END => {}
                _ if r.ref_id() != 0 => {
                    by_ref.entry(r.ref_id()).or_insert(i);
                }
                _ => {}
            }
        }
        Ok(Raw { records, parents, children, names, by_ref, palette: palette.unwrap_or_default() })
    }

    /// The layer a record is on: its nearest ancestor that is a layer.
    pub fn layer_of(&self, mut i: usize) -> Option<usize> {
        loop {
            if self.records[i].kind() == kind::LAYER {
                return Some(i);
            }
            i = self.parents[i]?;
        }
    }

    /// The name of the object of a record, when it has one.
    pub fn name(&self, i: usize) -> Option<&str> {
        let r = &self.records[i];
        match r.kind() {
            kind::LAYER => None,
            _ => self.names.get(&r.ref_id()).map(String::as_str),
        }
    }
}

/// The name list (type 55): after a 20-byte head, entries of `u32` reference number, `u32`
/// length in bytes and that many bytes of UTF-16LE.
fn name_list(record: &[u8]) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let mut at = container::RECORD_HEADER + 20;
    while let (Some(id), Some(len)) = (le::u32(record, at), le::u32(record, at + 4)) {
        let Some(text) = record.get(at + 8..at + 8 + len as usize) else { break };
        out.push((id, utf16(text)));
        at += 8 + len as usize;
    }
    out
}

pub fn utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
    String::from_utf16_lossy(&units)
}

/// A layer record (type 31): its name is the last field, a `u32` length in bytes and UTF-16LE,
/// followed by 8 bytes; its elevation is an `f64` at 0x20e.
pub fn layer_name(record: &[u8]) -> Option<String> {
    let n = record.len();
    // the length field sits where its value says the name ends 8 bytes before the record does
    (2..=510usize).step_by(2).find_map(|len| {
        let at = n.checked_sub(8 + len + 4)?;
        (le::u32(record, at)? as usize == len).then(|| utf16(&record[at + 4..at + 4 + len]))
    })
}

pub fn layer_elevation(record: &[u8]) -> f64 {
    le::f64(record, 0x20e).filter(|z| z.is_finite()).unwrap_or(0.0)
}
