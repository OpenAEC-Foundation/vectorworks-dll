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
| 0x1a | `u32le` | reference number of the object, 0 for none (pair 02/05: a named line gets one; layers, classes, symbol definitions and record formats always have one) |
| 0x1f | `u8` | flags, `0x40` or `0x41` |
| 0x20 | `u32le` | reference number of the class of the object (pair 02/40) |
| 0x24 | `u8` | what the object takes from its class: bit 0x08 the line weight, 0x10 the pen colour, 0x20 the pen, 0x40 the fill colour, 0x80 the fill (pairs 230 to 235; an object made with all its attributes by class has 0xff here and in the next byte) |
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

## Document tables

Document-level records before the layers (their parent is the document).

**Reference table (type 33).** `u32le` size at 0x50, then per reference number 12 bytes from
0x54: the `u64le` address of the object and a `u32` (0 so far). Number 1 is the first layer.

**Name list (type 55).** A 12-byte head (entry count as `u32le` at 0x58), then entries of
`u32le` reference number, `u32le` length in bytes, UTF-16LE text (the list of a new document
starts with an entry for number 0 without a name). Holds the names of classes,
symbol definitions, record formats and named objects (pairs 02/05, 02/40: `L1`, `Muren`), not of
layers.

**Classes (type 94).** One record per class; its reference number at 0x1a is the number objects
name at 0x20, its name is in the name list. The Dutch default class is `Geen` (number 2).
Its visibility is the byte at 0xb6: 0 visible, 0xff invisible, 2 greyed (samples 130, 131, 132;
the values the scripting interface gives). Its attributes: palette indexes as `u16`
of the fill at 0x54 and of the pen at 0x56, the pen as `i32` at 0x5a, the line weight as `u16`
at 0x5e, the fill as `i32` at 0x60 (as in objects); bit 0x80 of the byte at 0x50 says objects take the class attributes
when made (samples 132, 230).

**Layers (type 31).** Reference number at 0x1a, elevation `f64` at 0x20e (pair 90/95), its texts,
one after the other from 0x2a2, each a `u32le` length in bytes and UTF-16LE: its name, its title
(a sheet layer of `Residential New Construction.sta`: `210`, `Plans`) and the level it belongs
to (a design layer: `Top of Slab`, `Finish Floor`; empty in a new document). The byte at 0x24c is 1 for a
design layer and 2 for a sheet layer (the numbers the scripting interface gives). The `i16` at
0xd8 is the visibility of the layer: 0 visible, -1 invisible, 2 greyed (samples 150, 151, 152). The children of the
record are the objects on the layer.

**Colour palette.** The `TDMD` record with key `DCDN` (see below): fields 2, 3, 4, 5 are cyan,
magenta, yellow and black of each colour as `f32` 0..1, field 1 the number of colours in use.
RGB = (1 - ink)(1 - black). Colours 0..=1238 are the same in every document (256 white, 257
black, 1237 red, 1238 blue); colours a document adds follow (sample 100: 1239). Checked against
the scripting API for all 1239 and for an added colour.

## User data (type 76) and `TDMD`

A user data record follows the object it belongs to, as its child. Its payload starts with a
tag. Most have the tag `TDMD`:

```text
"TDMD" · u16 version (5) · key, 4 letters stored reversed · u16 field count · u32 size of the rest
field count x (u16 field number · u32 offset, counted from the end of this 16-byte head)
per field: u16 type · u32 count · values
```

Types seen: 1 (one byte per value), 6 (`f64`), 8 (four bytes per value: `f32` or `u32`). Keys
seen at document level: `dprv`, `DPDN`, `shsm`, `DCDN` (palette); on drawn objects `OPhI` and
others, not decoded.

## Coordinates

`f64`, millimetres, whatever the document units. 2D points are stored as (y, x); 3D points as
(x, y, z). 3D objects hold their height in the document: an extrude made on a layer with
elevation 3000 has 3000 in its own placement (sample 105). 2D objects lie on their layer.

A placement is nine `f64` (the images of the local x, y and z axes, in that order) followed by
the offset: `world = x·row0 + y·row1 + z·row2 + offset`.

## Objects

