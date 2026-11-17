//! The file header: which version made and saved a file, and where its parts are.
//!
//! Layout (big-endian, `docs/FORMAT.md` § Header), checked on 787 files saved by Vectorworks
//! 2023 and 2027:
//!
//! | Offset | | |
//! |---|---|---|
//! | 0 | `u32` | length of the document header that follows this header |
//! | 8 | `u32` | length of this header (`0x94`) |
//! | 12 | `u16` | `0x0020` |
//! | 14 | text | `VW<created> VW<saved>`, NUL-terminated, e.g. `VW24.0.0 VW32.0.0` |
//! | 0x50 | `u32` | build number of the saving application |
//! | 0x54 | `u32` | size of the file |
//!
//! The first version is the one the document was first made with, the second the one that saved
//! it. Major version 13 is Vectorworks 2008; from then on the year is the major version + 1995.
//!
//! Files of Vectorworks 12.5 to 2010 carry `VW12.5` style text at 14 and files of Vectorworks 9
//! and 10 the text `VectorWorks 10` (public PRONOM signatures fmt/450, 451, 686, 1139, 1140; not
//! checked on samples): [`Header::parse`] recognises them, the reader does not read them.

use serde::Serialize;

/// Where the version text starts.
const VERSION_AT: usize = 14;

/// A version as stored: `VW32.0.0` is `{32, 0, 0}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// The marketing name: `Vectorworks 2027`, `Vectorworks 12.5`.
    pub fn product(&self) -> String {
        match self.major {
            13.. => format!("Vectorworks {}", self.major + 1995),
            _ if self.minor > 0 => format!("Vectorworks {}.{}", self.major, self.minor),
            _ => format!("Vectorworks {}", self.major),
        }
    }

    /// `VW32.0.0`, `VW12.5` or `12` at the start of `d`: the version and the length of the text.
    fn parse(d: &[u8]) -> Option<(Version, usize)> {
        let mut at = if d.starts_with(b"VW") { 2 } else { 0 };
        let mut parts = [0u32; 3];
        for (k, part) in parts.iter_mut().enumerate() {
            if k > 0 {
                match d.get(at) {
                    Some(b'.') => at += 1,
                    _ => break,
                }
            }
            let n = d[at..].iter().take(4).take_while(|b| b.is_ascii_digit()).count();
            if n == 0 {
                return None;
            }
            *part = std::str::from_utf8(&d[at..at + n]).ok()?.parse().ok()?;
            at += n;
        }
        Some((Version { major: parts[0], minor: parts[1], patch: parts[2] }, at))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Header {
    /// The version the document was first made with.
    pub created: Version,
    /// The version that saved the file (the same as `created` in old files, which name one).
    pub saved: Version,
    /// Length of this header.
    pub len: usize,
    /// Length of the document header after it; the compressed body follows that.
    pub document_len: usize,
    /// Build number of the saving application (0 when the header is too short to hold it).
    pub build: u32,
    /// Size of the file as the header states it.
    pub file_size: u32,
}

impl Header {
    /// The header of `data`, when it starts like a Vectorworks file.
    pub fn parse(data: &[u8]) -> Option<Header> {
        let document_len = crate::be::u32(data, 0)? as usize;
        let len = crate::be::u32(data, 8)? as usize;
        let rest = data.get(VERSION_AT..)?;
        let (created, saved) = if rest.starts_with(b"VW") {
            let (created, n) = Version::parse(rest)?;
            // the saving version follows after a space (2011 and later; before that the same
            // text is repeated)
            let saved = rest.get(n..).and_then(|r| r.strip_prefix(b" ")).and_then(Version::parse).map_or(created, |v| v.0);
            (created, saved)
        } else {
            let (v, _) = Version::parse(rest.strip_prefix(b"VectorWorks ")?)?;
            (v, v)
        };
        Some(Header {
            created,
            saved,
            len,
            document_len,
            build: crate::be::u32(data, 0x50).unwrap_or(0),
            file_size: crate::be::u32(data, 0x54).unwrap_or(0),
        })
    }

    /// The marketing name of the version that saved the file.
    pub fn product(&self) -> String {
        self.saved.product()
    }

    /// Where the compressed body starts.
    pub fn body_at(&self) -> usize {
        self.len + self.document_len
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A header as Vectorworks 2027 writes it, for a file with a document header of `doc` bytes.
    pub(crate) fn header_2027(doc: u32, file_size: u32) -> Vec<u8> {
        let mut d = vec![0u8; 0x94];
        d[0..4].copy_from_slice(&doc.to_be_bytes());
        d[8..12].copy_from_slice(&0x94u32.to_be_bytes());
        d[12..14].copy_from_slice(&[0, 0x20]);
        d[14..32].copy_from_slice(b"VW24.0.0 VW32.0.0\0");
        d[0x50..0x54].copy_from_slice(&882075u32.to_be_bytes());
        d[0x54..0x58].copy_from_slice(&file_size.to_be_bytes());
        d
    }

    #[test]
    fn reads_both_versions_and_the_lengths() {
        let h = Header::parse(&header_2027(0x9e0, 80960)).unwrap();
        assert_eq!(h.created, Version { major: 24, minor: 0, patch: 0 });
        assert_eq!(h.saved, Version { major: 32, minor: 0, patch: 0 });
        assert_eq!((h.len, h.document_len, h.body_at()), (0x94, 0x9e0, 0xa74));
        assert_eq!((h.build, h.file_size), (882075, 80960));
        assert_eq!(h.product(), "Vectorworks 2027");
        assert_eq!(h.created.product(), "Vectorworks 2019");
    }

    #[test]
    fn reads_the_old_texts() {
        let mut d = vec![0, 0, 8, 0x58, 0, 0, 0, 0, 0, 0, 0, 0x94, 0, 0x20];
        d.extend(b"VW12.5 VW12.5  VectorWorks");
        let h = Header::parse(&d).unwrap();
        assert_eq!(h.product(), "Vectorworks 12.5");
        assert_eq!(h.saved, h.created);
        let mut old = vec![0u8; 14];
        old.extend(b"VectorWorks 10\0");
        assert_eq!(Header::parse(&old).unwrap().product(), "Vectorworks 10");
    }

    #[test]
    fn other_files_have_no_header() {
        assert_eq!(Header::parse(b"PK\x03\x04 a zip file, not vectorworks"), None);
        assert_eq!(Header::parse(&[0u8; 16]), None);
    }
}
