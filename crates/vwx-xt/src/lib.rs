//! Parasolid XT neutral binary: the solid bodies a Vectorworks file keeps for walls, slabs, CSG
//! solids and other solids.
//!
//! - [`parse`]: the nodes of the data (topology and geometry of a boundary representation);
//! - [`tessellate`]: the triangles of the faces of a body;
//! - [`find`]: XT data inside other bytes.
//!
//! The format is public: Annex E ("XT B-Rep data segment") of Siemens' "JT File Format Reference
//! Version 10.0". Nothing here comes from Parasolid itself.

pub mod curve;
pub mod geom;
pub mod nurbs;
pub mod parse;
pub mod schema;
pub mod surface;
pub mod tess;

pub use parse::{parse, Error, File, Node, Result, Value};
pub use tess::{tessellate, Mesh};

/// Node type codes used by name.
pub mod code {
    pub const BODY: u16 = 12;
    pub const SHELL: u16 = 13;
    pub const FACE: u16 = 14;
    pub const LOOP: u16 = 15;
    pub const EDGE: u16 = 16;
    pub const FIN: u16 = 17;
    pub const VERTEX: u16 = 18;
    pub const POINT: u16 = 29;
    pub const LINE: u16 = 30;
    pub const CIRCLE: u16 = 31;
    pub const ELLIPSE: u16 = 32;
    pub const PLANE: u16 = 50;
    pub const CYLINDER: u16 = 51;
    pub const CONE: u16 = 52;
    pub const SPHERE: u16 = 53;
    pub const TORUS: u16 = 54;
}

/// The offsets in `data` where XT neutral binary starts (`PS`, two zero bytes, a length and
/// `: TRANSMIT FILE`).
pub fn find(data: &[u8]) -> Vec<usize> {
    const FLAG: &[u8] = b"PS\0\0";
    const TEXT: &[u8] = b": TRANSMIT FILE";
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(k) = data[at..].windows(FLAG.len()).position(|w| w == FLAG) {
        let start = at + k;
        if data.get(start + 6..start + 6 + TEXT.len()) == Some(TEXT) {
            out.push(start);
        }
        at = start + FLAG.len();
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_xt_data_between_other_bytes() {
        let mut d = b"xxPS\0\0junk".to_vec();
        let at = d.len();
        d.extend(b"PS\0\0\0\x33: TRANSMIT FILE created by modeller");
        assert_eq!(super::find(&d), [at]);
    }
}
