//! Records: the data attached to objects (`docs/FORMAT.md` § Records).
//!
//! A record format (type 47) names its fields; a record (type 48, a child of the object it is
//! attached to) holds a value for each of them.

use vwx_core::container::Record;
use vwx_core::le;

use crate::raw::{kind, utf16, Raw};

/// Most fields read of one format, and most characters of one text.
const MAX_FIELDS: usize = 4096;
const MAX_TEXT: usize = 1 << 16;

/// What a field holds, by the room its value takes in a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Integer,
    Boolean,
    /// An `f64` and two more bytes.
    Number,
    /// A length and a place in the record, where its UTF-16 text is. A choice from a list is
    /// kept this way too, the chosen text first.
    Text,
    /// Kinds whose values are not decoded.
    Other,
}

impl Kind {
    /// The kind of a field from its number in the format: 1 integer, 2 boolean, 4 text; the
    /// numbers 3, 7, 10, 11, 20, 21 and 22 take 10 bytes each and hold a number; 8, 9, 14, 18
    /// and 19 take 8 bytes as text does (seen on the corpus: lists to choose from). The others
    /// (23 to 30) take 1, 2 or 4 bytes.
    fn of(number: u16) -> Kind {
        match number {
            1 => Kind::Integer,
            2 => Kind::Boolean,
            3 | 7 | 10 | 11 | 20 | 21 | 22 => Kind::Number,
            4 | 8 | 9 | 14 | 18 | 19 => Kind::Text,
            _ => Kind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub kind: Kind,
    /// Where its value is in the fixed part of a record.
    pub offset: usize,
}

/// A value of a field.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Boolean(bool),
    Number(f64),
    Text(String),
}

/// The fields of a record format (type 47): a `u32` count at 0x50; from 0x60 a block of 136
/// bytes per field: `u16` kind, `u16` offset of its value in a record, 4 bytes, and its name
/// as UTF-16 ending in a zero (what follows the zero in its 128 bytes is not part of it).
pub fn format(r: &Record) -> Option<Vec<Field>> {
    let count = (le::u32(r.bytes, 0x50)? as usize).min(MAX_FIELDS);
    let mut fields = Vec::with_capacity(count);
    for k in 0..count {
        let at = 0x60 + 136 * k;
        let name = r.bytes.get(at + 8..at + 136)?;
        let end = name.as_chunks::<2>().0.iter().position(|c| *c == [0, 0]).map_or(name.len(), |n| 2 * n);
        fields.push(Field { name: utf16(&name[..end]), kind: Kind::of(le::u16(r.bytes, at)?), offset: le::u16(r.bytes, at + 2)? as usize });
    }
    Some(fields)
}

/// The reference number of the format of a record (type 48), at 0x50.
pub fn format_of(r: &Record) -> Option<u32> {
    le::u32(r.bytes, 0x50)
}

/// The values of a record (type 48) of a format with `fields`: after the format number a `u16`
/// count of bytes to pass over (none, or one per field), then the fixed part, each value at
/// the offset its field gives, then the texts. A text value is a `u32` length in bytes and a
/// `u32` place counted from the start of the fixed part. Fields whose value is not there or
/// not decoded are left out.
pub fn values(r: &Record, fields: &[Field]) -> Vec<(String, Value)> {
    let Some(body) = le::u16(r.bytes, 0x54).and_then(|skip| r.bytes.get(0x56 + skip as usize..)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for field in fields {
        let at = field.offset;
        let value = match field.kind {
            Kind::Integer => le::u32(body, at).map(|v| Value::Integer(i64::from(v as i32))),
            Kind::Boolean => le::u16(body, at).map(|v| Value::Boolean(v != 0)),
            Kind::Number => le::f64(body, at).filter(|v| v.is_finite() && body.len() >= at + 10).map(Value::Number),
            Kind::Text => (|| {
                let (len, place) = (le::u32(body, at)? as usize, le::u32(body, at + 4)? as usize);
                let text = utf16(body.get(place..place.checked_add(len.min(MAX_TEXT))?)?);
                // of a list to choose from, the chosen text comes first
                Some(Value::Text(text.split('\0').next().unwrap_or_default().to_string()))
            })(),
            Kind::Other => None,
        };
        if let Some(value) = value {
            out.push((field.name.clone(), value));
        }
    }
    out
}

/// The records attached to object `i`: per record the name of its format and its values.
pub fn of_object(raw: &Raw, i: usize) -> Vec<(String, Vec<(String, Value)>)> {
    let mut out = Vec::new();
    for &c in &raw.children[i] {
        let r = &raw.records[c];
        if r.kind() != kind::RECORD {
            continue;
        }
        let Some(number) = format_of(r) else { continue };
        let Some(fields) = raw.by_ref.get(&number).map(|&f| &raw.records[f]).filter(|f| f.kind() == kind::RECORD_FORMAT).and_then(format) else {
            continue;
        };
        let name = raw.names.get(&number).cloned().unwrap_or_else(|| format!("Record {number}"));
        out.push((name, values(r, &fields)));
    }
    out
}

/// The values of a record as pairs to show. A record of name and value fields (`prm0_name`,
/// `prm0_value`, .., as the IFC data of an object is kept) gives its values under those names.
pub fn pairs(values: &[(String, Value)]) -> Vec<(String, Value)> {
    let find = |name: &str| values.iter().find(|v| v.0 == name).map(|v| &v.1);
    let mut out = Vec::new();
    for (name, value) in values {
        if let Some(n) = name.strip_prefix("prm").and_then(|rest| rest.strip_suffix("_name")) {
            // the name half of a pair: shown through its value half
            if let (Value::Text(key), Some(value)) = (value, find(&format!("prm{n}_value"))) {
                if !key.is_empty() {
                    out.push((key.clone(), value.clone()));
                }
            }
        } else if !(name.starts_with("prm") && (name.ends_with("_value") || name.ends_with("_type"))) {
            out.push((name.clone(), value.clone()));
        }
    }
    out
}

/// The IFC entity a record holds the data of, from the name of its format: Vectorworks keeps
/// the IFC data of an object in records named `NNA_IfcDoor`, `NNA_TIfcDoor` (the entity given by
/// the kind of object) or `NNA_4IfcDoor` (for IFC 4).
pub fn ifc_entity(format: &str) -> Option<&str> {
    let rest = format.strip_prefix("NNA_")?;
    let rest = rest.strip_prefix('4').or(rest.strip_prefix('T')).unwrap_or(rest);
    rest.starts_with("Ifc").then_some(rest)
}

/// The name a record is shown under: that of its format, without the prefix Vectorworks gives
/// its own IFC records (`NNA_Pset_WallCommon` is `Pset_WallCommon`; those for IFC 4 say so).
pub fn label(format: &str) -> String {
    match format.strip_prefix("NNA_") {
        Some(rest) => match rest.strip_prefix('4') {
            Some(four) => format!("{four} (IFC4)"),
            None => rest.strip_prefix('T').filter(|r| r.starts_with("Ifc")).unwrap_or(rest).to_string(),
        },
        None => format.to_string(),
    }
}

/// What a plug-in object (type 86) is: the name of the format of its last record, the one
/// that holds its settings (`Door`, `Window`, `Slab`, `Space`, `Stairs CW`, ..).
pub fn plug_in<'r>(raw: &'r Raw, i: usize) -> Option<&'r String> {
    let last = raw.children[i].iter().rev().map(|&c| &raw.records[c]).find(|r| r.kind() == kind::RECORD)?;
    raw.names.get(&format_of(last)?)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::shapes::tests::record;

