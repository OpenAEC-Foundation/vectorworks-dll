//! The named resources of a document, as its resource browser lists them: symbol definitions and
//! the folders they are in, styles, text styles, record formats, document scripts and script
//! palettes, saved views, fonts, and the descriptions of classes and layers (`docs/FORMAT.md`
//! § Resources).

use vwx_core::container::{Record, RECORD_HEADER};
use vwx_core::le;
use vwx_model::{Resource, Value};

use crate::raw::{self, kind, utf16, Raw};

/// Kinds of resource that are listed when they have a name.
const NAMED: &[u8] = &[16, 18, 19, 36, 37, 47, 49, 51, 66, 92, 96, 97, 102, 106, 107, 108, 109, 115, 119, 120, 127];

/// What holds resources: the roots of the symbol library (54), the other resources (57) and the
/// hatches (65), a folder (92) and a script palette (51).
const HOLDERS: &[u8] = &[54, 57, 65, 92, 51];

/// The tag (reversed) of the user data of a class or layer that holds its description.
const DESCRIPTION: &[u8; 4] = b"CSED";

/// Windows-1252 for the bytes 0x80 to 0x9f (0 where it has no character).
const CP1252: [u16; 32] = [
    0x20ac, 0, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017d, 0, 0, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x02dc, 0x2122, 0x0161,
    0x203a, 0x0153, 0, 0x017e, 0x0178,
];

/// Text kept as bytes in the code page of the system (Windows-1252), as scripts are.
fn ansi(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9f => char::from_u32(u32::from(CP1252[usize::from(b - 0x80)])).filter(|c| *c != '\0').unwrap_or('\u{fffd}'),
            _ => char::from(b),
        })
        .collect()
}

/// The text of a script (type 49): a `u32` length at 0x50, four zero bytes, the text in the code
/// page of the system with a carriage return between lines. A Python script has an underscore
/// before its text. A saved view is such a record too, with a text of one character, a zero.
/// Returns the language (`VectorScript` or `Python`) and the text, lines ending in `\n`.
pub fn script(r: &Record) -> Option<(&'static str, String)> {
    let len = le::u32(r.bytes, RECORD_HEADER)? as usize;
    let text = r.bytes.get(RECORD_HEADER + 8..RECORD_HEADER + 8 + len)?;
    let text = ansi(text.split(|&b| b == 0).next().unwrap_or_default());
    Some(match text.strip_prefix('_') {
        Some(python) => ("Python", python.replace('\r', "\n")),
        None => ("VectorScript", text.replace('\r', "\n")),
    })
}

/// The text of the user data tagged `CSED` (the description of a class or a layer): a `u32` size
/// at 0x60, a `u32` count of bytes at 0x64 and that many of UTF-16 text, starting with a byte
/// order mark and ending in a zero.
pub fn description(r: &Record) -> Option<String> {
    if r.bytes.get(RECORD_HEADER..RECORD_HEADER + 4)? != DESCRIPTION {
        return None;
    }
    let len = le::u32(r.bytes, RECORD_HEADER + 0x14)? as usize;
    let text = r.bytes.get(RECORD_HEADER + 0x18..RECORD_HEADER + 0x18 + len)?;
    let text = text.strip_prefix(&[0xff, 0xfe]).unwrap_or(text);
    Some(utf16(text).trim_end_matches('\0').to_string())
}

/// The size of the letters of a text style (type 109) in points: an `f64` at 0x58, in inches
/// (a style of 12 points holds 1/6; the names of the styles of the corpus say 9, 10 and 12).
pub fn text_style_size(r: &Record) -> Option<f64> {
    le::f64(r.bytes, 0x58).filter(|s| s.is_finite() && *s > 0.0).map(|inches| (inches * 72.0 * 1000.0).round() / 1000.0)
}

/// The fonts of the document (type 74): a `u32` count at 0x50, then per font 518 bytes from
/// 0x54: a `u16` number (the font id the objects name) and the name as UTF-16 ending in a zero.
pub fn fonts(r: &Record) -> Vec<(u16, String)> {
    let count = le::u32(r.bytes, RECORD_HEADER).unwrap_or(0).min(4096) as usize;
    let mut out = Vec::new();
    for k in 0..count {
        let at = 0x54 + 518 * k;
        let (Some(id), Some(name)) = (le::u16(r.bytes, at), r.bytes.get(at + 2..at + 258)) else { break };
        let end = name.as_chunks::<2>().0.iter().position(|c| *c == [0, 0]).map_or(name.len(), |n| 2 * n);
        out.push((id, utf16(&name[..end])));
    }
    out
}

fn fields_of(r: &Record) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    match r.kind() {
        49 => {
            if let Some((language, text)) = script(r) {
                if !text.is_empty() {
                    out.push(("language".to_string(), Value::Text(language.to_string())));
                    out.push(("text".to_string(), Value::Text(text)));
                }
            }
        }
        109 => {
            if let Some(size) = text_style_size(r) {
                out.push(("size".to_string(), Value::Real(size)));
            }
        }
        _ => {}
    }
    out
}

