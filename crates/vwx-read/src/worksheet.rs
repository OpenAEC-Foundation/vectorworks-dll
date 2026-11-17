//! Worksheets (type 18) and their images on drawings (type 56). See "Worksheets" in
//! `docs/FORMAT.md` for the layout.

use std::collections::HashMap;

use vwx_core::le;
use vwx_model::{Cell, DatabaseRow, Worksheet, WorksheetPlacement};

use crate::raw::{self, Raw};

/// A worksheet (type 18).
pub const WORKSHEET: u8 = 18;
/// A worksheet shown on a drawing (type 56).
pub const WORKSHEET_IMAGE: u8 = 56;
/// The rows a database row computed, a child of its worksheet (type 60).
pub const SUBROWS: u8 = 60;

/// Offset in a worksheet record of the two `u16` with the numbers of columns and of rows.
const SIZE_AT: usize = 0xc0;
/// Offset of the table of cell offsets, which count from `TABLE_BASE`.
const TABLE_AT: usize = 0x22a;
const TABLE_BASE: usize = 0x222;
/// Bytes of a cell before its value: the formatting.
const CELL_HEAD: usize = 106;
/// Room the computed value of a cell has before the formula, unless its text is longer.
const VALUE_ROOM: usize = 34;

/// All worksheets of a file, each with the images of it on drawings.
pub fn read(raw: &Raw) -> Vec<Worksheet> {
    let mut sheets: Vec<Worksheet> = Vec::new();
    let mut by_ref: HashMap<u32, usize> = HashMap::new();
    for (i, r) in raw.records.iter().enumerate() {
        if r.kind() != WORKSHEET {
            continue;
        }
        let Some(mut sheet) = worksheet(r.bytes) else { continue };
        sheet.id = i as u64;
        sheet.name = raw.names.get(&r.ref_id()).cloned().unwrap_or_default();
        if let Some(sub) = raw.children[i].iter().find(|&&c| raw.records[c].kind() == SUBROWS) {
            attach_subrows(&mut sheet, raw.records[*sub].bytes);
        }
        by_ref.insert(r.ref_id(), sheets.len());
        sheets.push(sheet);
    }
    for (i, r) in raw.records.iter().enumerate() {
        if r.kind() != WORKSHEET_IMAGE {
            continue;
        }
        let Some(&at) = le::u32(r.bytes, 0x8c).and_then(|n| by_ref.get(&n)) else { continue };
        let Some(b) = f64s::<4>(r.bytes, 0x68) else { continue };
        // the box in the millimetres of the layer: top, left, bottom, right
        let layer = raw.layer_of(i);
        sheets[at].placements.push(WorksheetPlacement {
            id: i as u64,
            layer: layer.and_then(|l| raw::layer_name(raw.records[l].bytes)).unwrap_or_default(),
            layer_id: layer.map(|l| l as u64),
            x: b[1] / 1000.0,
            y: b[0] / 1000.0,
            width: (b[3] - b[1]) / 1000.0,
            height: (b[0] - b[2]) / 1000.0,
        });
    }
    sheets
}

fn f64s<const N: usize>(b: &[u8], at: usize) -> Option<[f64; N]> {
    let mut out = [0.0; N];
    for (k, v) in out.iter_mut().enumerate() {
        *v = le::f64(b, at + 8 * k).filter(|x| x.is_finite())?;
    }
    Some(out)
}

/// The slots of a table: pairs of `u32`, a flag and an offset from `base`.
fn slots(b: &[u8], at: usize, count: usize) -> Option<Vec<(u32, usize)>> {
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        out.push((le::u32(b, at + 8 * k)?, le::u32(b, at + 8 * k + 4)? as usize));
    }
    Some(out)
}

/// The bytes between an offset and the next larger one of the table.
struct Spans<'a> {
    bytes: &'a [u8],
    base: usize,
    offsets: Vec<usize>,
}

impl<'a> Spans<'a> {
    fn new(bytes: &'a [u8], base: usize, table: &[(u32, usize)]) -> Spans<'a> {
        let mut offsets: Vec<usize> = table.iter().map(|s| s.1).filter(|&o| o != 0 && base + o <= bytes.len()).collect();
        offsets.sort_unstable();
        offsets.dedup();
        Spans { bytes, base, offsets }
    }

    /// The bytes that start at `offset`: up to the next offset, or the end of the record.
    fn get(&self, offset: usize) -> Option<&'a [u8]> {
        if offset == 0 {
            return None;
        }
        let end = match self.offsets.binary_search(&offset) {
            Ok(i) => self.offsets.get(i + 1).map_or(self.bytes.len(), |&n| self.base + n),
            Err(_) => return None,
        };
        self.bytes.get(self.base + offset..end)
    }
}

