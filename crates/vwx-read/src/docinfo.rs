//! What a document says about itself besides its objects: units, user origin, georeference,
//! stories and levels, the project data of the title block, the page (`docs/FORMAT.md` §
//! Document settings).

use vwx_core::container::{Record, Unpacked, RECORD_HEADER};
use vwx_core::le;
use vwx_model::{AngleUnit, DocumentSettings, Georeference, Level, LevelTemplate, Origin, Page, ProjectInfo, Story, UnitFormat, UnitSystem, Units};

use crate::raw::{kind, utf16, Raw};

/// Distance between the primary and the secondary units in the document header.
const SECONDARY: usize = 0x5c6;
/// Inches in a metre fraction: one inch is 0.0254 m.
const INCH: f64 = 0.0254;

/// All settings of a document.
pub fn settings(unpacked: &Unpacked, raw: &Raw) -> DocumentSettings {
    let header = &unpacked.document;
    let (level_types, level_templates) = (level_types(raw), level_templates(raw));
    let mut stories = stories(raw);
    stories.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
    DocumentSettings { units: units(header), origin: origin(header), georeference: georeference(raw), stories, level_types, level_templates, project: project(raw), page: page(header) }
}

/// A text of the document header: UTF-16 up to its zero, within `room` bytes.
fn header_text(header: &[u8], at: usize, room: usize) -> String {
    let Some(bytes) = header.get(at..at + room) else { return String::new() };
    let end = bytes.as_chunks::<2>().0.iter().position(|c| *c == [0, 0]).map_or(room, |n| 2 * n);
    utf16(&bytes[..end])
}

/// The units of the document header (`docs/FORMAT.md`): the primary set from 0x114, the secondary
/// 0x5c6 bytes later. The angle unit is the `u16` at 0x116.
pub fn units(header: &[u8]) -> Units {
    let angle = match le::u16(header, 0x116) {
        Some(1) => AngleUnit::Radians,
        Some(2) => AngleUnit::Gradians,
        _ => AngleUnit::Degrees,
    };
    Units { primary: unit_system(header, 0), secondary: unit_system(header, SECONDARY), angle }
}

fn unit_system(h: &[u8], d: usize) -> UnitSystem {
    let upi = le::f64(h, 0x118 + d).filter(|v| v.is_finite() && *v > 0.0).unwrap_or(25.4);
    let flags = le::u16(h, 0x1d4 + d).unwrap_or(0);
    let format = match le::u16(h, 0x130 + d) {
        Some(1) => UnitFormat::Fractional,
        Some(2) => UnitFormat::DecimalFeetInches,
        Some(3) => UnitFormat::FractionalFeetInches,
        _ => UnitFormat::Decimal,
    };
    UnitSystem {
        name: header_text(h, 0x132 + d, 0x2a),
        mark: header_text(h, 0x15c + d, 0x12),
        format,
        units_per_inch: upi,
        metres_per_unit: INCH / upi,
        display_precision: le::u32(h, 0x1d0 + d).unwrap_or(0),
        dimension_precision: u32::from(le::u16(h, 0x1d6 + d).unwrap_or(0)),
        leading_zero: flags & 0x40 != 0,
        trailing_zeros: flags & 0x20 != 0,
        show_mark: flags & 0x80 != 0,
        thousands_separator: flags & 0x10 != 0,
        area_name: header_text(h, 0x1e4 + d, 0x2a),
        area_mark: header_text(h, 0x20e + d, 0x1a),
        area_units_per_square_inch: le::f64(h, 0x120 + d).unwrap_or(0.0),
        area_precision: u32::from(le::u16(h, 0x228 + d).unwrap_or(0)),
        volume_name: header_text(h, 0x22c + d, 0x2a),
        volume_mark: header_text(h, 0x256 + d, 0x1a),
        volume_units_per_cubic_inch: le::f64(h, 0x128 + d).unwrap_or(0.0),
        volume_precision: u32::from(le::u16(h, 0x270 + d).unwrap_or(0)),
    }
}

