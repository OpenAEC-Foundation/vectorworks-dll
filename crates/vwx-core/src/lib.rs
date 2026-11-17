//! Container layer of the VWX reader.
//!
//! Reads the container of a `.vwx` file and holds the tools the format is decoded with, all on
//! plain bytes so they also run in the browser:
//!
//! - [`header`]: the versions that made and saved a file, and where its parts are;
//! - [`container`]: the unpacked body, its records and their tree;
//! - [`snappy`]: the compression of the body;
//! - [`scan`]: compressed (zlib) streams, ASCII and UTF-16 strings, entropy per block;
//! - [`probe`]: one summary of a file (known containers, streams, strings, entropy);
//! - [`diff`]: the byte ranges in which two files differ, for files saved from Vectorworks
//!   that differ in one thing only (see `docs/ORACLE.md`);
//! - [`hex`]: hex dumps.

pub mod container;
pub mod diff;
mod error;
pub mod header;
pub mod hex;
pub mod probe;
pub mod scan;
pub mod snappy;

pub use error::{Error, Result};
pub use header::Header;
pub use probe::{Container, Probe};

/// Little-endian readers on a byte slice, `None` past its end.
pub mod le {
    pub fn u16(d: &[u8], at: usize) -> Option<u16> {
        d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn u32(d: &[u8], at: usize) -> Option<u32> {
        d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u64(d: &[u8], at: usize) -> Option<u64> {
        d.get(at..at + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    }
    pub fn f64(d: &[u8], at: usize) -> Option<f64> {
        u64(d, at).map(f64::from_bits)
    }
}

/// Big-endian readers on a byte slice, `None` past its end (Vectorworks began on the Mac).
pub mod be {
    pub fn u16(d: &[u8], at: usize) -> Option<u16> {
        d.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }
    pub fn u32(d: &[u8], at: usize) -> Option<u32> {
        d.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u64(d: &[u8], at: usize) -> Option<u64> {
        d.get(at..at + 8).map(|b| u64::from_be_bytes(b.try_into().unwrap()))
    }
    pub fn f64(d: &[u8], at: usize) -> Option<f64> {
        u64(d, at).map(f64::from_bits)
    }
}