Every drawn object starts alike:

| Offset | Type | Content |
|---|---|---|
| 0x50 | `i32` | the pen: 0 none, 2 a solid line, minus the reference number of a line type (type 96) for a dashed one |
| 0x54 | `u16` | line weight in thousandths of an inch (4 by default; pair 02/104: 40) |
| 0x56 | `u16` | palette index, 257 by default (fill foreground, assumed) |
| 0x58 | `u16` | palette index of the fill (pair 10/41: 256 white to 1237 red) |
| 0x5a | `u16` | palette index of the pen (pair 02/42: 257 black to 1238 blue) |
| 0x5c | `u16` | palette index, 256 by default (pen background, assumed) |
| 0x5e | 2 x `u8` | opacity of fill and pen in percent (sample 103: 40, 40) |
| 0x64 | `i32` | the fill: 0 none (sample 102), 1 solid, more for a pattern, minus the reference number of a hatch (type 66), gradient or image (sample 260) |
| 0x68 | 4 x `f64` | bounding box in plan: top, left, bottom, right |

| Type | Size | Payload after the common part |
|---|---|---|
| 2 line | 140 | instead of the bounding box: y1, x1, y2, x2 (samples 02, 03, 04, 07) |
| 17 locus | 120 | y, x at 0x68 (sample 18) |
| 3 rectangle, 4 oval | 188 | origin (y, x) at 0x8c; direction (y, x) at 0x9c; width along the direction at 0xac; height to its left at 0xb4. An oval is the ellipse in that frame (samples 10, 11, 12, 17) |
| 5 polygon | 0x90 + 16n | the byte at 0x8d is 1 when its last point is joined to its first (pair 190/191); `u16` count at 0x8e; points (y, x) from 0x90 (samples 13, 14) |
| 10 text | | the box around it at 0x68 (top, left, bottom, right); `u16` count of characters at 0xa6; `u16` count of lines at 0x100, two more `u16`, and from 0x106 a `u16` per line (the number of its first character); then the size of the letters in points as an `f64`; the characters as UTF-16 at the end of the record, a carriage return between lines (samples 160 to 164) |
| 6 arc | 210 | start point at 0x90, end point at 0xa0, centre at 0xb0 (each y, x); at 0xc0 an angle a and at 0xc8 the sweep, in degrees: the arc runs from 90 - (a + sweep) to 90 - a, counter-clockwise from the x axis (samples 15, 16) |
| 25 3D polygon | 0xd2 + 24n | six `f64` at 0x9c (largest then smallest x, y, z); `u16` count at 0xd0; points from 0xd2 (sample 30) |
| 24 extrude | 324 | addresses of its first and last child at 0x8c, 0x94; local box at 0x9c (largest then smallest x, y, z); placement at 0xcc; bottom and top along local z at 0x130, 0x138. The children are the profile, 2D objects in the local xy plane (samples 20 to 27) |
| 40 mesh | 0xd4 + 28n | `u32` point count at 0xd0; from 0xd4 per point x, y, z and a `u32` (`0x100`). Its children of type 41 are the faces: `u16` count at 0xcc, then the numbers of its points as `u32` from 0xce, counted from 1 (sample 120) |
| 15 symbol | 280 | `u32` reference number of the definition at 0x8e; placement at 0x96; scale factors along the local axes at 0x100 (samples 50, 51) |
| 16 symbol definition | 372 | its children are the content; found by its reference number |
| 11 group | 204 | its children |
| 84 CSG solid | 386 | children: a type 114 record with the result and the operands |
| 34 sweep | 356 | as the extrude up to 0x138; pitch (the rise along the axis in one turn) at 0x140; step, start and arc in degrees at 0x148, 0x150, 0x158. The profile, drawn in the local xy plane, turns about the line along local y through its smallest x: at angle a a profile point (x, y) is at (axis + (x - axis) sin a, y + pitch (a - start) / 360, (x - axis) cos a) (samples 29, 200 to 203) |
| 38 multiple extrude | | placement at 0xcc as the extrude; children: its profiles and a type 42 record with one ring of points per profile (samples 28, 222) |
| 42 profile points | 0x72 + 24n | a child of a profile shape (or of a multiple extrude): the points Vectorworks worked the profile out to, curves already stepped. `u16` points per ring at 0x6c, `u16` rings at 0x6e, `u16` total at 0x70; x, y, z from 0x72, for each point of the profile one per ring. An extrude has two rings (bottom, top), a multiple extrude one per profile, a sweep every turned position of its profile |
| 95 solid | | a primitive (sphere, hemisphere, cone): placement at 0x94, box in the document at 0xf4. Its type 85 child is the mesh Vectorworks keeps for it (samples 31 to 33, 300 to 303) |
| 85 mesh of a solid | | four `u32` at 0x8c: points, corner numbers, faces, edges; the points (x, y, z) from 0x9c, in the coordinates the solid is placed in; the corner numbers as `u32`, counted from 1; per face a `u32` count of corners and the `u32` position of its first corner number; per edge 16 bytes |
| 113 NURBS surface | 0xb0 + 32 nu nv + 8 (ku + kv) | six `u32` at 0x98: control points along u and along v, knots of u and of v, degree along u and along v; from 0xb0 the control points, four `f64` each (x w, y w, z w, w), u running slowest; then the knots of u and those of v, written out in full. In the coordinates the object is placed in (the layer elevation is in the points). Its type 111 children are its boundaries (sample 122) |
| 111 NURBS curve | | as a child of a NURBS surface: one boundary of it, in the parameters (u, v) of the surface. `u32` count of pieces at 0x8c; from 0x90 piece after piece around the boundary, each: `u32` control points, `u32` knots, `u32` degree, a `u32`; the control points, four `f64` each (u w, v w, 0, w); the knots (sample 122: four straight pieces around the unit square) |
| 21 polyline | 0x90 + 36n | `u16` count at 0x8e; vertices of 36 bytes from 0x90: y, x, a `u32` whose first byte is 0x80 when the side to the next vertex is drawn (a polyline with a hole runs to the hole and back along sides that are not) and whose second byte is the kind of vertex (0 corner, 1 Bezier, 2 cubic, 3 arc, 4 a point on an arc), an `f64` radius, 8 bytes. A Bezier vertex is the control point of a quadratic curve, an arc vertex a corner rounded by a circle of that radius (0: as large as fits); a curve runs from and to a neighbouring corner, and half way to a neighbour that is a curve itself. As a profile its type 42 child has Vectorworks' own points of it (samples 111, 113) |
| 13 rounded rectangle | 204 | as the rectangle, then the width and height of the ellipse its corners are quarters of at 0xbc and 0xc4 (samples 112, 220) |
| 68 wall | 2274+ | children: plan graphics (lines, polygons), a record, and a CSG solid with the wall's body |

