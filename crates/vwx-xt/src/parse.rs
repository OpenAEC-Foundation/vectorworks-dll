//! The nodes of XT neutral binary data.
//!
//! Layout (Annex E of Siemens' public JT v10 file format reference; big-endian):
//!
//! ```text
//! "PS" 0 0 · u16 length + modeller text · u32 length + schema name ·
//! [u16 highest node type, when the schema name ends in a base schema number] · u32 user field size
//! node* · u16 1 · u16 1
//! node = u16 type · [field layout, at the first node of a type, when the schema is embedded]
//!        · [u32 length of the variable field] · index · fields
//! ```
//!
//! An index (and a count in a field layout) is an `i16` holding the value + 1; a negative one is
//! followed by a second `i16` and the value is `second * 32767 - first - 1`.
//!
//! Field layout of a node type that exists in the base schema: `0xff` (as the base), or a count
//! of fields and an edit sequence over the base fields: `C` copy, `D` delete, `I`/`A` insert or
//! append a field (name as a short string, `i16` pointer class, count coded as an index, type as a
//! short string when the pointer class is 0, one more byte when the field is of variable
//! length), `Z` end. A type not in the base schema: a count, name and description (short
//! strings), then the fields in that form.

use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::schema;

/// What went wrong, with the offset in the data.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub at: usize,
    pub what: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Parasolid data at {:#x}: {}", self.at, self.what)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// One field of a node type.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: String,
    /// Type letter of the reference: `u`, `c`, `l` bytes; `n`, `w` shorts; `d` int; `p` index;
    /// `f` double; `i` two, `v` and `h` three, `b` six doubles.
    pub kind: u8,
    /// Number of values; 0 for the variable-length last field.
    pub count: u16,
}

/// A node type as the file lays it out.
#[derive(Debug, Clone, PartialEq)]
pub struct Def {
    pub code: u16,
    pub name: String,
    pub fields: Vec<FieldDef>,
}

/// A field value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Real(f64),
    /// The index of another node; 0 is null.
    Ptr(u32),
    /// A vector, an interval (the third value 0) or the low corner of a box.
    Vector([f64; 3]),
    Many(Vec<Value>),
}

/// A node: its type and the values of its fields, in the order of its definition.
#[derive(Debug, Clone)]
pub struct Node {
    pub def: Rc<Def>,
    pub values: Vec<Value>,
}

impl Node {
    pub fn code(&self) -> u16 {
        self.def.code
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.def.fields.iter().position(|f| f.name == name).map(|k| &self.values[k])
    }

    /// A pointer field; 0 when null or missing.
    pub fn ptr(&self, name: &str) -> u32 {
        match self.get(name) {
            Some(Value::Ptr(p)) => *p,
            _ => 0,
        }
    }