    /// A record format with `fields` (name, kind number, offset).
    pub(crate) fn format_record(fields: &[(&str, u16, u16)]) -> Vec<u8> {
        let mut r = record(kind::RECORD_FORMAT, 0x60 + 136 * fields.len(), &[]);
        r[0x50..0x54].copy_from_slice(&(fields.len() as u32).to_le_bytes());
        for (k, (name, number, offset)) in fields.iter().enumerate() {
            let at = 0x60 + 136 * k;
            r[at..at + 2].copy_from_slice(&number.to_le_bytes());
            r[at + 2..at + 4].copy_from_slice(&offset.to_le_bytes());
            let text: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
            r[at + 8..at + 8 + text.len()].copy_from_slice(&text);
            // what is left in memory after the name
            r[at + 8 + text.len() + 2..at + 136].fill(0xab);
        }
        r
    }

    /// The record of sample 90: format 41, one byte to pass over, the text `Balk-1`.
    pub(crate) fn sample_90() -> Vec<u8> {
        let mut r = record(kind::RECORD, 0x56, &[]);
        r[0x50..0x54].copy_from_slice(&41u32.to_le_bytes());
        r[0x54] = 1;
        r.push(1);
        r.extend(12u32.to_le_bytes());
        r.extend(8u32.to_le_bytes());
        r.extend("Balk-1".encode_utf16().flat_map(u16::to_le_bytes));
        let n = r.len() as u64;
        r[0..8].copy_from_slice(&n.to_le_bytes());
        r
    }

