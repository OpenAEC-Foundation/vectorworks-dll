# vwx-dll

Native library (DLL) that reads Vectorworks `.vwx` files without Vectorworks, for use by
[open-3d-viewer](https://github.com/OpenAEC-Foundation/open-3d-viewer) and other tools.

**Status:** private, pre-alpha. The container is read (header, Snappy compressed body, records,
object types and the object tree: all 787 files of a Vectorworks 2027 installation open);
object geometry is being decoded. Do not make this repository
public without a decision by the maintainers (see "Legal" below).

## Scope

- Input: `.vwx` documents and `.sta` templates as saved by Vectorworks 2023 and later (older
  versions as samples of them are checked; `.mcd` files later, if at all).
- Output: a neutral model (layers, classes, 2D and 3D objects, symbols, BIM objects such as
  walls, slabs, roofs, doors and windows, with their records) and triangle meshes for viewers,
  exposed through Rust, a C ABI (`vwx.dll`) and a command line tool.
- `vwx_read::read(&[u8])` works on bytes, so the reader also runs in the browser (wasm).

## Crates

| Crate | Role |
|---|---|
| `vwx-core` | Container layer: header, blocks, compressed streams; analysis tools (probe, scan, diff, hex) |
| `vwx-model` | Neutral, serialisable model and viewer meshes |
| `vwx-read` | Semantic readers: records of a file to `vwx-model` |
| `vwx-ffi` | `vwx.dll`: `vwx_open`, `vwx_query` (JSON in/out), `vwx_free`, `vwx_close` |
| `vwx-cli` | `vwx` command: `info`, `read`, `records`, `tree`, `unpack`, `probe`, `hex`, `strings`, `diff` |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the layers and their status,
[docs/FORMAT.md](docs/FORMAT.md) for what is known about the format so far, and
[docs/ORACLE.md](docs/ORACLE.md) for how sample files are made with Vectorworks.

## Clean-room rules

The reader is built only from black-box analysis of files and of what Vectorworks itself shows
and exports.

- Never open, disassemble, or decompile Vectorworks, its plug-ins or other third-party binaries;
  no memory dumps.
- Never consult third-party VWX parsers, SDK headers that describe the file format, or leaked or
  licensed format documentation.
- Contributors who have seen such material do not write reader code.
- Vectorworks is used as an oracle: it makes sample files from scripts we write, and exports
  them (IFC, DXF/DWG, 3DS/OBJ) as the reference for what a file contains. Files from the
  oracle are made from our own content only.

## Test data

Never commit `.vwx`, `.mcd` or `.sta` files, Vectorworks sample or library content, or exports
derived from them. Sample files live outside the repository (set `VWX_SAMPLES` to their folder);
`.gitignore` enforces the file types. The corpus test runs over the files a Vectorworks
installation ships with, where they are installed. Samples we make ourselves with the oracle
(`docs/ORACLE.md`) are our own and are kept in the private `vwx-samples` repository.

## Legal

Vectorworks is a registered trademark of Vectorworks, Inc. This project is not affiliated with or
endorsed by Vectorworks, Inc. Before any public release: legal review, settle the licence of the
Vectorworks installation used as oracle, and a patent freedom-to-operate check.