/// The resources of a document, in file order.
pub fn read(raw: &Raw) -> Vec<Resource> {
    let mut out = Vec::new();
    for (i, r) in raw.records.iter().enumerate() {
        let k = r.kind();
        if k == 74 {
            for (id, name) in fonts(r) {
                out.push(Resource { id: i as u64, kind: "font".into(), name, folder: Vec::new(), fields: vec![("font".to_string(), Value::Int(i64::from(id)))] });
            }
            continue;
        }
        if k == kind::CLASS || k == kind::LAYER {
            let text = raw.children[i].iter().find_map(|&c| description(&raw.records[c]));
            if let Some(text) = text.filter(|t| !t.is_empty()) {
                let name = if k == kind::LAYER { raw::layer_name(r.bytes) } else { raw.names.get(&r.ref_id()).cloned() };
                let kind_name = if k == kind::LAYER { "layer" } else { "class" };
                out.push(Resource { id: i as u64, kind: kind_name.into(), name: name.unwrap_or_default(), folder: Vec::new(), fields: vec![("description".to_string(), Value::Text(text))] });
            }
            continue;
        }
        if !NAMED.contains(&k) {
            continue;
        }
        let Some(parent) = raw.parents[i] else { continue };
        if !HOLDERS.contains(&raw.records[parent].kind()) {
            continue;
        }
        let Some(name) = raw.names.get(&r.ref_id()).filter(|n| !n.is_empty()) else { continue };
        // the folders around it, innermost first while walking up
        let mut folder = Vec::new();
        let mut up = parent;
        loop {
            let holder = &raw.records[up];
            if matches!(holder.kind(), 92 | 51) {
                folder.push(raw.names.get(&holder.ref_id()).cloned().unwrap_or_default());
            }
            match raw.parents[up] {
                Some(p) if holder.kind() == 92 && folder.len() < 64 => up = p,
                _ => break,
            }
        }
        folder.reverse();
        let mut kind_name = crate::kinds::name(k).to_string();
        let fields = fields_of(r);
        // a saved view is a script without text
        if k == 49 && fields.is_empty() {
            kind_name = "saved view".into();
        }
        out.push(Resource { id: i as u64, kind: kind_name, name: name.clone(), folder, fields });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A record of `kind` under the parent address `parent` with `payload` after the header.
    fn record(kind: u8, parent: u64, payload: &[u8]) -> Vec<u8> {
        let mut r = vec![0u8; RECORD_HEADER];
        r[0..8].copy_from_slice(&((RECORD_HEADER + payload.len()) as u64).to_le_bytes());
        r[0x19] = kind;
        r[0x26..0x2e].copy_from_slice(&parent.to_le_bytes());
        r.extend_from_slice(payload);
        r
    }

    fn with(mut r: Vec<u8>, reference: u32) -> Vec<u8> {
        r[0x1a..0x1e].copy_from_slice(&reference.to_le_bytes());
        r
    }

    fn names(list: &[(u32, &str)]) -> Vec<u8> {
        let mut payload = vec![0u8; 12];
        payload[8..12].copy_from_slice(&(list.len() as u32).to_le_bytes());
        for (id, name) in list {
            let text: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
            payload.extend(id.to_le_bytes());
            payload.extend((text.len() as u32).to_le_bytes());
            payload.extend(text);
        }
        record(kind::NAMES, 1, &payload)
    }

    fn script_record(text: &[u8]) -> Vec<u8> {
        let mut payload = (text.len() as u32).to_le_bytes().to_vec();
        payload.extend([0; 4]);
        payload.extend(text);
        record(49, 31, &payload)
    }

    fn description_record(text: &str, parent: u64) -> Vec<u8> {
        let units: Vec<u8> = std::iter::once(0xfeffu16).chain(text.encode_utf16()).chain([0]).flat_map(u16::to_le_bytes).collect();
        let mut payload = b"CSED".to_vec();
        payload.extend([0, 0]);
        payload.extend(((units.len() + 24) as u32).to_le_bytes());
        payload.extend([0, 0, 0, 0, 0, 0x11]);
        payload.extend(((units.len() + 14) as u32).to_le_bytes());
        payload.extend((units.len() as u32).to_le_bytes());
        payload.extend(units);
        payload.extend([0; 4]);
        record(76, parent, &payload)
    }

    fn parent_of(mut r: Vec<u8>, address: u64) -> Vec<u8> {
        r[0x26..0x2e].copy_from_slice(&address.to_le_bytes());
        r
    }

    fn open(records: Vec<Vec<u8>>) -> Vec<Resource> {
        let body = records.concat();
        let records = vwx_core::container::records(&body).unwrap();
        let parents = vwx_core::container::parents(&records);
        let mut children = vec![Vec::new(); records.len()];
        let mut names = std::collections::HashMap::new();
        for (i, r) in records.iter().enumerate() {
            if let Some(p) = parents[i] {
                children[p].push(i);
            }
            if r.kind() == kind::NAMES {
                let n = le::u32(r.bytes, RECORD_HEADER + 8).unwrap();
                let mut at = RECORD_HEADER + 12;
                for _ in 0..n {
                    let (id, len) = (le::u32(r.bytes, at).unwrap(), le::u32(r.bytes, at + 4).unwrap() as usize);
                    names.insert(id, utf16(&r.bytes[at + 8..at + 8 + len]));
                    at += 8 + len;
                }
            }
        }
        let raw = Raw { records, parents, children, names, by_ref: Default::default(), palette: Default::default() };
        read(&raw)
    }

    #[test]
    fn scripts_and_saved_views_in_a_palette() {
        let records = vec![
            names(&[(5, "Palette"), (6, "Hi"), (7, "Py"), (8, "View A")]),
            with(record(57, 1, &[0; 4]), 0),
            with(parent_of(record(51, 1, &[0; 4]), 2), 5),
            with(parent_of(script_record(b"PROCEDURE a;\rBEGIN\rEND;"), 3), 6),
            with(parent_of(script_record(b"_import vs\rvs.Message('\xe9')"), 3), 7),
            with(parent_of(script_record(b"\0"), 3), 8),
        ];
        let resources = open(records);
        let kinds: Vec<_> = resources.iter().map(|r| (r.kind.as_str(), r.name.as_str(), r.folder.clone())).collect();
        assert_eq!(
            kinds,
            [
                ("script palette", "Palette", vec![]),
                ("document script", "Hi", vec!["Palette".to_string()]),
                ("document script", "Py", vec!["Palette".to_string()]),
                ("saved view", "View A", vec!["Palette".to_string()])
            ]
        );
        assert_eq!(resources[1].fields[1].1, Value::Text("PROCEDURE a;\nBEGIN\nEND;".into()));
        assert_eq!(resources[2].fields[0].1, Value::Text("Python".into()));
        assert_eq!(resources[2].fields[1].1, Value::Text("import vs\nvs.Message('\u{e9}')".into()));
        assert!(resources[3].fields.is_empty());
    }

    #[test]
    fn nested_folders_and_text_styles() {
        let mut payload = vec![0u8; 110 - RECORD_HEADER];
        payload[8..16].copy_from_slice(&(10.0f64 / 72.0).to_le_bytes());
        let style = record(109, 1, &payload);
        let records = vec![
            names(&[(2, "Outer"), (3, "Inner"), (4, "Sym"), (5, "Body 10 pt")]),
            with(record(54, 1, &[0; 4]), 0),
            with(parent_of(record(92, 1, &[0; 4]), 2), 2),
            with(parent_of(record(92, 1, &[0; 4]), 3), 3),
            with(parent_of(record(16, 1, &[0; 4]), 4), 4),
            with(parent_of(style, 2), 5),
        ];
        let resources = open(records);
        let style = resources.iter().find(|r| r.name == "Body 10 pt").unwrap();
        assert_eq!((style.kind.as_str(), &style.fields[0]), ("text style", &("size".to_string(), Value::Real(10.0))));
        let folder = resources.iter().find(|r| r.name == "Sym").unwrap();
        assert_eq!(folder.folder, ["Outer", "Inner"]);
        assert_eq!(folder.kind, "symbol definition");
        let inner = resources.iter().find(|r| r.name == "Inner").unwrap();
        assert_eq!((inner.kind.as_str(), inner.folder.clone()), ("symbol folder", vec!["Outer".to_string()]));
    }

    #[test]
    fn text_style_size_is_in_points() {
        let mut payload = vec![0u8; 110 - RECORD_HEADER];
        payload[8..16].copy_from_slice(&(12.0f64 / 72.0).to_le_bytes());
        let bytes = record(109, 1, &payload);
        assert_eq!(text_style_size(&Record { offset: 0, bytes: &bytes }), Some(12.0));
    }

    #[test]
    fn descriptions_of_classes_and_layers() {
        let class = with(record(kind::CLASS, 1, &[0; 8]), 9);
        let records = vec![names(&[(9, "Walls")]), class, description_record("Use this class for walls", 2)];
        // the description follows its class as its child
        let resources = open(records);
        assert_eq!(resources.len(), 1);
        assert_eq!((resources[0].kind.as_str(), resources[0].name.as_str()), ("class", "Walls"));
        assert_eq!(resources[0].fields[0].1, Value::Text("Use this class for walls".into()));
    }

    #[test]
    fn fonts_are_listed_with_their_ids() {
        let mut payload = 2u32.to_le_bytes().to_vec();
        for (id, name) in [(29u16, "Arial"), (30, "Arial Black")] {
            let mut entry = id.to_le_bytes().to_vec();
            entry.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
            entry.resize(518, 0);
            payload.extend(entry);
        }
        let bytes = record(74, 1, &payload);
        assert_eq!(fonts(&Record { offset: 0, bytes: &bytes }), [(29, "Arial".to_string()), (30, "Arial Black".to_string())]);
    }
}
