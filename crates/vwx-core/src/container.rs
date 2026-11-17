//! The container: header, document header, compressed body and the records in it.
//!
//! ```text
//! file      = header (0x94 bytes) · document header · block*
//! block     = u32le length · snappy block of that length
//! body      = the blocks, unpacked and joined (each block holds at most 1 MiB)
//! body      = record*
//! record    = u64le length (itself included) · record header to 0x50 · payload
//! ```
//!
//! Record header (little-endian; `docs/FORMAT.md` § Records):
//!
//! | Offset | | |
//! |---|---|---|
//! | 0x00 | `u64` | length of the record |
//! | 0x19 | `u8` | object type, as the scripting API numbers them (2 line, 24 extrude, 68 wall, ...) |
//! | 0x1a | `u32` | reference number of the object (0: none) |
//! | 0x20 | `u32` | reference number of its class |
//! | 0x26 | `u64` | address of the parent object when the file was saved |
//! | 0x38 | 16 bytes | UUID of the object |
//! | 0x50 | | payload, by object type |
//!
//! The records are the object tree in depth-first order. No record states its own address, so
//! the tree follows from the parent addresses: a record whose parent address is new is a child
//! of the record before it; one whose parent address is known goes back to that parent.

use crate::{le, snappy, Error, Header, Result};

/// Length of the record header; the payload follows.
pub const RECORD_HEADER: usize = 0x50;

/// A file taken apart: its header, the document header and the unpacked body.
#[derive(Debug, Clone)]
pub struct Unpacked {
    pub header: Header,
    /// The document header: settings of the document (units and more), uncompressed.
    pub document: Vec<u8>,
    /// The records, unpacked.
    pub body: Vec<u8>,
    /// Number of compressed blocks the body came in.
    pub blocks: usize,
}

/// Takes a file apart. Reads files saved by Vectorworks 2019 and later (the oldest checked);
/// older headers are recognised and refused.
pub fn unpack(data: &[u8]) -> Result<Unpacked> {
    let header = Header::parse(data).ok_or_else(|| Error::NotVwx("no Vectorworks header".into()))?;
    let start = header.body_at();
    let document = data.get(header.len..start).ok_or_else(|| Error::Corrupt("the document header runs past the end of the file".into()))?;
    let mut body = Vec::new();
    let mut at = start;
    let mut blocks = 0;
    while at < data.len() {
        let len = le::u32(data, at).ok_or_else(|| Error::Corrupt("a block length is cut off".into()))? as usize;
        let block = data.get(at + 4..at + 4 + len).ok_or_else(|| Error::Corrupt(format!("block {blocks} runs past the end of the file")))?;
        let (bytes, used) = snappy::decompress(block).map_err(|e| match (blocks, &e) {
            // an older, not compressed body starts differently: say which version it is
            (0, _) => Error::NotYetDecoded(format!("the file body of {} ({e})", header.product())),
            _ => e,
        })?;
        if used != len {
            return Err(Error::Corrupt(format!("block {blocks} has {} bytes after its data", len - used)));
        }
        body.extend_from_slice(&bytes);
        at += 4 + len;
        blocks += 1;
    }
    Ok(Unpacked { header, document: document.to_vec(), body, blocks })
}

/// One record of the body.
#[derive(Debug, Clone, Copy)]
pub struct Record<'a> {
    /// Offset of the record in the body.
    pub offset: usize,
    /// The whole record, header included.
    pub bytes: &'a [u8],
}

impl<'a> Record<'a> {
    /// The object type (scripting API numbering); 0 ends the children of a container.
    pub fn kind(&self) -> u8 {
        self.bytes[0x19]
    }

    /// The reference number of the object: its index in the document's reference table and
    /// the key of its name in the name list; 0 for objects without one.
    pub fn ref_id(&self) -> u32 {
        le::u32(self.bytes, 0x1a).unwrap_or(0)
    }

    /// The reference number of the class the object is in.
    pub fn class_id(&self) -> u32 {
        le::u32(self.bytes, 0x20).unwrap_or(0)
    }

    /// Address of the parent object when the file was saved; only equality means something.
    pub fn parent_address(&self) -> u64 {
        le::u64(self.bytes, 0x26).unwrap_or(0)
    }

    pub fn uuid(&self) -> [u8; 16] {
        self.bytes[0x38..0x48].try_into().unwrap()
    }

    pub fn payload(&self) -> &'a [u8] {
        &self.bytes[RECORD_HEADER..]
    }
}