### Records: types 47 and 48

A **record format** (type 47) has its name in the name list under its reference number. A `u32`
count of fields at 0x50; from 0x60 a block of 136 bytes per field: `u16` kind, `u16` offset of
the field's value in a record, 4 bytes, and the name as UTF-16 ending in a zero in 128 bytes
(what follows the zero is whatever was in memory). Kinds by the room their value takes, found
from the offsets of 152 formats: 1 integer (4 bytes), 2 boolean (2), 4 text (8); 3, 7, 10, 11,
20, 21, 22 numbers (10: an `f64` and two more bytes); 8, 9, 14, 18, 19 take 8 bytes as text does
(lists to choose from); 23 to 26, 29, 30 take 4, 28 takes 2, 27 takes 1.

A **record** (type 48) is a child of the object it is attached to: `u32` reference number of
its format at 0x50, a `u16` count of bytes to pass over at 0x54 (none, or one per field), then
the fixed part with each value at its field's offset, then the texts. A text value is a `u32`
length in bytes and a `u32` place counted from the start of the fixed part; a choice holds the
chosen text first, then the others, zeros between (sample 90: format `Gegevens`, field `Naam`,
value `Balk-1`; 1033 of the 1083 records of `Residential New Construction.sta` have exactly the
length this gives).

Vectorworks keeps the **IFC data** of an object in such records: `NNA_IfcDoor`, `NNA_TIfcDoor`
or `NNA_4IfcDoor` (IFC 4) for the entity, `NNA_Pset_...` for property sets; their fields come in
pairs `prm0_name`, `prm0_value` (`GlobalId`, `3UrNonhr...`).