/// Reads a worksheet record: its size, its rows and columns, its cells and database rows.
pub fn worksheet(r: &[u8]) -> Option<Worksheet> {
    let columns = le::u16(r, SIZE_AT)? as usize;
    let rows = le::u16(r, SIZE_AT + 2)? as usize;
    // the slot of each row and column (columns + 1 slots per row, and a row before the first)
    let width = columns + 1;
    let table = slots(r, TABLE_AT, (rows + 1) * width)?;
    if TABLE_AT + 8 * table.len() > r.len() {
        return None;
    }
    let spans = Spans::new(r, TABLE_BASE, &table);
    let mut sheet = Worksheet { rows: rows as u32, columns: columns as u32, ..Worksheet::default() };
    for c in 0..columns {
        sheet.column_widths.push(spans.get(table[c].1).and_then(|b| le::u16(b, 2)).unwrap_or(0) as u32);
    }
    for y in 1..=rows {
        // a row starts where the row before ends: the last slot of that row
        let start = table[(y - 1) * width + columns].1;
        let row = spans.get(start);
        sheet.row_heights.push(row.and_then(|b| le::u16(b, 2)).unwrap_or(0) as u32);
        // the kind 4 of a row says it is a database row, whose criteria is its own stream
        if let Some(row) = row.filter(|b| le::u16(b, 0) == Some(4)) {
            sheet.database_rows.push(DatabaseRow { row: y as u32, criteria: criteria(row), subrows: Vec::new() });
        }
        for x in 1..=columns {
            let (_, offset) = table[y * width + x - 1];
            if let Some(cell) = spans.get(offset).and_then(|b| cell(b, y as u32, x as u32)) {
                sheet.cells.push(cell);
            }
        }
    }
    sheet.window = [0, 2, 4, 6].map(|k| le::u16(r, 0x8e + k).unwrap_or(0) as u32);
    Some(sheet)
}

/// The criteria of a database row record: `u32`, `u16`, `u32` length, then the formula stream
/// (the first 8 bytes are the row itself).
fn criteria(row: &[u8]) -> Option<String> {
    let len = le::u32(row, 14)? as usize;
    let stream = row.get(18..18 + len)?;
    decompile(stream, 0, 0)
}

/// Reads the cell block of a worksheet cell (`None` for an empty one): the formatting in its
/// first 106 bytes, then the value. A text cell has `u16`, `u32` count, 34 bytes of nothing and
/// the characters; a number or formula cell `u16` kind (1 formula, 2 number), `u32` length of
/// its formula, the value Vectorworks worked out in a room of at least 34 bytes, and the
/// formula.
fn cell(b: &[u8], row: u32, column: u32) -> Option<Cell> {
    if b.len() < CELL_HEAD + 6 {
        return None;
    }
    let mut c = format(b, row, column);
    let kind = b[CELL_HEAD];
    let count = le::u32(b, CELL_HEAD + 2)? as usize;
    if kind == 0 && count > 0 && b.len() == CELL_HEAD + 6 + VALUE_ROOM + 2 * count {
        c.text = raw::utf16(&b[b.len() - 2 * count..]);
        return Some(c);
    }
    // a formula or a typed number: its stream is the last `count` bytes and ends with 0x0690
    if matches!(kind, 1 | 2) && count >= 2 && b.len() >= CELL_HEAD + 6 + count {
        let stream = &b[b.len() - count..];
        if !stream.ends_with(&[0x90, 0x06]) {
            return None;
        }
        let value = &b[CELL_HEAD + 6..b.len() - count];
        // a number typed in is not shown as a formula
        if kind == 1 {
            c.formula = decompile(stream, row, column);
        }
        match token_value(value) {
            Some(Value::Number(n)) => {
                c.value = Some(n);
                c.text = number_text(n);
            }
            Some(Value::Text(t)) => c.text = t,
            Some(Value::Bool(v)) => c.text = if v { "TRUE" } else { "FALSE" }.into(),
            Some(Value::Error) => c.text = "#ERROR".into(),
            None => {}
        }
        return Some(c);
    }
    None
}