/// The records of a body, in file order.
pub fn records(body: &[u8]) -> Result<Vec<Record<'_>>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < body.len() {
        let len = le::u64(body, at).ok_or_else(|| Error::Corrupt("a record length is cut off".into()))? as usize;
        if len < RECORD_HEADER || len > body.len() - at {
            return Err(Error::Corrupt(format!("record at {at:#x} has length {len:#x}")));
        }
        out.push(Record { offset: at, bytes: &body[at..at + len] });
        at += len;
    }
    Ok(out)
}

/// The tree of `records`: for each record the index of its parent, `None` for the records of
/// the document itself.
pub fn parents(records: &[Record<'_>]) -> Vec<Option<usize>> {
    // the open chain of parents: (address the children name, index of that parent)
    let mut stack: Vec<(u64, Option<usize>)> = Vec::new();
    let mut out = Vec::with_capacity(records.len());
    for (i, r) in records.iter().enumerate() {
        let address = r.parent_address();
        match stack.iter().rposition(|(a, _)| *a == address) {
            Some(depth) => stack.truncate(depth + 1),
            // a new address: the record before this one is the parent (the first record names
            // the document)
            None => stack.push((address, i.checked_sub(1))),
        }
        out.push(stack.last().and_then(|(_, parent)| *parent));
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A record of `kind` under the parent at `parent`, with `payload`.
    pub(crate) fn record(kind: u8, parent: u64, payload: &[u8]) -> Vec<u8> {
        let mut r = vec![0u8; RECORD_HEADER];
        r[0..8].copy_from_slice(&((RECORD_HEADER + payload.len()) as u64).to_le_bytes());
        r[0x19] = kind;
        r[0x26..0x2e].copy_from_slice(&parent.to_le_bytes());
        r.extend_from_slice(payload);
        r
    }

    /// A file as Vectorworks 2027 writes it, from an unpacked `body`, in blocks of `block` bytes
    /// (stored as snappy literals).
    pub(crate) fn file(body: &[u8], block: usize) -> Vec<u8> {
        let document = [7u8; 40];
        let mut out = crate::header::tests::header_2027(document.len() as u32, 0);
        out.extend(document);
        for chunk in body.chunks(block.max(1)) {
            let mut z = Vec::new();
            let mut n = chunk.len();
            while n >= 0x80 {
                z.push(n as u8 | 0x80);
                n >>= 7;
            }
            z.push(n as u8);
            for lit in chunk.chunks(60) {
                z.push(((lit.len() - 1) as u8) << 2);
                z.extend(lit);
            }
            out.extend((z.len() as u32).to_le_bytes());
            out.extend(z);
        }
        out
    }

    #[test]
    fn unpacks_blocks_and_walks_records() {
        let mut body = record(31, 1, b"layer");
        body.extend(record(2, 2, &[1; 60]));
        body.extend(record(0, 2, &[]));
        let data = file(&body, 100);
        let u = unpack(&data).unwrap();
        assert_eq!(u.body, body);
        assert_eq!(u.blocks, 4);
        assert_eq!(u.document, [7u8; 40]);
        let recs = records(&u.body).unwrap();
        assert_eq!(recs.iter().map(|r| r.kind()).collect::<Vec<_>>(), [31, 2, 0]);
        assert_eq!(recs[0].payload(), b"layer");
        assert_eq!(recs[1].offset, RECORD_HEADER + 5);
    }

    #[test]
    fn tree_follows_from_parent_addresses() {
        // document(1): layer; layer(2): group, line; group(3): rect, end; layer: end
        let kinds = [(31, 1), (11, 2), (3, 3), (76, 4), (0, 3), (2, 2), (0, 2), (94, 1)];
        let body: Vec<u8> = kinds.iter().flat_map(|&(k, p)| record(k, p, &[])).collect();
        let recs = records(&body).unwrap();
        assert_eq!(parents(&recs), [None, Some(0), Some(1), Some(2), Some(1), Some(0), Some(0), None]);
    }

    #[test]
    fn damaged_files_are_errors() {
        assert!(matches!(unpack(b"not a vectorworks file at all"), Err(Error::NotVwx(_))));
        let mut data = file(&record(2, 1, &[0; 60]), 1000);
        data.truncate(data.len() - 3);
        assert!(matches!(unpack(&data), Err(Error::Corrupt(_))));
        assert!(records(&[5, 0, 0, 0, 0, 0, 0, 0]).is_err());
    }
}