A **plug-in object** (type 86: door, window, slab, space, stair, title block, ..) holds its
settings in a record too, the last of its records, whose format has the name of the plug-in
(`Door`, `Window`, `Slab`, `Space`, `Stairs CW`): that name says what the object is. A door or
window in a wall is a child of the wall. What a plug-in object holds (its 3D parts and its 2D
shapes) is in its own coordinates: its placement is the matrix at 0xf0 with the offset at 0x138
(a window of the corpus: offset (2000, 8050, 0), its extrude at (0, 40, 805)).

### Viewports: type 122

A viewport is a child of its sheet layer, named in the name list. The box it takes on the sheet
at 0x68 (top, left, bottom, right); a placement at 0x8c whose offset at 0xd4 is where the origin
of the model lies, in the millimetres of the model (divided by the scale: on the sheet); at
0x23e nine `f64`, where the x, y and z of the model point in the view (x right, y up, z towards
the eye: the other way round from the type 43 record of a saved view); the scale as an `f64`
at 0x2e4 (pair 200/201: 50 and 100; samples 203, 204 for the view; the eight viewports of
`Residential New Construction.sta`: two plans, two sections, four elevations, all 1 : 50). A
viewport made by a script and never drawn has a box without a size.

The record ends with what the viewport shows: `u16` counts of layers and of classes at 0x356 and
0x358, then 8 bytes for each, the layers first: `u32` reference number, `i16` visibility (0
visible, -1 invisible, 2 greyed), a `u16` (pairs 240 to 245; `Section-1` of the corpus project
names its 9 design layers and 25 classes, the classes of spaces and fittings invisible). A new
viewport names none and shows nothing.

Among the children of a viewport are groups, each starting with a user data record that tags
it: its crop (`porc`, a 2D shape on the sheet; the box of the viewport is that of its crop),
its annotations, and what it keeps of the drawing it shows, as shapes in the millimetres of
the view: a section viewport the outlines of what it cuts (`hcsc`, polygons and polylines with
the records of the objects cut), the lines beyond the cut (`ncsp`) and the outline of the
whole (`hcac`); an elevation the outline of what it shows. A point of the view lies on the
sheet at the origin of the viewport plus the point divided by the scale. Where the cut of a
section viewport lies in the model is not decoded.

### Dimensions: type 63

A dimension (848 bytes; its length as an `f64` at 0xb6, its offset at 0xbe) holds what it is
drawn with as its children, in the coordinates it lies in: lines (witness lines, the dimension
line, ticks) and a group with its text (samples 210 to 212; 317 of the 318 dimensions of two
corpus projects have their lines inside their own box).

### Saved views: types 49 and 43

A saved view is a type 49 record (89 bytes), a child of the type 51 record that holds them all;
its name is in the name list under its reference number. Its type 43 child (380 bytes) is the
way it looks at the model: at 0x9c nine `f64`, the directions in the document of right on the
screen, up on the screen and towards the eye, and three more, a point (sample 140: the view set
to 60 and 45 degrees has (0.707, 0.707, 0), (-0.354, 0.354, 0.866), (0.612, -0.612, 0.5); a top
view the unit matrix). A layer has a type 43 child of its own, the view it was last seen in.
The other children of a saved view (type 124, user data `svdn` and `ccst`) are not decoded.

### Solids: type 114 and Parasolid

A type 114 record is the cached solid body of its parent: of a CSG solid (84), an extrude (24),
a sweep (34), a multiple extrude (38), a NURBS surface (113). The body of a wall is that of the
CSG solid among the wall's children. After 10 bytes at 0x50: `u16`, `u32` total size, then 15
slots of (`u16` section number or `0xffff`, `u32` offset counted from 0x60).

- section 0: points, three `f64` each, in millimetres;
- section 13: polylines through them, 6 bytes each (`u16` point count, `u32` first point): the
  edges of the solid (wall of sample 60: 24 points, 12 edges of a box 5000 x 150 x 3000);