/// Cell formatting: offsets from the start of the block (samples 540, 541).
fn format(b: &[u8], row: u32, column: u32) -> Cell {
    let u8_ = |at: usize| b.get(at).copied().unwrap_or(0) as u32;
    let style = u8_(8);
    let merged = (u8_(17) == 1).then(|| [le::u16(b, 18), le::u16(b, 22), le::u16(b, 20), le::u16(b, 24)].map(|v| v.unwrap_or(0) as u32));
    Cell {
        row,
        column,
        bold: style & 1 != 0,
        italic: style & 2 != 0,
        underline: style & 4 != 0,
        size: le::u16(b, 6).unwrap_or(0) as u32,
        align: match le::u16(b, 2) {
            Some(1) => "left",
            Some(2) => "right",
            Some(3) => "center",
            _ => "general",
        }
        .into(),
        number_format: u8_(10),
        accuracy: u8_(11),
        text_color: le::u16(b, 32).unwrap_or(0) as u32,
        merged,
        ..Cell::default()
    }
}

#[derive(Debug, PartialEq)]
enum Value {
    Number(f64),
    Text(String),
    Bool(bool),
    Error,
}

/// The value token that starts the room of a computed value: `07 x` and an `i32`, `00 x` and an
/// `f64`, `05 x` and a `u32` count with the characters, `01 x` and a `u32` truth, `04 x` an error.
fn token_value(b: &[u8]) -> Option<Value> {
    if !matches!(b.get(1)?, 0 | 2 | 6) {
        return None;
    }
    match b[0] {
        7 => Some(Value::Number(le::u32(b, 2)? as i32 as f64)),
        0 => Some(Value::Number(le::f64(b, 2)?)).filter(|v| matches!(v, Value::Number(n) if n.is_finite())),
        5 => {
            let n = le::u32(b, 2)? as usize;
            (n <= 4096).then(|| b.get(6..6 + 2 * n)).flatten().map(|t| Value::Text(raw::utf16(t)))
        }
        1 => Some(Value::Bool(le::u32(b, 2)? != 0)),
        4 => Some(Value::Error),
        _ => None,
    }
}

