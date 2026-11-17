//! Container layer of the VWX reader.
//!
//! The container of a `.vwx` file is not decoded yet. Until it is, this crate holds the tools
//! that decode it, all on plain bytes so they also run in the browser:
//!
//! - [`scan`]: compressed (zlib) streams, ASCII and UTF-16 strings, entropy per block;
//! - [`probe`]: one summary of a file (known containers, streams, strings, entropy);
//! - [`diff`]: the byte ranges in which two files differ, for files saved from Vectorworks
//!   that differ in one thing only (see `docs/ORACLE.md`);
//! - [`hex`]: hex dumps.

mod error;
pub mod diff;
pub mod hex;
pub mod probe;
pub mod scan;

pub use error::{Error, Result};
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
