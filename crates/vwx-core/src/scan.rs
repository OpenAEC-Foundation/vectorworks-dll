//! Format-blind scans of a file: compressed streams, strings and entropy.

use flate2::{Decompress, FlushDecompress, Status};
use serde::Serialize;

/// The most one stream may inflate to (a guard against garbage that happens to inflate).
const MAX_INFLATED: usize = 512 << 20;

/// A zlib stream found in a file.
#[derive(Debug, Clone)]
pub struct Stream {
    /// Offset of the zlib header in the file.
    pub offset: usize,
    /// Compressed length, header and checksum included.
    pub compressed: usize,
    pub data: Vec<u8>,
}

/// True when `b0 b1` is a valid zlib header (deflate, 32 KiB window or smaller, no dictionary).
fn zlib_header(b0: u8, b1: u8) -> bool {
    b0 & 0x0f == 8 && b0 >> 4 <= 7 && b1 & 0x20 == 0 && (u16::from(b0) << 8 | u16::from(b1)) % 31 == 0
}

/// Inflates the zlib stream at the start of `data`: the bytes and the compressed length, when it
/// is a complete stream (checksum included).
pub fn inflate_at(data: &[u8]) -> Option<(Vec<u8>, usize)> {
    if data.len() < 6 || !zlib_header(data[0], data[1]) {
        return None;
    }
    let mut z = Decompress::new(true);
    // small at first: most candidates in a file are not streams and fail in the first bytes
    let mut out = Vec::with_capacity(16 << 10);
    loop {
        if out.len() == out.capacity() {
            if out.len() >= MAX_INFLATED {
                return None;
            }
            out.reserve(out.len());
        }
        let consumed = z.total_in() as usize;
        match z.decompress_vec(&data[consumed..], &mut out, FlushDecompress::None) {
            Ok(Status::StreamEnd) => return Some((out, z.total_in() as usize)),
            Ok(Status::Ok) => {}
            // no progress: input ends before the stream does
            Ok(Status::BufError) if out.len() < out.capacity() => return None,
            Ok(Status::BufError) => {}
            Err(_) => return None,
        }
    }
}

/// Every complete zlib stream in `data` that inflates to at least `min_len` bytes, in file
/// order; a stream found is skipped as a whole.
pub fn zlib_streams(data: &[u8], min_len: usize) -> Vec<Stream> {
    let mut found = Vec::new();
    let mut at = 0;
    while at + 6 <= data.len() {
        if zlib_header(data[at], data[at + 1]) {
            if let Some((bytes, used)) = inflate_at(&data[at..]) {
                if bytes.len() >= min_len {
                    found.push(Stream { offset: at, compressed: used, data: bytes });
                    at += used;
                    continue;
                }
            }
        }
        at += 1;
    }
    found
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Encoding {
    Ascii,
    Utf16le,
    Utf16be,
}

/// A run of printable text.
#[derive(Debug, Clone, Serialize)]
pub struct Text {
    pub offset: usize,
    pub encoding: Encoding,
    pub text: String,
}

fn printable(b: u8) -> bool {
    (0x20..0x7f).contains(&b) || b == b'\t'
}

/// Printable runs of at least `min_len` characters: ASCII (Latin-1 letters included) and
/// UTF-16 (little and big endian, ASCII range), in file order.
pub fn strings(data: &[u8], min_len: usize) -> Vec<Text> {
    let mut out = Vec::new();
    let latin = |b: u8| printable(b) || b >= 0xc0;
    // single byte
    let mut start = None;
    for (i, &b) in data.iter().chain(std::iter::once(&0)).enumerate() {
        match (latin(b), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if i - s >= min_len {
                    let text = data[s..i].iter().map(|&c| c as char).collect();
                    out.push(Text { offset: s, encoding: Encoding::Ascii, text });
                }
                start = None;
            }
            _ => {}
        }
    }
    // two bytes: on even and on odd offsets
    for (encoding, lo) in [(Encoding::Utf16le, 0usize), (Encoding::Utf16be, 1usize)] {
        for phase in 0..2 {
            let mut run: Vec<char> = Vec::new();
            let mut s = phase;
            let mut i = phase;
            while i + 1 < data.len() + 2 {
                let pair = data.get(i..i + 2);
                let ch = pair.and_then(|p| (p[1 - lo] == 0 && printable(p[lo])).then_some(p[lo] as char));
                match ch {
                    Some(c) => {
                        if run.is_empty() {
                            s = i;
                        }
                        run.push(c);
                    }
                    None => {
                        if run.len() >= min_len {
                            out.push(Text { offset: s, encoding, text: run.iter().collect() });
                        }
                        run.clear();
                    }
                }
                i += 2;
            }
        }
    }
    out.sort_by_key(|t| t.offset);
    out
}

/// Shannon entropy in bits per byte (0..8) of each block of `block` bytes: compressed or
/// encrypted data is near 8, records and tables lower.
pub fn entropy(data: &[u8], block: usize) -> Vec<f32> {
    data.chunks(block.max(1))
        .map(|c| {
            let mut n = [0u32; 256];
            for &b in c {
                n[b as usize] += 1;
            }
            let len = c.len() as f64;
            -n.iter().filter(|&&k| k > 0).map(|&k| k as f64 / len).map(|p| p * p.log2()).sum::<f64>() as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn finds_streams_between_other_bytes() {
        let a: Vec<u8> = (0..2000u32).map(|i| (i % 7) as u8).collect();
        let b = b"hello hello hello hello hello world".repeat(20);
        let mut file = b"HEAD\x78\x9c junk".to_vec();
        let at_a = file.len();
        file.extend(zlib(&a));
        file.extend(b"\x00\x01\x02");
        let at_b = file.len();
        file.extend(zlib(&b));
        let found = zlib_streams(&file, 16);
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].offset, &found[0].data), (at_a, &a));
        assert_eq!((found[1].offset, &found[1].data), (at_b, &b));
        assert_eq!(found[1].offset + found[1].compressed, file.len());
    }

    #[test]
    fn finds_ascii_and_utf16_strings() {
        let mut d = b"\x01\x02Layer-1\x00\x00".to_vec();
        d.extend("Design Layer".encode_utf16().flat_map(u16::to_le_bytes));
        d.extend([0, 0, 7]);
        d.extend("Klasse".encode_utf16().flat_map(u16::to_be_bytes));
        let s = strings(&d, 5);
        let found = |e: Encoding, t: &str| s.iter().any(|x| x.encoding == e && x.text == t);
        assert!(found(Encoding::Ascii, "Layer-1"), "{s:?}");
        assert!(found(Encoding::Utf16le, "Design Layer"), "{s:?}");
        assert!(found(Encoding::Utf16be, "Klasse"), "{s:?}");
    }

    #[test]
    fn entropy_of_constant_and_uniform_blocks() {
        let e = entropy(&[[0u8; 256].to_vec(), (0..=255).collect()].concat(), 256);
        assert_eq!(e[0], 0.0);
        assert!((e[1] - 8.0).abs() < 1e-6);
    }
}
