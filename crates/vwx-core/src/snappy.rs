//! Snappy block decompression (the raw format, without framing).
//!
//! The body of a `.vwx` file is Snappy compressed (see `docs/FORMAT.md`). The format is public
//! (Google's `format_description.txt`): a varint with the uncompressed length, then elements, each
//! a literal run or a copy of earlier output.

use crate::{Error, Result};

/// The uncompressed length a block announces and the number of bytes of that varint.
pub fn announced_len(data: &[u8]) -> Option<(usize, usize)> {
    let mut len = 0usize;
    for (i, &b) in data.iter().take(5).enumerate() {
        len |= ((b & 0x7f) as usize) << (7 * i);
        if b & 0x80 == 0 {
            return Some((len, i + 1));
        }
    }
    None
}

/// Decompresses the Snappy block at the start of `data`: the bytes and how much of `data` the
/// block took.
pub fn decompress(data: &[u8]) -> Result<(Vec<u8>, usize)> {
    let bad = |what: &str| Error::Corrupt(format!("snappy: {what}"));
    let (len, mut at) = announced_len(data).ok_or_else(|| bad("no length"))?;
    if len > 1 << 31 {
        return Err(bad("length too large"));
    }
    let mut out: Vec<u8> = Vec::with_capacity(len);
    let take = |at: usize, n: usize| data.get(at..at + n).ok_or_else(|| bad("input ends inside an element"));
    while out.len() < len {
        let tag = *data.get(at).ok_or_else(|| bad("input ends before the output is complete"))?;
        at += 1;
        let (copy_len, offset) = match tag & 3 {
            0 => {
                // literal: length - 1 in the upper six bits, or in the next 1..4 bytes
                let mut n = (tag >> 2) as usize;
                if n >= 60 {
                    let extra = n - 59;
                    let b = take(at, extra)?;
                    n = b.iter().rev().fold(0usize, |v, &x| v << 8 | x as usize);
                    at += extra;
                }
                let n = n + 1;
                out.extend_from_slice(take(at, n)?);
                at += n;
                continue;
            }
            1 => {
                let b = take(at, 1)?;
                at += 1;
                (4 + ((tag >> 2) & 7) as usize, ((tag >> 5) as usize) << 8 | b[0] as usize)
            }
            2 => {
                let b = take(at, 2)?;
                at += 2;
                ((tag >> 2) as usize + 1, u16::from_le_bytes([b[0], b[1]]) as usize)
            }
            _ => {
                let b = take(at, 4)?;
                at += 4;
                ((tag >> 2) as usize + 1, u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
            }
        };
        if offset == 0 || offset > out.len() {
            return Err(bad("copy from before the start"));
        }
        // byte by byte: a copy may overlap its own output (a run)
        let from = out.len() - offset;
        for k in 0..copy_len {
            out.push(out[from + k]);
        }
    }
    if out.len() != len {
        return Err(bad("output longer than announced"));
    }
    Ok((out, at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_and_overlapping_copy() {
        // 10 bytes: literal "ab" then a copy of length 8 from offset 2 (a run of "ab")
        let block = [10, 0x04, b'a', b'b', 0x11, 0x02];
        assert_eq!(decompress(&block).unwrap(), (b"ababababab".to_vec(), 6));
    }

    #[test]
    fn copy_with_two_byte_offset_and_long_literal() {
        // 70 bytes: a 61-byte literal (length in one extra byte), then copy 9 from offset 61
        let mut block = vec![70, 60 << 2, 60];
        let text: Vec<u8> = (0..61).map(|i| b'A' + i % 26).collect();
        block.extend(&text);
        block.extend([(9 - 1) << 2 | 2, 61, 0]);
        let (out, used) = decompress(&block).unwrap();
        assert_eq!(used, block.len());
        assert_eq!(&out[..61], &text[..]);
        assert_eq!(&out[61..], &text[..9]);
    }

    #[test]
    fn damaged_blocks_are_errors() {
        assert!(decompress(&[5, 0x04, b'a']).is_err());
        assert!(decompress(&[5, 0x11, 0x09]).is_err());
        assert!(decompress(&[]).is_err());
    }
}
