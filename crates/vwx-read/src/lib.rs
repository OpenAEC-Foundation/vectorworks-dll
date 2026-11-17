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
//! **Status:** the container is not decoded yet; [`open`] and [`read`] answer
//! [`Error::NotYetDecoded`] for every Vectorworks file. See `docs/ARCHITECTURE.md`.

pub mod mesh;

pub use vwx_core::{Error, Result};
pub use vwx_model as model;
use vwx_model::{Document, Info, Scene};

/// True when `data` looks like a Vectorworks file. Until the signature is known this is false
/// for every file; callers go by the extension.
pub fn sniff(data: &[u8]) -> bool {
    let _ = data;
    false
}

/// Facts about a file that need no decoding of its content.
pub fn info(data: &[u8]) -> Info {
    Info { size: data.len() as u64, ..Info::default() }
}

/// Reads a document.
pub fn open(data: &[u8]) -> Result<Document> {
    if data.is_empty() {
        return Err(Error::NotVwx("the file is empty".into()));
    }
    Err(Error::NotYetDecoded("the Vectorworks file container".into()))
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
}