/// The user origin: two `f64` in millimetres in the document header, y at 0x882 and x at 0x88a
/// (samples 604: origin (1000, 2000) is stored y = 2000 then x = 1000, in millimetres whatever
/// the document units).
pub fn origin(header: &[u8]) -> Origin {
    let get = |at| le::f64(header, at).filter(|v| v.is_finite()).unwrap_or(0.0) / 1000.0;
    Origin { x: get(0x88a), y: get(0x882) }
}

/// The page: counts of pages across (`u16` at 0xf6) and down (0xf8) and the printable size of one
/// page in inches (`f64` at 0xfc and 0x104).
pub fn page(header: &[u8]) -> Page {
    let inches = |at| le::f64(header, at).filter(|v| v.is_finite() && *v > 0.0).unwrap_or(0.0) * INCH;
    Page { width: inches(0xfc), height: inches(0x104), columns: le::u16(header, 0xf6).unwrap_or(0), rows: le::u16(header, 0xf8).unwrap_or(0) }
}

/// The tag of a user data record that is not `TDMD`: its first four bytes, as stored.
fn tag(r: &Record) -> Option<[u8; 4]> {
    r.bytes.get(RECORD_HEADER..RECORD_HEADER + 4)?.try_into().ok()
}

/// Walks the items of a user data record: each a `u32` length and that many bytes.
struct Items<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Items<'a> {
    fn new(bytes: &'a [u8], at: usize) -> Items<'a> {
        Items { bytes, at }
    }

    fn next(&mut self) -> Option<&'a [u8]> {
        let len = le::u32(self.bytes, self.at)? as usize;
        let item = self.bytes.get(self.at + 4..self.at.checked_add(4)?.checked_add(len)?)?;
        self.at += 4 + len;
        Some(item)
    }

    fn text(&mut self) -> Option<String> {
        self.next().map(item_text)
    }

    fn real(&mut self) -> Option<f64> {
        le::f64(self.next()?, 0)
    }
}

/// A text item: UTF-16 after a byte order mark `ff fe`, ending in a zero.
fn item_text(item: &[u8]) -> String {
    let body = item.strip_prefix(&[0xff, 0xfe]).unwrap_or(item);
    let end = body.as_chunks::<2>().0.iter().position(|c| *c == [0, 0]).map_or(body.len() & !1, |n| 2 * n);
    utf16(&body[..end])
}

/// Where the items of a record of the document tables start: the tag, 12 bytes, then a `u32`
/// with the length of what follows (counted from 0x5e).
const ITEMS: usize = 0x64;

/// The level types (user data `LLTP`, children of `LLTL`): the name of each.
fn level_types(raw: &Raw) -> Vec<String> {
    let mut out = Vec::new();
    for r in raw.records.iter().filter(|r| r.kind() == kind::USER_DATA && tag(r) == Some(*b"PTLL")) {
        if let Some(name) = Items::new(r.bytes, ITEMS).text() {
            out.push(name);
        }
    }
    out
}

/// The level templates (`STLT`): layer name, scale, level type, elevation and wall height.
fn level_templates(raw: &Raw) -> Vec<LevelTemplate> {
    let mut out = Vec::new();
    for r in raw.records.iter().filter(|r| r.kind() == kind::USER_DATA && tag(r) == Some(*b"TLTS")) {
        let mut items = Items::new(r.bytes, ITEMS);
        let (Some(layer), Some(scale), Some(level_type), Some(elevation), Some(wall_height)) = (items.text(), items.real(), items.text(), items.real(), items.real()) else {
            continue;
        };
        out.push(LevelTemplate { layer, scale, level_type, elevation: elevation / 1000.0, wall_height: wall_height / 1000.0 });
    }
    out
}

