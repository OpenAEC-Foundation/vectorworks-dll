# Architecture and plan

## Goal

One Rust library that opens a Vectorworks `.vwx` file without Vectorworks and

1. gives **viewer meshes** of the 3D model: one mesh per layer and class, objects pickable, with
   their properties (records, IFC data) — the first user is open-3d-viewer;
2. lets a caller **query every object**: layers, classes, 2D and 3D objects, symbols and their
   instances, BIM objects (walls, slabs, roofs, columns, doors, windows, stairs, spaces), records
   and their fields, sheet layers and viewports;
3. is usable from Rust (also in the browser: everything works on `&[u8]`), from C/C# and other
   languages through `vwx.dll`, and from the command line.

## How the format is decoded

Black-box only (see the clean-room rules in the README):

1. **Survey.** `vwx survey`, `vwx probe` and `vwx records` over the files a Vectorworks
   installation ships with (the corpus: 787 files): header, compression, records, object types.
2. **Controlled pairs.** Vectorworks saves a file, one thing changes, it saves again
   (`docs/ORACLE.md`). `vwx diff a b --streams` shows where the change lands: a coordinate shows
   up as a changed `f64`, a renamed layer as a changed string with its length field, a new object
   as an inserted record. The ladder of samples goes from an empty document up to BIM objects.
3. **Reference exports.** For each sample Vectorworks also writes IFC and DXF; the reader's
   output is compared with them (counts, names, coordinates, bounding boxes).
4. Every finding goes into `docs/FORMAT.md` with the sample pair that shows it, then into code
   with a test.

## Layers and status

| Layer | Status | Check |
|---|---|---|
| Analysis tools: probe, survey, scan (zlib streams, strings, entropy), diff, hex, unpack, records, tree | **done** | unit tests |
| Header: versions (made with, saved by), lengths, build | **done** for files saved by VW 2023 and 2027; older headers recognised only | 787 corpus files |
| Container: Snappy blocks, records, object types, object tree | **done** (`vwx_core::container`) | 787 of 787 corpus files open, 2.2 M objects, 26 s |
| Document tables: reference table, name list, classes (name, visibility), layers (name, elevation), colour palette | **done**; units and origin, class attributes, layer visibility open | ladder samples; palette against the scripting API |
| 2D primitives: line, rectangle, oval, polygon, arc, circle, locus | **done**, with rounded rectangle and polyline (arc and Bezier vertices); text and cubic vertices open | ladder samples 02 to 18, 111 to 113, 220 |
| 3D primitives: extrude, multiple extrude, sweep, 3D polygon, mesh, sphere, hemisphere, cone, NURBS surface with its boundaries | **done**; NURBS curves on their own open | meshes with the bounds of Vectorworks' STL export for all 48 samples that have one |
| Symbols: definitions, instances (placement, scale) | **done** | STL export of samples 50, 51 |
| Groups and nesting | **done** | sample 43 |
| Solids as Parasolid bodies (walls, CSG solids, sweeps, multiple extrudes): type 114 section 19, read by `vwx-xt` | **done** for planes, cylinders, cones, spheres, tori, B-surfaces, swept and spun surfaces; faces on blend and offset surfaces are filled flat | STL export of samples 34, 35, 60 to 65, 210 to 216; 67 714 of 67 788 corpus bodies parse; 99.2 % of the 641 490 corpus faces are triangulated on their surface |
| Slabs, roofs, columns, doors, windows, stairs, spaces | not started | — |
| Records and IFC data attached to objects | not started | — |
| Viewer meshes (`vwx_read::read`): per layer and class, objects pickable, colours from the palette; a file with nothing on its layers shows its symbol definitions side by side | **done** for what is decoded: 455 of the 787 corpus files give meshes (99 M triangles, 2 minutes for all); the others hold resources and 2D symbols without anything filled. What is in an invisible class starts hidden | STL comparison `tools/compare_stl.py`: all 48 samples with an export have its bounds |
| C ABI: `vwx_open`, `vwx_open_bytes`, `vwx_query` (info, probe, document, objects), `vwx_free`, `vwx_close` | **done** (answers grow with the reader) | unit tests |

## Crates

| Crate | Role |
|---|---|
| `vwx-core` | Container layer and analysis tools; no knowledge of objects |
| `vwx-xt` | Parasolid XT neutral binary: nodes, topology, triangulation of faces. Knows nothing of Vectorworks |
| `vwx-model` | Neutral, serialisable model (`Document`, `Layer`, `Class`, `Object`) and viewer output (`Scene`, `Mesh`); metres, Z up |
| `vwx-read` | Object readers: container records to `vwx-model`; mesh building |
| `vwx-ffi` | `vwx.dll` (C ABI, JSON requests) |
| `vwx-cli` | `vwx` command: reader front end and analysis tools |

## Use in open-3d-viewer

The viewer depends on `vwx-read` (Git dependency pinned to a commit) and maps `vwx_model::Mesh`
one to one onto its `Layer` in `src/loaders/vwx.rs`, as it does for `skp-reader`. No dependency
of this repository on the viewer.