    #[test]
    fn record_of_sample_90() {
        let format_bytes = format_record(&[("Naam", 4, 0)]);
        let fields = format(&Record { offset: 0, bytes: &format_bytes }).unwrap();
        assert_eq!(fields, [Field { name: "Naam".into(), kind: Kind::Text, offset: 0 }]);
        let record_bytes = sample_90();
        assert_eq!(record_bytes.len(), 107);
        let r = Record { offset: 0, bytes: &record_bytes };
        assert_eq!(format_of(&r), Some(41));
        assert_eq!(values(&r, &fields), [("Naam".to_string(), Value::Text("Balk-1".into()))]);
    }

    #[test]
    fn numbers_booleans_and_choices() {
        // an integer, a boolean, a number of 10 bytes and a choice, without bytes to pass over
        let fields = format(&Record { offset: 0, bytes: &format_record(&[("Count", 1, 0), ("On", 2, 4), ("Length", 7, 6), ("Mode", 8, 16), ("Other", 27, 24)]) }).unwrap();
        let mut r = record(kind::RECORD, 0x56, &[]);
        r.extend((-3i32).to_le_bytes());
        r.extend(1u16.to_le_bytes());
        r.extend(127.0f64.to_le_bytes());
        r.extend([0x69, 0]);
        r.extend(26u32.to_le_bytes());
        r.extend(25u32.to_le_bytes());
        r.push(7);
        r.extend("Auto\0Auto\0Fit".encode_utf16().flat_map(u16::to_le_bytes));
        let got = values(&Record { offset: 0, bytes: &r }, &fields);
        assert_eq!(
            got,
            [("Count".to_string(), Value::Integer(-3)), ("On".to_string(), Value::Boolean(true)), ("Length".to_string(), Value::Number(127.0)), ("Mode".to_string(), Value::Text("Auto".into()))]
        );
        // a record cut short gives what is there
        assert_eq!(values(&Record { offset: 0, bytes: &r[..0x5c] }, &fields).len(), 2);
    }

    #[test]
    fn ifc_records_by_their_names() {
        assert_eq!((ifc_entity("NNA_IfcDoor"), ifc_entity("NNA_TIfcSlab"), ifc_entity("NNA_4IfcWall")), (Some("IfcDoor"), Some("IfcSlab"), Some("IfcWall")));
        assert_eq!((ifc_entity("NNA_Pset_WallCommon"), ifc_entity("Gegevens")), (None, None));
        assert_eq!(
            (label("NNA_Pset_WallCommon"), label("NNA_4Pset_WallCommon"), label("NNA_TIfcSlab"), label("Gegevens")),
            ("Pset_WallCommon".into(), "Pset_WallCommon (IFC4)".into(), "IfcSlab".into(), "Gegevens".into())
        );
    }

    #[test]
    fn name_and_value_fields_pair_up() {
        let text = |s: &str| Value::Text(s.into());
        let values = vec![
            ("IFC_UUID".to_string(), text("{1}")),
            ("prm0_name".to_string(), text("GlobalId")),
            ("prm0_value".to_string(), text("3Ur")),
            ("prm1_name".to_string(), text("")),
            ("prm1_value".to_string(), text("x")),
        ];
        assert_eq!(pairs(&values), [("IFC_UUID".to_string(), text("{1}")), ("GlobalId".to_string(), text("3Ur"))]);
    }
}