/// The stories (`YRTS`, children of `STRL`): the suffix, the elevation in millimetres, then a
/// list of levels. The name is that of the object in the name list.
fn stories(raw: &Raw) -> Vec<Story> {
    let mut out = Vec::new();
    for r in raw.records.iter().filter(|r| r.kind() == kind::USER_DATA && tag(r) == Some(*b"YRTS")) {
        if let Some(story) = story(&raw.names, r) {
            out.push(story);
        }
    }
    out
}

fn story(names: &std::collections::HashMap<u32, String>, r: &Record) -> Option<Story> {
    let mut items = Items::new(r.bytes, ITEMS);
    let suffix = items.text()?;
    let elevation = items.real()?;
    // 4 bytes, a `u16`, a `u32` with the length of the list, then its count as an item
    items.at += 4 + 2 + 4;
    let count = le::u32(items.next()?, 0)? as usize;
    // 4 bytes, then the levels: `u16` number, `u32` length of the level counted from the number
    // (itself included), and in it the level type, the elevation and the layer
    let mut at = items.at + 4;
    let mut levels = Vec::new();
    for _ in 0..count.min(256) {
        let size = le::u32(r.bytes, at + 2)? as usize;
        let end = at.checked_add(size)?.min(r.bytes.len());
        let mut level = Items::new(&r.bytes[..end], at + 6);
        let (Some(name), Some(elev), Some(layer)) = (level.text(), level.real(), level.text()) else { break };
        levels.push(Level { name, elevation: elev / 1000.0, layer });
        at = end;
    }
    let layers = levels.iter().map(|l| l.layer.clone()).filter(|l| !l.is_empty()).collect();
    Some(Story { name: names.get(&r.ref_id()).cloned().unwrap_or_default(), suffix, elevation: elevation / 1000.0, levels, layers })
}

/// The georeference (user data `GRef` at document level, absent when the document has none): a
/// list of items: a text, latitude, longitude and angle to north as `f64`, three texts, four
/// `f64` (the last the project elevation in millimetres), and further down the coordinate
/// system as Well Known Text.
fn georeference(raw: &Raw) -> Option<Georeference> {
    let r = raw.records.iter().enumerate().find(|(i, r)| r.kind() == kind::USER_DATA && tag(r) == Some(*b"feRG") && raw.parents[*i].is_none())?.1;
    let mut items = Items::new(r.bytes, ITEMS);
    items.text()?;
    let (latitude, longitude, angle_to_north) = (items.real()?, items.real()?, items.real()?);
    for _ in 0..3 {
        items.text()?;
    }
    let mut elevation = 0.0;
    for k in 0..4 {
        let v = items.real()?;
        if k == 3 {
            elevation = v;
        }
    }
    // the rest: find the coordinate system text; its name is the text before it, when there is one
    let rest = r.bytes.get(items.at..)?;
    let wkt = text_starting(rest, "PROJCS[").or_else(|| text_starting(rest, "GEOGCS["))?;
    let epsg = wkt.rfind("AUTHORITY[\"EPSG\",\"").and_then(|i| {
        let digits: String = wkt[i + 18..].chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    });
    let crs_name = wkt.split('"').nth(1).unwrap_or_default().to_string();
    Some(Georeference { latitude, longitude, angle_to_north, project_elevation: elevation / 1000.0, wkt, epsg, crs_name })
}

/// The first text item of `bytes` (a record tail, items of unknown count) that starts with
/// `prefix`: found by scanning for the UTF-16 prefix and taking the zero-ended text from there.
fn text_starting(bytes: &[u8], prefix: &str) -> Option<String> {
    let needle: Vec<u8> = prefix.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let at = bytes.windows(needle.len()).position(|w| w == needle)?;
    let tail = &bytes[at..];
    let end = tail.as_chunks::<2>().0.iter().position(|c| *c == [0, 0]).map_or(tail.len() & !1, |n| 2 * n);
    Some(utf16(&tail[..end]))
}

