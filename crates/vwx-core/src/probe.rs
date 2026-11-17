//! One summary of a file, the first look at an unknown sample.

use serde::Serialize;

use crate::scan::{self, Encoding};

/// Containers the probe recognises by their signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Container {
    /// Microsoft compound file (OLE2).
    CompoundFile,
    Zip,
    Gzip,
    Sqlite,
    /// The whole file is one zlib stream.
    Zlib,
    Unknown,
}

impl Container {
    pub fn of(data: &[u8]) -> Container {
        match data {
            [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1, ..] => Container::CompoundFile,
            [b'P', b'K', 3, 4, ..] => Container::Zip,
            [0x1f, 0x8b, ..] => Container::Gzip,
            _ if data.starts_with(b"SQLite format 3\0") => Container::Sqlite,
            _ if scan::inflate_at(data).is_some_and(|(_, used)| used == data.len()) => Container::Zlib,
            _ => Container::Unknown,
        }
    }
}

/// A compressed stream, without its bytes.
#[derive(Debug, Clone, Serialize)]
pub struct StreamInfo {
    pub offset: usize,
    pub compressed: usize,
    pub inflated: usize,
    /// The first bytes after inflating, in hex.
    pub head: String,
}

/// What a first look at a file shows.
#[derive(Debug, Clone, Serialize)]
pub struct Probe {
    pub size: usize,
    /// The first 64 bytes, in hex.
    pub head: String,
    /// The first 64 bytes as text (`.` for bytes that do not print).
    pub head_text: String,
    pub container: Container,
    /// The Vectorworks header, when the file starts with one.
    pub header: Option<crate::Header>,
    /// Zlib streams of 16 bytes or more.
    pub streams: Vec<StreamInfo>,
    /// Bytes covered by the streams (compressed).
    pub stream_bytes: usize,
    /// Strings of 6 characters or more, by encoding.
    pub strings_ascii: usize,
    pub strings_utf16le: usize,
    pub strings_utf16be: usize,
    /// The first strings found (up to 40).
    pub sample_strings: Vec<scan::Text>,
    /// Entropy (bits per byte) of each 4 KiB block, rounded to one decimal.
    pub entropy: Vec<f32>,
}

impl Probe {
    pub fn of(data: &[u8]) -> Probe {
        let streams = scan::zlib_streams(data, 16);
        let strings = scan::strings(data, 6);
        let count = |e: Encoding| strings.iter().filter(|t| t.encoding == e).count();
        Probe {
            size: data.len(),
            head: crate::hex::bytes(&data[..data.len().min(64)]),
            head_text: crate::hex::text(&data[..data.len().min(64)]),
            container: Container::of(data),
            header: crate::Header::parse(data),
            stream_bytes: streams.iter().map(|s| s.compressed).sum(),
            streams: streams
                .iter()
                .map(|s| StreamInfo {
                    offset: s.offset,
                    compressed: s.compressed,
                    inflated: s.data.len(),
                    head: crate::hex::bytes(&s.data[..s.data.len().min(32)]),
                })
                .collect(),
            strings_ascii: count(Encoding::Ascii),
            strings_utf16le: count(Encoding::Utf16le),
            strings_utf16be: count(Encoding::Utf16be),
            sample_strings: strings.into_iter().take(40).collect(),
            entropy: scan::entropy(data, 4096).into_iter().map(|e| (e * 10.0).round() / 10.0).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_containers() {
        assert_eq!(Container::of(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1, 0]), Container::CompoundFile);
        assert_eq!(Container::of(b"PK\x03\x04rest"), Container::Zip);
        assert_eq!(Container::of(b"something else"), Container::Unknown);
    }
}