- section 19: a 6-byte head and a **Parasolid** transmit file, neutral binary, starting
  `PS`, 0, 0, `: TRANSMIT FILE created by modeller version 3801180`, schema
  `SCH_3801180_38101_13006`. The solid itself, read by the `vwx-xt` crate.

**Unit.** One unit of the Parasolid data is 10 000 mm (sample 60: the wall of 5000 mm is 0.5).

**Coordinates.** The body of an extrude and of a sweep is in the object's own coordinates (its
placement at 0xcc applies); the body of a CSG solid and of a multiple extrude is in the
coordinates the object is placed in (their placement is already in it). The reader tells them
apart per object: a body that fills the object's own box at 0x9c is in the object's coordinates.
Found on the corpus (`Residential New Construction.sta`), checked on samples 34, 35, 60 to 65
against Vectorworks' STL export: same bounds and the same number of triangles.

**Parasolid XT.** A public format: Annex E ("XT B-Rep data segment") of Siemens' "JT File Format
Reference Version 10.0". Big-endian; a header, then nodes (type, index, fields), each a piece of
topology (body, shell, face, loop, fin, edge, vertex) or geometry (plane, cylinder, B-surface,
line, circle, B-curve, ...). The field layout of each node type is the base schema 13006 with an
edit sequence the file gives at the first node of the type; the reference prints later structs
for `LIST` and `POINTER_LIS_BLOCK`, whose 13006 layouts were found from those edit sequences
(`tools/gen_xt_schema.py`). The corpus holds 67 788 bodies in 281 files, written by 65 versions
of the modeller (schemas 19008 to 38101); 67 714 parse.

**Faces.** A face is a part of a surface inside loops of edges. Planes are triangulated in
their plane; every other face in the parameters (u, v) of its surface (a constrained Delaunay
triangulation of the loops, with points inside where the surface bends both ways), so that
neighbouring faces share the points of their common edge. Surfaces that are evaluated: plane,
cylinder, cone, sphere, torus, B-surface (NURBS), swept surface (a curve moved along a
direction) and spun surface (a curve turned about an axis). Blend and offset surfaces are not:
their faces are filled flat between their edges and counted as approximate.

Of the 641 490 faces in the corpus (plane 84 %, cylinder 7 %, B-surface 3 %, swept 1 %, the
others under 1 % each) 4 851 are filled flat: 2 194 on blend and 199 on offset surfaces, the
others faces whose loops could not be laid out in the parameters of their surface (a loop that
seems to go around more than once, two loops around the axis the same way); 132 give no
triangles. `Residential New Construction.sta`: all 4 345 faces on their surface.

Where the data is not as the reference prints it, found on corpus files:

- **cone**: `P + vA + (X cos u + Y sin u)(r + v tan a)`, the mirror image of the printed
  `P - vA + ...`. The corners of 58 cone faces of `Residential New Construction.sta` lie on
  this surface and up to 0.5 mm off the printed one, and curves given in the parameters of a
  cone have `v` along `A`;
- **ellipse**: `sense` comes before `centre`, as in every other curve (the printed struct has
  `centre` first, which gives centres and axes that are not numbers);
- **B-surface**: the control points run with u slowest (225 faces fit that order, none the
  other). A face may lie beyond the range of its surface: there the surface goes on straight.
  A closed direction is followed round;
- a curve in the parameters of a surface (SP-curve) that is straight there is still curved in
  space: a half circle on a cylinder is one segment of degree 1.

An orientation check that needs no reference: in a closed skin every edge between two triangles
is used once in each direction. `vwx xt` counts the edges used so, those used twice the same way
(one of the two faces inside out) and the rest (edges of faces whose common edge was stepped
differently, and of open sheets).

Solids made as primitives (type 95: sphere, cone, hemisphere) have no Parasolid body: their
type 85 child is a mesh (see the table above).

### Not decoded yet

What a viewport shows (layers, classes, crop, cut), cubic vertices of polylines, NURBS curve (111) on its own, slab (71), plug-in object (86), viewport
(122), records and formats (47, 48), styles (102, 107, 127), what a saved view shows (its `svdn` and `ccst` user data), sheet layers, class
attributes and visibility.
