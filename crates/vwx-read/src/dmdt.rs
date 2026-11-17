//! Typed data in user data records (type 76) whose payload starts with `TDMD`: a key of four
//! letters and numbered fields, each an array of one type. About half the records of a file are
//! of this kind; they hold what belongs to the object they follow (and, at document level,
//! tables such as the colour palette).
//!
//! ```text
//! "TDMD" · u16 version (5) · key (4 letters, reversed) · u16 field count · u32 size of the rest
//! field count × (u16 field number · u32 offset from the end of this head)
//! per field: u16 type · u32 count · the values
//! ```
//!
//! Types seen: 1 (one byte each), 6 (`f64`), 8 (four bytes each: `f32` or `u32`).

use vwx_core::container::{Record, RECORD_HEADER};
use vwx_core::le;

/// One field: its type number, the count of values and their bytes.
#[derive(Debug, Clone, Copy)]
pub struct Field<'a> {
    pub kind: u16,
    pub count: usize,
    pub bytes: &'a [u8],
}

impl Field<'_> {
    /// The values as `f32` (type 8).
    pub fn f32s(&self) -> Vec<f32> {
        self.bytes.as_chunks::<4>().0.iter().take(self.count).map(|c| f32::from_le_bytes(*c)).collect()
    }
}

/// A `TDMD` record taken apart.
#[derive(Debug, Clone)]
pub struct Data<'a> {
    /// The key as written in the file read backwards, e.g. `DCDN`.
    pub key: [u8; 4],
    fields: Vec<(u16, Field<'a>)>,
}

impl<'a> Data<'a> {
    /// The typed data of a user data record; `None` when it is not `TDMD` data.
    pub fn parse(record: &Record<'a>) -> Option<Data<'a>> {
        let p = record.bytes.get(RECORD_HEADER..)?;
        if !p.starts_with(b"TDMD") || p.len() < 16 {
            return None;
        }
        let key = [p[9], p[8], p[7], p[6]];
        let (count, size) = (le::u16(p, 10)? as usize, le::u32(p, 12)? as usize);
        let body = &p[16..(16 + size).min(p.len())];
        let entry = |k: usize| Some((le::u16(body, 6 * k)?, le::u32(body, 6 * k + 2)? as usize));
        let mut fields = Vec::with_capacity(count.min(256));
        for k in 0..count {
            let (number, at) = entry(k)?;
            let end = if k + 1 < count { entry(k + 1)?.1 } else { body.len() };
            let raw = body.get(at..end)?;
            let (kind, n) = (le::u16(raw, 0)?, le::u32(raw, 2)? as usize);
            fields.push((number, Field { kind, count: n, bytes: &raw[6..] }));
        }
        Some(Data { key, fields })
    }

    pub fn field(&self, number: u16) -> Option<Field<'a>> {
        self.fields.iter().find(|(n, _)| *n == number).map(|(_, f)| *f)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A `TDMD` user data record with the given fields (number, type, count, bytes).
    pub(crate) fn record(key: &[u8; 4], fields: &[(u16, u16, u32, Vec<u8>)]) -> Vec<u8> {
        let mut table = Vec::new();
        let mut data = Vec::new();
        for (number, kind, count, bytes) in fields {
            table.extend(number.to_le_bytes());
            table.extend(((fields.len() * 6 + data.len()) as u32).to_le_bytes());
            data.extend(kind.to_le_bytes());
            data.extend(count.to_le_bytes());
            data.extend(bytes);
        }
        let mut r = vec![0u8; RECORD_HEADER];
        r[0x19] = 76;
        r.extend(b"TDMD");
        r.extend(5u16.to_le_bytes());
        r.extend(key.iter().rev());
        r.extend((fields.len() as u16).to_le_bytes());
        r.extend(((table.len() + data.len()) as u32).to_le_bytes());
        r.extend(table);
        r.extend(data);
        let n = r.len() as u64;
        r[0..8].copy_from_slice(&n.to_le_bytes());
        r
    }

    #[test]
    fn reads_fields_by_number() {
        let floats: Vec<u8> = [0.5f32, 0.25].iter().flat_map(|f| f.to_le_bytes()).collect();
        let bytes = record(b"DCDN", &[(1, 8, 1, vec![2, 0, 0, 0]), (2, 8, 2, floats)]);
        let data = Data::parse(&Record { offset: 0, bytes: &bytes }).unwrap();
        assert_eq!(&data.key, b"DCDN");
        assert_eq!(data.field(2).unwrap().f32s(), [0.5, 0.25]);
        assert_eq!(data.field(1).unwrap().count, 1);
        assert!(data.field(9).is_none());
    }
}
