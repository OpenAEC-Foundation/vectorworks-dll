# VWX format notes

What is known about the file format, each finding with what shows it. Only from our own
analysis of files Vectorworks wrote (see the clean-room rules in the README), plus two public
sources that are named where they are used: the PRONOM file signatures of the UK National
Archives, and the Vectorworks scripting reference (object type numbers).

Notation: offsets in hex; `u32be`, `u64le` for integers; `f64` for IEEE doubles (little-endian);
"corpus" is the 787 `.vwx` and `.sta` files a Vectorworks 2027 installation ships with (786 saved
by VW 32.x = 2027, one by VW 28.0.4 = 2023; first made with VW 12.0 to 32.0).

## Overview

```text
file      = header (0x94 bytes) · document header · block*
block     = u32le length · snappy block of that length
body      = the blocks unpacked and joined (a block holds at most 1 MiB unpacked)
body      = record*
record    = u64le length (itself included) · record header to 0x50 · payload
```

A `.sta` file (template) is the same format as a `.vwx`. Checked: all 787 corpus files unpack
with no byte left over and their bodies split into records up to the last byte (2 200 946
objects besides user data).

## Header (0x94 bytes, big-endian)

| Offset | Type | Content | Seen |
|---|---|---|---|
| 0x00 | `u32be` | length of the document header | `0x9e0` in all corpus files |
| 0x04 | `u32be` | 0 | |
| 0x08 | `u32be` | length of this header | `0x94` |
| 0x0c | `u16be` | `0x0020` | |
| 0x0e | text | `VW<created> VW<saved>`, NUL-terminated | `VW24.0.0 VW32.0.0` |
| 0x20 | text | a product/licence code, NUL-terminated | |
| 0x3a | text | `VectorWorks` | |
| 0x50 | `u32be` | build number of the saving application | 882075 |
| 0x54 | `u32be` | file size | equals the size in the file checked |
| 0x58 | `u32be` | ? | `0x1160` |
| 0x5c | `u32be` | ? | `0x915b` |
| 0x62 | text | `BUILD: <number>` | |

The first version is the one the document was first made with (corpus: 12.0.0 to 32.0.0), the
second the one that saved it. Version 13 is Vectorworks 2008; year = major + 1995.

Older files (public PRONOM signatures fmt/1139, 1140, 450, 451, 686; not checked on samples):
VW 12.5, 2009 and 2010 have the same first 14 bytes with document header length `0x858` (or
`0x7f2`), then `VW12.5`/`VW14.0`/`VW15.0` twice and `VectorWorks`; VW 9 and 10 have
`VectorWorks 9`/`VectorWorks 10` at 0x0e. Whether their body is compressed is not known.

## Document header (0x9e0 bytes, little-endian)

Settings of the document, uncompressed. Seen in it: its own length as `u32le` at 4, pointers
(see "Addresses"), UTF-16LE unit names (`Feet & Inches`, ` sq ft`, `Cubic Feet`), `f64` values,
the renderer name `QuickDraw 3D Interactive`. Fields not assigned yet; the units are the first
to find (ladder sample 00 against a document in other units).

## Body

At `header length + document header length`: blocks, each a `u32le` length and a raw Snappy
block (public format: varint uncompressed length, then literals and copies). Every block but the
last unpacks to exactly 1 MiB; the joined output is one stream, records run across block
borders.

## Records

Header (0x50 bytes, little-endian):

| Offset | Type | Content |
|---|---|---|
| 0x00 | `u64le` | length of the record, this field included |
| 0x08 | `u64le` | `0x05792ca4` or 0 (meaning not known) |
| 0x10 | `u64le` | an address or 0 (not known: differs per object) |
| 0x18 | `u8` | 0 |
| 0x19 | `u8` | object type |
| 0x1f | `u8` | flags, `0x40` or `0x41` |
| 0x26 | `u64le` | address of the parent object |
| 0x2e | `u16` | `0x0011` objects, `0x0012`/`0x0002` user data and resources (not settled) |
| 0x38 | 16 bytes | UUID of the object (zero in some records) |
| 0x50 | | payload, by object type |

**Object type** is the number the scripting API gives (`GetTypeN`; public table, Function
Reference Appendix D): 2 line, 3 rectangle, 5 polygon, 6 arc, 10 text, 11 group, 15 symbol,
16 symbol definition, 21 polyline, 24 extrude, 25 3D polygon, 31 layer, 34 sweep, 38 multiple
extrude, 40 mesh, 41 mesh vertex, 47 record format, 48 record, 68 wall, 71 slab, 84 CSG solid,
86 plug-in object, 94 class, 95 solid, 111 NURBS curve, 113 NURBS surface, 122 viewport,
127 wall style. Types in the files that the table does not list: 0, 26–28, 33, 42–44, 54, 55,
57, 59, 62, 65, 69, 70, 74, 76–80, 88, 91, 100, 101, 110, 112, 114, 116, 123–125.

- **Type 0** (88 bytes) ends the children of a container (layer, group, symbol definition,
  plug-in object).
- **Type 76** is user data: its payload starts with a four-letter tag stored reversed
  (`TDMD` = `DMDT`, `NCSW`, `SGAT`, ...). It is the most common record (about half of all).

### Addresses and the tree

Objects refer to each other with the 64-bit memory addresses they had when the file was saved
(values like `0x000001c8b0760af0`). No record states its own address. The records are the object
tree in depth-first order, so the tree follows from the parent addresses at 0x26:

- a record whose parent address has not been seen in the open chain of parents is a child of the
  record before it;
- a record whose parent address is in the chain goes back to that parent.

A container holds the address of its first child at 0x8c (seen in groups, symbol definitions
and plug-in objects). User data records follow the object they belong to as its children.

## Coordinates

`f64`, millimetres, whatever the document units (seen: 0.79375 = 1/32 inch, 39.6875 in an
imperial library file).

## Objects

Payloads start alike for drawn objects: 24 bytes of attributes at 0x50, then at 0x68 four `f64`
(the bounding box). A line is 140 bytes, a rectangle 188, an extrude 324, a group 204, a symbol
definition 372. The fields are decoded from the sample ladder (`docs/ORACLE.md`); nothing is
settled yet.