fn number_text(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Fills the database rows with the rows the record of computed rows (type 60) holds: `u16`
/// number of rows and of columns at 0x8c, from 0xb0 a table of slots (the row and one cell per
/// column, per row, and one at the end) counting from 0x90. A row is `u16` 3, height, top and
/// 0; a cell is formatted as a worksheet cell and then holds just its value token.
fn attach_subrows(sheet: &mut Worksheet, r: &[u8]) {
    let (Some(rows), Some(columns)) = (le::u16(r, 0x8c), le::u16(r, 0x8e)) else { return };
    let (rows, columns) = (rows as usize, columns as usize);
    let width = columns + 1;
    let Some(table) = slots(r, 0xb0, rows * width + 1) else { return };
    let spans = Spans::new(r, 0x90, &table);
    // the computed rows follow the database rows in the order of the rows; with one database
    // row they all belong to it. Where there are several, the rows of each start where the
    // column-0 slot repeats its row: found by the row number kept in the first cell, which this
    // reader does not know, so they are given to the first database row that has none yet
    let mut subrows = Vec::new();
    for s in 0..rows {
        let mut cells = Vec::new();
        for x in 1..=columns {
            let Some(b) = spans.get(table[s * width + x].1) else { continue };
            if b.len() < CELL_HEAD + 6 {
                continue;
            }
            let mut c = format(b, 0, x as u32);
            match token_value(&b[CELL_HEAD..]) {
                Some(Value::Number(n)) => {
                    c.value = Some(n);
                    c.text = number_text(n);
                }
                Some(Value::Text(t)) if !t.is_empty() => c.text = t,
                Some(Value::Bool(v)) => c.text = if v { "TRUE" } else { "FALSE" }.into(),
                _ => continue,
            }
            cells.push(c);
        }
        subrows.push(cells);
    }
    if let Some(d) = sheet.database_rows.first_mut() {
        for cells in &mut subrows {
            cells.iter_mut().for_each(|c| c.row = d.row);
        }
        d.subrows = subrows;
    }
}

// --- formulas --------------------------------------------------------------------------------

/// Column letters: 1 is A, 27 is AA.
fn column_name(mut n: i64) -> String {
    let mut s = Vec::new();
    while n > 0 {
        s.push(b'A' + ((n - 1) % 26) as u8);
        n = (n - 1) / 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

fn utf16z(b: &[u8], at: usize) -> Option<(String, usize)> {
    let mut end = at;
    while b.get(end..end + 2)? != [0, 0] {
        end += 2;
    }
    Some((raw::utf16(&b[at..end]), end + 2))
}

/// The name of a type in `T=` of a criteria.
fn type_name(n: u16) -> String {
    match n {
        2 => "LINE".into(),
        3 => "RECT".into(),
        4 => "OVAL".into(),
        5 => "POLY".into(),
        6 => "ARC".into(),
        10 => "TEXT".into(),
        11 => "GROUP".into(),
        15 => "SYMBOL".into(),
        21 => "POLYLINE".into(),
        n => n.to_string(),
    }
}

/// Functions written in front of their arguments, which end with the token 0x02a5.
fn call_name(code: u16) -> Option<&'static str> {
    Some(match code {
        0x0293 => "SUM",
        0x028f => "MAX",
        0x0290 => "MIN",
        0x02c7 => "COUNT",
        0x029a => "DATABASE",
        0x01d7 => "LEFT",
        0x01d8 => "RIGHT",
        0x01d9 => "MID",
        0x01d2 => "TRIM",
        0x00f3 => "CONCAT",
        _ => return None,
    })
}

/// Functions written after their one argument.
fn function_name(code: u16) -> Option<&'static str> {
    Some(match code {
        0x00e7 => "SIN",
        0x00e8 => "COS",
        0x0292 => "TAN",
        0x00ec => "ATAN",
        0x00ea => "LN",
        0x028e => "LOG",
        0x00e9 => "EXP",
        0x028d => "INT",
        0x01ce => "LOWER",
        0x01d4 => "UPPER",
        0x01d5 => "LEN",
        0x02aa => "VALUE",
        0x01b1 => "ABS",
        0x00eb => "SQRT",
        _ => return None,
    })
}

/// Properties of the object a row is about, followed by the token 0x02a6.
fn attribute_name(code: u16) -> Option<&'static str> {
    Some(match code {
        0x02e8 => "AREA",
        0x02f3 => "VOLUME",
        0x02ee => "LENGTH",
        0x02c2 => "HEIGHT",
        0x02c1 => "WIDTH",
        0x039c => "NAME",
        0x039a => "CLASS",
        0x0399 => "LAYER",
        _ => return None,
    })
}

fn operator(code: u16) -> Option<&'static str> {
    Some(match code {
        0x064d => "+",
        0x064a => "-",
        0x0654 => "*",
        0x0653 => "/",
        0x0655 => "^",
        0x0644 => ">",
        0x0645 => "<",
        0x0642 => ">=",
        0x0643 => "<=",
        0x0646 => "<>",
        0x0647 => "=",
        0x064f => " & ",
        0x0648 => " | ",
        _ => return None,
    })
}