/// The project data of the title block: the values of the first record of the format
/// `Title Block Project Data` (Vectorworks keeps the same data in each title block of a file).
fn project(raw: &Raw) -> ProjectInfo {
    use crate::records::{self, Value};
    for r in raw.records.iter().filter(|r| r.kind() == kind::RECORD) {
        let Some(number) = records::format_of(r) else { continue };
        if raw.names.get(&number).map(String::as_str) != Some("Title Block Project Data") {
            continue;
        }
        let Some(fields) = raw.by_ref.get(&number).map(|&f| &raw.records[f]).filter(|f| f.kind() == kind::RECORD_FORMAT).and_then(records::format) else { continue };
        let values = records::values(r, &fields);
        let text = |v: &Value| match v {
            Value::Text(t) => t.replace('\r', "\n"),
            Value::Integer(n) => n.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Boolean(b) => b.to_string(),
            other => format!("{other:?}"),
        };
        let fields: Vec<(String, String)> = values.iter().map(|(k, v)| (k.clone(), text(v))).collect();
        let get = |names: &[&str]| names.iter().find_map(|n| fields.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone())).unwrap_or_default();
        return ProjectInfo {
            name: get(&["Project Name"]),
            number: get(&["Project ID", "Project Number"]),
            address: get(&["Project Address"]),
            client: get(&["Client", "Opdrachtgever", "Owner"]),
            date: get(&["Project Date"]),
            file_name: get(&["File Name"]),
            fields,
        };
    }
    ProjectInfo::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_item(s: &str) -> Vec<u8> {
        let mut body = vec![0xff, 0xfe];
        body.extend(s.encode_utf16().flat_map(u16::to_le_bytes));
        body.extend([0, 0]);
        let mut out = (body.len() as u32).to_le_bytes().to_vec();
        out.extend(body);
        out
    }

    fn real_item(v: f64) -> Vec<u8> {
        let mut out = 8u32.to_le_bytes().to_vec();
        out.extend(v.to_le_bytes());
        out
    }

    /// A user data record with a tag and the items after the 16-byte head.
    fn user_data(tag: &[u8; 4], items: &[Vec<u8>]) -> Vec<u8> {
        let mut r = vec![0u8; RECORD_HEADER];
        r[0x19] = kind::USER_DATA;
        r.extend(tag);
        r.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x11]);
        r.extend(0u32.to_le_bytes());
        for item in items {
            r.extend(item);
        }
        let n = r.len() as u64;
        r[0..8].copy_from_slice(&n.to_le_bytes());
        r
    }

    #[test]
    fn units_of_a_header() {
        let mut h = vec![0u8; 0x9e0];
        // millimetres: 25.4 units per inch, decimal, leading zero and metric flags
        h[0x118..0x120].copy_from_slice(&25.4f64.to_le_bytes());
        for (k, c) in "Millimeter".encode_utf16().enumerate() {
            h[0x132 + 2 * k..0x134 + 2 * k].copy_from_slice(&c.to_le_bytes());
        }
        for (k, c) in "mm".encode_utf16().enumerate() {
            h[0x15c + 2 * k..0x15e + 2 * k].copy_from_slice(&c.to_le_bytes());
        }
        h[0x1d4] = 0x41;
        h[0x1d6] = 2;
        let u = units(&h);
        assert_eq!((u.primary.name.as_str(), u.primary.mark.as_str()), ("Millimeter", "mm"));
        assert!((u.primary.metres_per_unit - 0.001).abs() < 1e-12);
        assert!(u.primary.leading_zero && !u.primary.trailing_zeros && !u.primary.show_mark);
        assert_eq!((u.primary.format, u.primary.dimension_precision, u.angle), (UnitFormat::Decimal, 2, AngleUnit::Degrees));
        // feet and inches in the secondary set, with the mark shown
        h[SECONDARY + 0x118..SECONDARY + 0x120].copy_from_slice(&1.0f64.to_le_bytes());
        h[SECONDARY + 0x130] = 2;
        h[SECONDARY + 0x1d4] = 0x86;
        h[0x116] = 1;
        let u = units(&h);
        assert_eq!((u.secondary.format, u.secondary.show_mark, u.angle), (UnitFormat::DecimalFeetInches, true, AngleUnit::Radians));
        assert!((u.secondary.metres_per_unit - 0.0254).abs() < 1e-12);
    }

    #[test]
    fn origin_and_page_of_a_header() {
        let mut h = vec![0u8; 0x9e0];
        h[0x882..0x88a].copy_from_slice(&2000.0f64.to_le_bytes());
        h[0x88a..0x892].copy_from_slice(&(-3000.0f64).to_le_bytes());
        h[0xf6] = 4;
        h[0xf8] = 3;
        h[0xfc..0x104].copy_from_slice(&16.535f64.to_le_bytes());
        h[0x104..0x10c].copy_from_slice(&11.69f64.to_le_bytes());
        assert_eq!(origin(&h), Origin { x: -3.0, y: 2.0 });
        let p = page(&h);
        assert_eq!((p.columns, p.rows), (4, 3));
        assert!((p.width - 0.42).abs() < 1e-3 && (p.height - 0.297).abs() < 1e-3);
    }

    #[test]
    fn story_with_two_levels() {
        // a story "2": suffix, elevation 6100 mm, two levels (floor with layer, ceiling with layer)
        let level = |n: u16, name: &str, elev: f64, layer: &str| {
            let mut body = text_item(name);
            body.extend(real_item(elev));
            body.extend(text_item(layer));
            body.extend(4u32.to_le_bytes());
            body.extend(0u32.to_le_bytes());
            let mut out = n.to_le_bytes().to_vec();
            out.extend(((body.len() + 6) as u32).to_le_bytes());
            out.extend(body);
            out
        };
        let mut items = vec![text_item("2"), real_item(6100.0)];
        let mut tail = vec![0u8; 4];
        tail.extend(2u16.to_le_bytes());
        tail.extend(0u32.to_le_bytes());
        tail.extend(4u32.to_le_bytes());
        tail.extend(2u32.to_le_bytes());
        tail.extend([0u8; 4]);
        tail.extend(level(0, "Sample Floor", 0.0, "Plan-2"));
        tail.extend(level(1, "Sample Ceiling", 2700.0, "Ceiling-2"));
        items.push(tail);
        let bytes = user_data(b"YRTS", &items);
        let story = story(&Default::default(), &Record { offset: 0, bytes: &bytes }).unwrap();
        assert_eq!((story.suffix.as_str(), story.elevation), ("2", 6.1));
        assert_eq!(story.levels.len(), 2);
        assert_eq!(story.levels[1], Level { name: "Sample Ceiling".into(), elevation: 2.7, layer: "Ceiling-2".into() });
        assert_eq!(story.layers, ["Plan-2", "Ceiling-2"]);
    }

    #[test]
    fn level_types_and_templates() {
        let bytes = user_data(b"PTLL", &[text_item("Top of Slab")]);
        assert_eq!(Items::new(&bytes, ITEMS).text().as_deref(), Some("Top of Slab"));
        let bytes = user_data(b"TLTS", &[text_item("Slab"), real_item(50.0), text_item("Top of Slab"), real_item(0.0), real_item(2700.0)]);
        let mut items = Items::new(&bytes, ITEMS);
        assert_eq!((items.text().unwrap(), items.real().unwrap(), items.text().unwrap()), ("Slab".to_string(), 50.0, "Top of Slab".to_string()));
    }

    #[test]
    fn coordinate_system_text() {
        let wkt = "PROJCS[\"Amersfoort / RD New\",AUTHORITY[\"EPSG\",\"28992\"]]";
        let mut tail: Vec<u8> = vec![0, 0, 0];
        tail.extend(wkt.encode_utf16().flat_map(u16::to_le_bytes));
        tail.extend([0, 0]);
        assert_eq!(text_starting(&tail, "PROJCS[").as_deref(), Some(wkt));
    }
}
