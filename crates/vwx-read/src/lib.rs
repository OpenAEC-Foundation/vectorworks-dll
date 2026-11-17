//! Reads a Vectorworks `.vwx` file into the neutral model ([`vwx_model::Document`]) and into
//! meshes for a viewer ([`vwx_model::Scene`]).
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
//! **Status:** the container, the records and their tree are read ([`open`] lists every object
//! with its type and parent); object geometry is not decoded yet, so [`read`] gives a scene
//! without meshes. See `docs/ARCHITECTURE.md`.

pub mod kinds;
pub mod mesh;

pub use vwx_core::{Error, Result};
pub use vwx_model as model;
use vwx_model::{Document, Info, Object, Scene};

/// True when `data` starts like a Vectorworks file (see [`vwx_core::header`]: known for
/// Vectorworks 9 to 2010 so far, so callers also go by the extension).
pub fn sniff(data: &[u8]) -> bool {
    vwx_core::Header::parse(data).is_some()
}

/// Facts about a file that need no decoding of its content.
pub fn info(data: &[u8]) -> Info {
    let header = vwx_core::Header::parse(data);
    Info { size: data.len() as u64, version: header.map(|h| h.product()), ..Info::default() }
}

/// Reads a document: every object with its type and its parent.
pub fn open(data: &[u8]) -> Result<Document> {
    if data.is_empty() {
        return Err(Error::NotVwx("the file is empty".into()));
    }
    let unpacked = vwx_core::container::unpack(data)?;
    let records = vwx_core::container::records(&unpacked.body)?;
    let parents = vwx_core::container::parents(&records);
    let objects: Vec<Object> = records
        .iter()
        .zip(&parents)
        .enumerate()
        // user data and list ends are part of their object, not objects themselves
        .filter(|(_, (r, _))| !matches!(r.kind(), 0 | 76))
        .map(|(i, (r, parent))| Object {
            id: i as u64,
            kind: match kinds::name(r.kind()) {
                "?" => format!("unknown:{}", r.kind()),
                name => name.to_string(),
            },
            parent: parent.map(|p| p as u64),
            ..Object::default()
        })
        .collect();
    let header = &unpacked.header;
    let info = Info {
        version: Some(header.product()),
        application: Some(format!("Vectorworks {}.{}.{} build {}", header.saved.major, header.saved.minor, header.saved.patch, header.build)),
        size: data.len() as u64,
        objects: objects.len(),
    };
    Ok(Document { info, objects, ..Document::default() })
}

/// Reads a document and makes the meshes a viewer draws.
pub fn read(data: &[u8]) -> Result<Scene> {
    open(data).map(|doc| mesh::scene(&doc))
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