/// Writes a formula stream out as text, or `None` when it holds a token this reader does not
/// know. A stream is a list of tokens (`u16` code, then what the code needs); operands are
/// written as they come, operators and functions after their operands (calls of variable length
/// in front, closed by 0x02a5), and 0x0690 ends it. `row` and `column` are where the formula is:
/// a reference is stored as its distance from there. (Samples 520 to 560.)
pub fn decompile(s: &[u8], row: u32, column: u32) -> Option<String> {
    let mut stack: Vec<String> = Vec::new();
    // where each open call starts on the stack, and its name
    let mut calls: Vec<(usize, &'static str)> = Vec::new();
    let mut at = 0;
    loop {
        let code = le::u16(s, at)?;
        at += 2;
        match code {
            0x0690 => break,
            // an integer
            0x0295 => {
                stack.push((le::u32(s, at)? as i32).to_string());
                at += 4;
            }
            // a real, 80-bit extended
            0x065a => {
                let v = extended(s.get(at..at + 10)?)?;
                stack.push(number_text(v));
                at += 10;
            }
            // a string literal: u32, characters, 0
            0x065b | 0x02a3 => {
                let (t, end) = utf16z(s, at + 4)?;
                stack.push(format!("'{t}'"));
                at = end;
            }
            // a name that stands for itself
            0x0694 => {
                let (t, end) = utf16z(s, at + 4)?;
                stack.push(t);
                at = end;
            }
            // a cell: row and column, each relative or (0x1000 and the number) absolute
            0x0297 => {
                let (r, c) = (le::u16(s, at)?, le::u16(s, at + 2)?);
                stack.push(cell_name(r, c, row, column));
                at += 4;
            }
            0x0298 => {
                let (t, end) = utf16z(s, at + 4)?;
                stack.push(format!("'{t}':{}", cell_name(le::u16(s, end)?, le::u16(s, end + 2)?, row, column)));
                at = end + 4;
            }
            // a record format and then a field of it
            0x029d => {
                let (t, end) = utf16z(s, at + 4)?;
                stack.push(format!("'{t}'"));
                at = end;
            }
            0x029f => {
                let (t, end) = utf16z(s, at + 4)?;
                let record = stack.pop()?;
                stack.push(format!("{record}.'{t}'"));
                at = end;
            }
            0x0294 => stack.push("PI".into()),
            0x0291 => stack.push("TRUE".into()),
            0x028c => stack.push("FALSE".into()),
            // criteria: the field compared (class, name, layer, type, selection), IN
            0x0306 => stack.push("C".into()),
            0x0303 => stack.push("N".into()),
            0x0305 => stack.push("L".into()),
            0x0313 => stack.push("SEL".into()),
            0x0309 => {
                stack.push("T".into());
                stack.push(type_name(le::u16(s, at)?));
                at += 2;
            }
            0x0661 => stack.push("R".into()),
            0x0641 => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(format!("{a} IN [{b}]"));
            }
            0x0296 | 0x0308 | 0x02a6 => {}
            0x0665 => {
                let a = stack.pop()?;
                stack.push(format!("({a})"));
            }
            0x02a9 => {
                let a = stack.pop()?;
                stack.push(format!("-{a}"));
            }
            0x0656 => {
                let a = stack.pop()?;
                stack.push(format!("NOT {a}"));
            }
            0x0682 => {
                let c = stack.pop()?;
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(format!("IF({a}; {b}; {c})"));
            }
            0x03d9 => stack.push("NOW()".into()),
            0x03d8 => stack.push("TODAY()".into()),
            0x02a5 => {
                let (start, name) = calls.pop()?;
                let args = stack.split_off(start.min(stack.len()));
                stack.push(format!("{name}({})", args.join("; ")));
            }
            c if operator(c).is_some() => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                stack.push(format!("{a}{}{b}", operator(c)?));
            }
            c if call_name(c).is_some() => calls.push((stack.len(), call_name(c)?)),
            c if function_name(c).is_some() => {
                let a = stack.pop()?;
                stack.push(format!("{}({a})", function_name(c)?));
            }
            c if attribute_name(c).is_some() => stack.push(attribute_name(c)?.into()),
            _ => {
                if std::env::var("VWX_WS_DEBUG").is_ok() {
                    eprintln!("UNKNOWN {code:04x}");
                }
                return None;
            }
        }
    }
    (stack.len() == 1 && calls.is_empty() && at == s.len()).then(|| format!("={}", stack[0]))
}

fn cell_name(r: u16, c: u16, row: u32, column: u32) -> String {
    let part = |v: u16, here: u32| -> (bool, i64) {
        if v & 0xf000 == 0x1000 {
            (true, (v & 0xfff) as i64)
        } else {
            (false, here as i64 + v as i16 as i64)
        }
    };
    let (row_abs, y) = part(r, row);
    let (col_abs, x) = part(c, column);
    format!("{}{}{}{}", if col_abs { "$" } else { "" }, column_name(x), if row_abs { "$" } else { "" }, y)
}