    pub fn real(&self, name: &str) -> Option<f64> {
        match self.get(name) {
            Some(Value::Real(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        match self.get(name) {
            Some(Value::Int(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn vector(&self, name: &str) -> Option<[f64; 3]> {
        match self.get(name) {
            Some(Value::Vector(v)) => Some(*v),
            _ => None,
        }
    }

    /// The values of an array field.
    pub fn many(&self, name: &str) -> &[Value] {
        match self.get(name) {
            Some(Value::Many(v)) => v,
            _ => &[],
        }
    }

    /// True when the `sense` field is `+` (also when the node has none).
    pub fn positive(&self) -> bool {
        self.int("sense") != Some(b'-' as i64)
    }
}

/// Parsed XT data: the nodes by index. Node 1 is the root (a body, usually).
#[derive(Debug, Clone)]
pub struct File {
    /// E.g. `: TRANSMIT FILE created by modeller version 3801180`.
    pub modeller: String,
    /// E.g. `SCH_3801180_38101_13006`.
    pub schema: String,
    pub nodes: HashMap<u32, Node>,
    /// Bytes of the data that the nodes and the terminator took.
    pub len: usize,
}

impl File {
    pub fn node(&self, index: u32) -> Option<&Node> {
        self.nodes.get(&index)
    }

    /// The nodes of type `code`, by index.
    pub fn of_type(&self, code: u16) -> Vec<(u32, &Node)> {
        let mut out: Vec<(u32, &Node)> = self.nodes.iter().filter(|(_, n)| n.code() == code).map(|(i, n)| (*i, n)).collect();
        out.sort_by_key(|(i, _)| *i);
        out
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn fail<T>(&self, what: impl Into<String>) -> Result<T> {
        Err(Error { at: self.at, what: what.into() })
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        match self.data.get(self.at..self.at + n) {
            Some(b) => {
                self.at += n;
                Ok(b)
            }
            None => self.fail("the data ends inside a node"),
        }
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn i16(&mut self) -> Result<i16> {
        let b = self.take(2)?;
        Ok(i16::from_be_bytes([b[0], b[1]]))
    }

    fn i32(&mut self) -> Result<i32> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn index(&mut self) -> Result<u32> {
        let r = self.i16()? as i64;
        let v = if r < 0 { self.i16()? as i64 * 32767 - r - 1 } else { r - 1 };
        match u32::try_from(v) {
            Ok(v) => Ok(v),
            Err(_) => self.fail(format!("index {v}")),
        }
    }

    fn short_string(&mut self) -> Result<String> {
        let n = self.u8()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    fn text(&mut self, len: usize) -> Result<String> {
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }

    /// A field in a layout edit.
    fn field_def(&mut self) -> Result<FieldDef> {
        let name = self.short_string()?;
        let pointer_class = self.i16()?;
        let count = self.index()?;
        let kind = if pointer_class != 0 {
            b'p'
        } else {
            let t = self.short_string()?;
            match t.as_bytes() {
                [k] => *k,
                _ => return self.fail(format!("field type {t:?} of {name}")),
            }
        };
        let count = match count {
            0 => 1,
            1 => {
                self.u8()?; // transmit code of a variable-length field
                0
            }
            n => match u16::try_from(n) {
                Ok(n) => n,
                Err(_) => return self.fail(format!("field {name} with {n} values")),
            },
        };
        Ok(FieldDef { name, kind, count })
    }

    fn one(&mut self, kind: u8) -> Result<Value> {
        Ok(match kind {
            b'u' | b'c' | b'l' => Value::Int(self.u8()? as i64),
            b'n' | b'w' => Value::Int(self.i16()? as i64),
            b'd' => Value::Int(self.i32()? as i64),
            b'p' => Value::Ptr(self.index()?),
            b'f' => Value::Real(self.f64()?),
            b'i' => Value::Vector([self.f64()?, self.f64()?, 0.0]),
            b'v' | b'h' => Value::Vector([self.f64()?, self.f64()?, self.f64()?]),
            b'b' => {
                let low = [self.f64()?, self.f64()?, self.f64()?];
                self.take(24)?;
                Value::Vector(low)
            }
            other => return self.fail(format!("field type {:?}", other as char)),
        })
    }
}

/// The layout of node type `code` as the file gives it at the first node of that type.
fn layout(r: &mut Reader, code: u16, embedded: bool) -> Result<Def> {
    let base = schema::base(code);
    let base_fields = |fields: &[schema::Field]| fields.iter().map(|&(name, kind, count)| FieldDef { name: name.into(), kind, count }).collect::<Vec<_>>();
    if !embedded {
        return match base {
            Some((name, fields)) => Ok(Def { code, name: name.into(), fields: base_fields(fields) }),
            None => r.fail(format!("node type {code} is not in the base schema")),
        };
    }
    let Some((name, base)) = base else {
        let count = r.u8()?;
        let name = r.short_string()?;
        let _description = r.short_string()?;
        let fields = (0..count).map(|_| r.field_def()).collect::<Result<Vec<_>>>()?;
        return Ok(Def { code, name, fields });
    };
    let count = r.u8()?;
    if count == 0xff {
        return Ok(Def { code, name: name.into(), fields: base_fields(base) });
    }
    let base = base_fields(base);
    let mut fields = Vec::with_capacity(count as usize);
    let mut next = 0;
    loop {
        match r.u8()? {
            b'C' => match base.get(next) {
                Some(f) => {
                    fields.push(f.clone());
                    next += 1;
                }
                None => return r.fail(format!("{name}: the file copies more fields than base schema 13006 has ({})", base.len())),
            },
            b'D' => next += 1,
            b'I' | b'A' => fields.push(r.field_def()?),
            b'Z' => break,
            op => return r.fail(format!("{name}: layout edit {:?}", op as char)),
        }
    }
    if fields.len() != count as usize {
        return r.fail(format!("{name}: {} fields after the edit, the file says {count}", fields.len()));
    }
    Ok(Def { code, name: name.into(), fields })
}

/// Parses XT neutral binary data; data after the terminator is left alone.
pub fn parse(data: &[u8]) -> Result<File> {
    let mut r = Reader { data, at: 0 };
    if r.take(4)? != b"PS\0\0" {
        r.at = 0;
        return r.fail("not XT neutral binary (no PS flag)");
    }
    let n = r.i16()?;
    let modeller = r.text(n.max(0) as usize)?;
    let n = r.i32()?;
    let schema_name = r.text(n.max(0) as usize)?;
    // SCH_<modeller>_<schema>_<base schema>: the layouts are given against the base schema
    let embedded = schema_name.split('_').count() == 4;
    if embedded {
        r.i16()?; // highest node type
    }
    r.i32()?; // user field size: 0 in the files seen
    let mut defs: HashMap<u16, Rc<Def>> = HashMap::new();
    let mut nodes = HashMap::new();
    loop {
        let code = r.i16()?;
        if code == 1 {
            r.i16()?;
            break;
        }
        let Ok(code) = u16::try_from(code) else { return r.fail(format!("node type {code}")) };
        let def = match defs.get(&code) {
            Some(d) => d.clone(),
            None => {
                let d = Rc::new(layout(&mut r, code, embedded)?);
                defs.insert(code, d.clone());
                d
            }
        };
        let variable = match def.fields.last() {
            Some(f) if f.count == 0 => {
                let n = r.i32()?;
                if n < 0 || n as usize > data.len() {
                    return r.fail(format!("{}: variable length {n}", def.name));
                }
                n as usize
            }
            _ => 0,
        };
        let index = r.index()?;
        let mut values = Vec::with_capacity(def.fields.len());
        for f in &def.fields {
            values.push(match f.count {
                1 => r.one(f.kind)?,
                0 => Value::Many((0..variable).map(|_| r.one(f.kind)).collect::<Result<_>>()?),
                n => Value::Many((0..n).map(|_| r.one(f.kind)).collect::<Result<_>>()?),
            });
        }
        nodes.insert(index, Node { def, values });
    }
    Ok(File { modeller, schema: schema_name, nodes, len: r.at })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Writes XT neutral binary for tests.
    #[derive(Default)]
    pub(crate) struct Writer(pub Vec<u8>);

    impl Writer {
        pub(crate) fn header(schema: &str) -> Writer {
            let mut w = Writer(b"PS\0\0".to_vec());
            let modeller = ": TRANSMIT FILE created by modeller version 3801180";
            w.0.extend((modeller.len() as u16).to_be_bytes());
            w.0.extend(modeller.bytes());
            w.0.extend((schema.len() as u32).to_be_bytes());
            w.0.extend(schema.bytes());
            if schema.split('_').count() == 4 {
                w.0.extend(249u16.to_be_bytes());
            }
            w.0.extend(0u32.to_be_bytes());
            w
        }
        pub(crate) fn u8(&mut self, v: u8) -> &mut Self {
            self.0.push(v);
            self
        }
        pub(crate) fn i16(&mut self, v: i16) -> &mut Self {
            self.0.extend(v.to_be_bytes());
            self
        }
        pub(crate) fn i32(&mut self, v: i32) -> &mut Self {
            self.0.extend(v.to_be_bytes());
            self
        }
        pub(crate) fn ptr(&mut self, v: u32) -> &mut Self {
            self.i16(v as i16 + 1)
        }
        pub(crate) fn vec(&mut self, v: [f64; 3]) -> &mut Self {
            v.iter().for_each(|c| self.0.extend(c.to_be_bytes()));
            self
        }
        pub(crate) fn text(&mut self, s: &str) -> &mut Self {
            self.0.push(s.len() as u8);
            self.0.extend(s.bytes());
            self
        }
        pub(crate) fn end(&mut self) -> Vec<u8> {
            self.i16(1).i16(1);
            self.0.clone()
        }
    }

    #[test]
    fn reads_a_point_with_the_base_layout() {
        let mut w = Writer::header("SCH_3801180_38101_13006");
        // POINT (29), layout as the base: node_id, attributes_groups, owner, next, previous, pvec
        w.i16(29).u8(0xff).ptr(8).i32(422).ptr(0).ptr(11).ptr(18).ptr(0).vec([0.0, 0.0075, 0.3]);
        // a second node of the type has no layout
        w.i16(29).ptr(18).i32(419).ptr(0).ptr(23).ptr(39).ptr(8).vec([0.5, 0.0075, 0.3]);
        let file = parse(&w.end()).unwrap();
        assert_eq!(file.schema, "SCH_3801180_38101_13006");
        let p = file.node(8).unwrap();
        assert_eq!((p.def.name.as_str(), p.int("node_id"), p.ptr("owner"), p.ptr("previous")), ("POINT", Some(422), 11, 0));
        assert_eq!(file.node(18).unwrap().vector("pvec"), Some([0.5, 0.0075, 0.3]));
        assert_eq!(file.of_type(29).len(), 2);
    }

    #[test]
    fn applies_a_layout_edit_as_sample_60_does_for_lists() {
        // LIST: copy node_id, insert list_type and notransmit, copy owner next previous, delete,
        // copy list_length block_length, delete, insert finger_index finger_block, copy list_block
        let mut w = Writer::header("SCH_3801180_38101_13006");
        w.i16(70).u8(11).u8(b'C');
        w.u8(b'I').text("list_type").i16(0).ptr(0).text("u");
        w.u8(b'I').text("notransmit").i16(0).ptr(0).text("l");
        for op in b"CCCDCCD" {
            w.u8(*op);
        }
        w.u8(b'I').text("finger_index").i16(0).ptr(0).text("d");
        w.u8(b'I').text("finger_block").i16(1012).ptr(0);
        w.u8(b'C').u8(b'Z');
        w.ptr(2).i32(0).u8(4).u8(1).ptr(1).ptr(0).ptr(0).i32(2).i32(20).i32(1).ptr(12).ptr(12);
        let file = parse(&w.end()).unwrap();
        let list = file.node(2).unwrap();
        let names: Vec<&str> = list.def.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["node_id", "list_type", "notransmit", "owner", "next", "previous", "list_length", "block_length", "finger_index", "finger_block", "list_block"]);
        assert_eq!((list.int("list_length"), list.int("block_length"), list.ptr("list_block")), (Some(2), Some(20), 12));
    }

    #[test]
    fn reads_variable_length_nodes_and_arrays_in_a_layout() {
        let mut w = Writer::header("SCH_3801180_38101_13006");
        // INT_VALUES (82): variable field of 3
        w.i16(82).u8(0xff).i32(3).ptr(5).i32(7).i32(8).i32(9);
        // ATTRIB_DEF (80): legal_owners replaced by an array of 17
        w.i16(80).u8(7);
        for op in b"CCCCCD" {
            w.u8(*op);
        }
        w.u8(b'I').text("legal_owners").i16(0).ptr(17).text("l").u8(b'C').u8(b'Z');
        w.i32(1).ptr(6).ptr(0).ptr(4).i32(9000);
        (0..8).for_each(|_| {
            w.u8(0);
        });
        w.ptr(0);
        (0..17).for_each(|k| {
            w.u8(u8::from(k == 4));
        });
        w.u8(1);
        let file = parse(&w.end()).unwrap();
        assert_eq!(file.node(5).unwrap().many("values"), [Value::Int(7), Value::Int(8), Value::Int(9)]);
        let def = file.node(6).unwrap();
        assert_eq!(def.int("type_id"), Some(9000));
        assert_eq!(def.many("legal_owners").len(), 17);
        assert_eq!(def.many("fields"), [Value::Int(1)]);
    }

    #[test]
    fn large_indexes_take_two_shorts() {
        let mut r = Reader { data: &[0xff, 0xfe, 0x00, 0x02], at: 0 };
        // first -2, second 2: 2 * 32767 + 2 - 1
        assert_eq!(r.index().unwrap(), 65535);
    }

    #[test]
    fn damaged_data_is_an_error_with_its_offset() {
        assert!(parse(b"XT").is_err());
        let mut w = Writer::header("SCH_3801180_38101_13006");
        w.i16(29).u8(3).u8(b'Q');
        let e = parse(&w.0).unwrap_err();
        assert!(e.what.contains("POINT"), "{e}");
    }
}
