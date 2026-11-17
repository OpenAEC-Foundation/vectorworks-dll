# Vectorworks as oracle

Vectorworks makes the sample files from our own scripts, and exports each one as the reference
for what it contains. Nothing from Vectorworks' own libraries, templates with content, or
sample projects goes into the samples.

## Where the files go

```
%VWX_SAMPLES%\                 outside this repository, never committed
  ladder\<version>\NN_name.vwx  the sample ladder below, one folder per Vectorworks version
  ladder\<version>\NN_name.ifc  IFC export of the same document
  ladder\<version>\NN_name.dxf  DXF export (2D + 3D)
  real\                         real project files (own projects only), for the corpus test
```

## The ladder

Each step changes **one** thing compared with the step before it, so that `vwx diff` of a pair
shows where that thing lives. Numbers stay fixed once samples exist; new steps get new numbers.

| # | Sample | Differs from | What the pair shows |
|---|---|---|---|
| 00 | empty document, saved | — | header, fixed structures |
| 01 | empty document, saved again | 00 | what changes on every save (time stamps, ids) |
| 02 | one line (0,0)→(1000,0) mm | 00 | an object record, coordinates, units |
| 03 | same line, end at (2000,0) | 02 | where the end point is, its number format |
| 04 | same line, named "L1" | 02 | object names |
| 05 | two lines | 02 | object list, ids, ordering |
| 06 | rectangle 1000×500 | 00 | rectangle record |
| 07 | polygon, 5 points | 00 | point lists |
| 08 | circle r=500, arc 90° | 00 | curves |
| 09 | layer renamed "Ontwerplaag-2" | 00 | layer table, string encoding |
| 10 | second design layer, elevation 3000 | 00 | layer elevation |
| 11 | class "Muren" added | 00 | class table |
| 12 | line 02 in class "Muren" | 02 + 11 | object → class link |
| 13 | fill colour of 06 red | 06 | attributes, colours |
| 14 | extrude of 06, height 3000 | 06 | 3D object, profile + height |
| 15 | extrude 14 at z=1000 | 14 | 3D placement |
| 16 | extrude 14 rotated 30° about z | 14 | transform matrix |
| 17 | sweep / multiple extrude | 00 | other 3D solids |
| 18 | symbol definition (the extrude) + one instance | 14 | symbols |
| 19 | second instance, rotated and moved | 18 | instance placement |
| 20 | group of 02 and 06 | 02, 06 | nesting |
| 21 | straight wall 5000 mm, 200 thick, 2700 high | 00 | wall (BIM) |
| 22 | wall 21 longer | 21 | wall path |
| 23 | wall with a door | 21 | inserted symbols / plug-in objects |
| 24 | wall with a window | 21 | idem |
| 25 | slab from a rectangle | 00 | slab |
| 26 | roof (roof face) | 00 | roof |
| 27 | column | 00 | column |
| 28 | stair | 00 | stair |
| 29 | space | 00 | space |
| 30 | record format with one field on line 02 | 02 | records |
| 31 | IFC data on wall 21 | 21 | IFC property sets |
| 32 | sheet layer with a viewport | 00 | sheet layers, viewports |

Units: millimetres, origin unchanged. One document is used throughout: the script clears it
before each step, so no "new document" dialog is needed.

## Oracle server

Vectorworks runs scripts only from inside the application, so `tools/oracle/server.py` is started
there once (Tools > Plug-ins > Run Script, from an empty document) and then waits for jobs:
Python files dropped into `%VWX_SAMPLES%\jobs`. Each job runs with `vs` and a `job` helper
(`job.step(name, draw, exports)` clears the document, draws, saves `name.vwx` and exports IFC
and STL when asked), writes `<job>.log` and moves to `jobs\done`. A file named `stop` ends the
server. Vectorworks does not respond while the server waits; that is expected.

The jobs that made the samples are kept in `tools/oracle/jobs/`, so a ladder can be made again
with another Vectorworks version. `001_ladder.py` is the table above (numbering as in the job).

After a run, `vwx diff a.vwx b.vwx` on the unpacked bodies (`vwx unpack --out`) shows what a
step changed; `00_empty` against `01_empty_again` is the noise floor.