/// An 80-bit extended number: `u64` mantissa with its top bit set, `u16` sign and exponent.
fn extended(b: &[u8]) -> Option<f64> {
    let mantissa = le::u64(b, 0)?;
    let se = le::u16(b, 8)?;
    if mantissa == 0 {
        return Some(0.0);
    }
    let exp = (se & 0x7fff) as i32 - 16383 - 63;
    let v = mantissa as f64 * 2f64.powi(exp);
    Some(if se & 0x8000 != 0 { -v } else { v })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    #[test]
    fn writes_out_references_and_operators() {
        // =A1+A2 in row 14, column 3 (sample 530)
        assert_eq!(decompile(&hex("97 02 f3 ff fe ff 97 02 f4 ff fe ff 4d 06 90 06"), 14, 3).as_deref(), Some("=A1+A2"));
        // =(A1+A2)*A3, =$A$1+B2 (row 38)
        assert_eq!(decompile(&hex("97 02 b2 ff fe ff 97 02 b3 ff fe ff 4d 06 65 06 97 02 b4 ff fe ff 54 06 90 06"), 79, 3).as_deref(), Some("=(A1+A2)*A3"));
        assert_eq!(decompile(&hex("97 02 01 10 01 10 97 02 dc ff ff ff 4d 06 90 06"), 38, 3).as_deref(), Some("=$A$1+B2"));
    }

    #[test]
    fn writes_out_literals_and_functions() {
        assert_eq!(decompile(&hex("95 02 07 00 00 00 a9 02 90 06"), 6, 3).as_deref(), Some("=-7"));
        assert_eq!(decompile(&hex("5a 06 00 00 00 00 00 00 00 90 00 40 90 06"), 9, 3).as_deref(), Some("=2.25"));
        assert_eq!(decompile(&hex("93 02 97 02 f9 ff fe ff 97 02 fa ff fe ff a5 02 90 06"), 8, 3).as_deref(), Some("=SUM(A1; A2)"));
        assert_eq!(decompile(&hex("95 02 10 00 00 00 eb 00 90 06"), 27, 3).as_deref(), Some("=SQRT(16)"));
        assert_eq!(decompile(&hex("97 02 f5 ff fe ff 95 02 05 00 00 00 44 06 95 02 01 00 00 00 95 02 02 00 00 00 82 06 90 06"), 12, 3).as_deref(), Some("=IF(A1>5; 1; 2)"));
    }

    #[test]
    fn writes_out_database_criteria_and_fields() {
        // =DATABASE((R IN ['WSRec']))
        let s = "9a 02 96 02 61 06 5b 06 9f 00 00 00 57 00 53 00 52 00 65 00 63 00 00 00 08 03 41 06 65 06 a5 02 90 06";
        assert_eq!(decompile(&hex(s), 2, 0).as_deref(), Some("=DATABASE((R IN ['WSRec']))"));
        // =COUNT(((C='Walls') & (R IN ['WSRec'])))
        let s = "c7 02 06 03 a3 02 ff ff ff ff 57 00 61 00 6c 00 6c 00 73 00 00 00 47 06 65 06 61 06 5b 06 9f 00 00 00 57 00 53 00 52 00 65 00 63 00 00 00 08 03 41 06 65 06 4f 06 65 06 a5 02 90 06";
        assert_eq!(decompile(&hex(s), 46, 3).as_deref(), Some("=COUNT(((C='Walls') & (R IN ['WSRec'])))"));
        // =('WSRec'.'Tag')
        let s = "9d 02 9f 00 00 00 57 00 53 00 52 00 65 00 63 00 00 00 9f 02 04 00 00 00 54 00 61 00 67 00 00 00 65 06 90 06";
        assert_eq!(decompile(&hex(s), 2, 1).as_deref(), Some("=('WSRec'.'Tag')"));
    }

    #[test]
    fn unknown_tokens_give_no_formula() {
        assert_eq!(decompile(&hex("ff 7f 90 06"), 1, 1), None);
        // a stream that never ends
        assert_eq!(decompile(&hex("95 02 07 00 00 00"), 1, 1), None);
    }

    #[test]
    fn reads_value_tokens() {
        assert_eq!(token_value(&hex("07 02 39 30 00 00")), Some(Value::Number(12345.0)));
        assert_eq!(token_value(&hex("00 02 00 00 00 00 00 00 69 40")), Some(Value::Number(200.0)));
        assert_eq!(token_value(&hex("05 02 02 00 00 00 48 00 49 00")), Some(Value::Text("HI".into())));
        assert_eq!(token_value(&hex("01 02 01 00 00 00")), Some(Value::Bool(true)));
        assert_eq!(token_value(&hex("42 02 01 00 00 00")), None);
    }

    /// A worksheet record of two columns and one row, built the way the file lays it out.
    fn sheet(cells: &[(usize, Vec<u8>)]) -> Vec<u8> {
        let rows = 1usize;
        let columns = 2usize;
        let mut head = vec![0u8; TABLE_AT + 8 * (rows + 1) * (columns + 1)];
        head[SIZE_AT..SIZE_AT + 2].copy_from_slice(&(columns as u16).to_le_bytes());
        head[SIZE_AT + 2..SIZE_AT + 4].copy_from_slice(&(rows as u16).to_le_bytes());
        // data: two column records, one row record, then the cells
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for _ in 0..columns {
            offsets.push(head.len() + data.len() - TABLE_BASE);
            data.extend_from_slice(&[1, 0, 120, 0, 0, 0]);
        }
        let row_start = head.len() + data.len() - TABLE_BASE;
        data.extend_from_slice(&[2, 0, 15, 0, 0, 0, 0, 0]);
        let mut cell_offsets = vec![0usize; columns];
        for (c, bytes) in cells {
            cell_offsets[*c] = head.len() + data.len() - TABLE_BASE;
            data.extend_from_slice(bytes);
        }
        let end = head.len() + data.len() - TABLE_BASE;
        let set = |head: &mut Vec<u8>, slot: usize, flag: u32, offset: usize| {
            let at = TABLE_AT + 8 * slot;
            head[at..at + 4].copy_from_slice(&flag.to_le_bytes());
            head[at + 4..at + 8].copy_from_slice(&(offset as u32).to_le_bytes());
        };
        set(&mut head, 0, 0, offsets[0]);
        set(&mut head, 1, 0, offsets[1]);
        set(&mut head, 2, 0, row_start);
        for c in 0..columns {
            set(&mut head, 3 + c, 6, cell_offsets[c]);
        }
        set(&mut head, 5, 0, end);
        head.extend(data);
        head
    }

    fn text_cell(text: &str) -> Vec<u8> {
        let mut b = vec![0u8; CELL_HEAD];
        b[6] = 10;
        b[8] = 1; // bold
        b[2] = 3; // centred
        let units: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&(text.encode_utf16().count() as u32).to_le_bytes());
        b.extend(vec![0u8; VALUE_ROOM]);
        b.extend(units);
        b
    }

    fn formula_cell(value: &str, stream: &str) -> Vec<u8> {
        let mut b = vec![0u8; CELL_HEAD];
        let stream = hex(stream);
        b.extend_from_slice(&[1, 0]);
        b.extend_from_slice(&(stream.len() as u32).to_le_bytes());
        let mut room = hex(value);
        room.resize(VALUE_ROOM.max(room.len()), 0);
        b.extend(room);
        b.extend(stream);
        b
    }

    #[test]
    fn reads_a_worksheet_record() {
        let record = sheet(&[(0, text_cell("Alpha")), (1, formula_cell("07 02 1e 00 00 00", "95 02 1e 00 00 00 90 06"))]);
        let w = worksheet(&record).unwrap();
        assert_eq!((w.rows, w.columns), (1, 2));
        assert_eq!(w.column_widths, vec![120, 120]);
        assert_eq!(w.row_heights, vec![15]);
        assert_eq!(w.cells.len(), 2);
        assert_eq!((w.cells[0].row, w.cells[0].column, w.cells[0].text.as_str()), (1, 1, "Alpha"));
        assert!(w.cells[0].bold && w.cells[0].size == 10 && w.cells[0].align == "center");
        assert_eq!(w.cells[1].text, "30");
        assert_eq!(w.cells[1].value, Some(30.0));
        assert_eq!(w.cells[1].formula.as_deref(), Some("=30"));
    }

    #[test]
    fn a_computed_text_longer_than_the_room_is_read() {
        let text: String = "x".repeat(40);
        let mut value = vec![5, 2];
        value.extend((text.len() as u32).to_le_bytes());
        value.extend(text.encode_utf16().flat_map(|u| u.to_le_bytes()));
        let hexed: Vec<String> = value.iter().map(|b| format!("{b:02x}")).collect();
        let record = sheet(&[(0, formula_cell(&hexed.join(" "), "97 02 00 00 00 00 90 06"))]);
        let w = worksheet(&record).unwrap();
        assert_eq!(w.cells[0].text, text);
    }

    #[test]
    fn empty_cells_are_left_out() {
        let record = sheet(&[]);
        let w = worksheet(&record).unwrap();
        assert!(w.cells.is_empty());
        assert_eq!(w.rows, 1);
    }

    #[test]
    fn extended_numbers() {
        assert_eq!(extended(&hex("00 00 00 00 00 00 00 e0 00 40")), Some(3.5));
        assert_eq!(extended(&hex("00 00 00 00 00 00 00 80 00 40")), Some(2.0));
        assert_eq!(extended(&hex("00 00 00 00 00 00 00 c0 ff 3f")), Some(1.5));
    }
}
