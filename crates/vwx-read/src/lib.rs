//! Reads a Vectorworks `.vwx` file (or `.sta` template) into the neutral model
//! ([`vwx_model::Document`]) and into meshes for a viewer ([`vwx_model::Scene`]).
//!
//! ```no_run
//! let bytes = std::fs::read("house.vwx")?;
//! let scene = vwx_read::read(&bytes)?;
//! for mesh in &scene.meshes {
//!     println!("{}: {} triangles", mesh.name, mesh.indices.len() / 3);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Everything works on bytes, so the reader also runs in the browser.
//!
//! **Status:** every object of a file is listed with its type, name, layer, class and parent
//! ([`open`]); [`read`] draws extrudes, 3D polygons, meshes, flat 2D shapes, placed symbols and
//! the solids kept as Parasolid bodies (walls, CSG solids, sweeps, multiple extrudes, NURBS
//! surfaces). See `docs/ARCHITECTURE.md` for what is open.

pub mod dmdt;
pub mod kinds;
pub mod mesh;
pub mod palette;
pub mod raw;
pub mod records;
pub mod shapes;
pub mod tri;

pub use vwx_core::{Error, Result};
pub use vwx_model as model;
use vwx_model::{Class, Document, Info, Layer, Object, Scene};

use raw::{kind, Raw};

/// True when `data` starts like a Vectorworks file (also the old versions the reader refuses).
pub fn sniff(data: &[u8]) -> bool {
    vwx_core::Header::parse(data).is_some()
}

/// Facts about a file that need no decoding of its content.
pub fn info(data: &[u8]) -> Info {
    let header = vwx_core::Header::parse(data);
    Info { size: data.len() as u64, version: header.map(|h| h.product()), ..Info::default() }
}

fn unpack(data: &[u8]) -> Result<vwx_core::container::Unpacked> {
    if data.is_empty() {
        return Err(Error::NotVwx("the file is empty".into()));
    }
    vwx_core::container::unpack(data)
}

fn info_of(unpacked: &vwx_core::container::Unpacked, size: usize, objects: usize) -> Info {
    let header = &unpacked.header;
    Info {
        version: Some(header.product()),
        application: Some(format!("Vectorworks {}.{}.{} build {}", header.saved.major, header.saved.minor, header.saved.patch, header.build)),
        size: size as u64,
        objects,
    }
}

fn document(raw: &Raw, info: Info) -> Document {
    let mut doc = Document { info, ..Document::default() };
    for (i, r) in raw.records.iter().enumerate() {
        match r.kind() {
            kind::END | kind::USER_DATA => continue,
            kind::LAYER => doc.layers.push(Layer {
                id: i as u64,
                name: raw::layer_name(r.bytes).unwrap_or_default(),
                elevation: raw::layer_elevation(r.bytes) / 1000.0,
                visible: !raw::layer_hidden(r.bytes),
                sheet: raw::layer_is_sheet(r.bytes),
                ..Layer::default()
            }),
            kind::CLASS => doc.classes.push(Class { id: r.ref_id() as u64, name: raw.names.get(&r.ref_id()).cloned().unwrap_or_default(), visible: !raw::class_hidden(r.bytes), fill: None }),
            _ => {}
        }
        doc.objects.push(Object {
            id: i as u64,
            kind: match kinds::name(r.kind()) {
                "?" => format!("unknown:{}", r.kind()),
                name => name.to_string(),
            },
            name: match r.kind() {
                kind::LAYER => raw::layer_name(r.bytes),
                _ => raw.name(i).map(str::to_string),
            },
            layer: raw.layer_of(i).filter(|&l| l != i).map(|l| l as u64),
            class: (r.class_id() != 0).then_some(r.class_id() as u64),
            parent: raw.parents[i].map(|p| p as u64),
            fields: Vec::new(),
        });
    }
    doc.info.objects = doc.objects.len();
    doc
}

/// Reads a document: every object with its type, name, layer, class and parent.
pub fn open(data: &[u8]) -> Result<Document> {
    let unpacked = unpack(data)?;
    let raw = Raw::new(&unpacked)?;
    Ok(document(&raw, info_of(&unpacked, data.len(), 0)))
}

/// Reads a document and makes the meshes a viewer draws.
pub fn read(data: &[u8]) -> Result<Scene> {
    let unpacked = unpack(data)?;
    let raw = Raw::new(&unpacked)?;
    let objects = raw.records.iter().filter(|r| !matches!(r.kind(), kind::END | kind::USER_DATA)).count();
    Ok(mesh::scene(&raw, info_of(&unpacked, data.len(), objects)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_file_is_not_vectorworks() {
        assert!(matches!(super::read(&[]), Err(super::Error::NotVwx(_))));
    }

    #[test]
    fn info_names_the_version_and_old_files_are_refused() {
        let mut d = vec![0, 0, 8, 0x58, 0, 0, 0, 0, 0, 0, 0, 0x94, 0, 0x20];
        d.extend(b"VW14.0 VW14.0 VectorWorks");
        d.resize(0x94 + 0x858 + 64, 0);
        assert!(super::sniff(&d));
        assert_eq!(super::info(&d).version.as_deref(), Some("Vectorworks 2009"));
        let e = super::read(&d).unwrap_err().to_string();
        assert!(e.contains("Vectorworks 2009"), "{e}");
    }
}
