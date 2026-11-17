use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value};
use vwx_core::{diff, hex, scan, Probe};

const USAGE: &str = "usage:
  vwx info <file.vwx>                       document facts
  vwx read <file.vwx>                       read the file; meshes and triangles per mesh
  vwx document <file.vwx>                   the neutral model as JSON
  vwx mesh <file.vwx> [--obj out.obj]       bounds and triangles of the meshes; writes an OBJ
  vwx xt <file> [--errors] [--obj out.obj]  the Parasolid bodies in a file: node types, failures,
                                            faces and triangles (in the units of the bodies)
analysis:
  vwx probe <file> [--pretty]               container, compressed streams, strings, entropy (JSON)
  vwx survey <dir>                          one line per file: size, head bytes, streams
  vwx hex <file> <offset> [len] [--stream n]
                                            hex dump (offset and len decimal or 0x..); --stream:
                                            inside inflated stream n (see `streams`)
  vwx strings <file> [--min n] [--stream n] printable runs (ASCII, UTF-16 LE/BE) with offsets
  vwx streams <file> [--out dir]            zlib streams; --out writes each one inflated
  vwx unpack <file> [--out body.bin] [--document doc.bin]
                                            versions and sizes; writes the unpacked body
  vwx records <file>                        records per object type
  vwx tree <file> [--depth n] [--all]       the object tree (--all: with user data records)
  vwx diff <a> <b> [--gap n] [--streams]    byte ranges that differ, with both sides in hex and
                                            read as numbers; --streams: also inside the streams
                                            (pairwise by index)
Commands that print JSON take `-o out.json` to write a file instead.";

/// The value after a `--name` option.
fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// The positional arguments (options and their values left out).
fn positional(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
        } else if let Some(name) = a.strip_prefix("--").or_else(|| a.strip_prefix('-').filter(|n| n.len() == 1)) {
            // options with a value
            skip = matches!(name, "gap" | "stream" | "min" | "out" | "o" | "depth" | "document" | "obj");
        } else {
            out.push(a.as_str());
        }
    }
    out
}

fn number(s: &str) -> Result<usize, String> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => usize::from_str_radix(h, 16),
        None => s.parse(),
    }
    .map_err(|_| format!("not a number: {s}"))
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{path}: {e}"))
}

/// The bytes of `path`, or of its inflated stream `--stream n`.
fn bytes_of(path: &str, args: &[String]) -> Result<Vec<u8>, String> {
    let data = read(path)?;
    match opt(args, "--stream") {
        None => Ok(data),
        Some(n) => {
            let n = number(n)?;
            let mut streams = scan::zlib_streams(&data, 16);
            if n >= streams.len() {
                return Err(format!("{path} has {} streams", streams.len()));
            }
            Ok(streams.swap_remove(n).data)
        }
    }
}

/// `data` read as the numbers a field of its length could be.
fn as_numbers(data: &[u8]) -> String {
    use vwx_core::{be, le};
    match data.len() {
        2 => format!("u16 le {} be {}", le::u16(data, 0).unwrap(), be::u16(data, 0).unwrap()),
        4 => format!("u32 le {} be {}; f32 le {} be {}", le::u32(data, 0).unwrap(), be::u32(data, 0).unwrap(), f32::from_bits(le::u32(data, 0).unwrap()), f32::from_bits(be::u32(data, 0).unwrap())),
        8 => format!("u64 le {}; f64 le {} be {}", le::u64(data, 0).unwrap(), le::f64(data, 0).unwrap(), be::f64(data, 0).unwrap()),
        _ => String::new(),
    }
}

fn print_diff(label: &str, a: &[u8], b: &[u8], gap: usize) -> usize {
    let changes = diff::join_close(&diff::diff(a, b, diff::MAX_EDITS), gap);
    println!("{label}: {} -> {} bytes, {} changed ranges", a.len(), b.len(), changes.len());
    for c in &changes {
        let (sa, sb) = (&a[c.a..c.a + c.a_len], &b[c.b..c.b + c.b_len]);
        println!("  a 0x{:x} +{}  ->  b 0x{:x} +{}", c.a, c.a_len, c.b, c.b_len);
        let show = |s: &[u8]| {
            if s.len() > 48 {
                format!("{} ...", hex::bytes(&s[..48]))
            } else {
                hex::bytes(s)
            }
        };
        println!("    a: {}", show(sa));
        println!("    b: {}", show(sb));
        if c.a_len == c.b_len && !as_numbers(sa).is_empty() {
            println!("    a: {}\n    b: {}", as_numbers(sa), as_numbers(sb));
        }
    }
    changes.len()
}

fn survey(dir: &Path) -> Result<(), String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.sort();
    for path in files {
        let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let p = Probe::of(&data);
        let share = if data.is_empty() { 0 } else { p.stream_bytes * 100 / data.len() };
        println!(
            "{:<40} {:>10}  {:<18} {}  {:?}  {} streams ({share}% of file)",
            path.file_name().unwrap_or_default().to_string_lossy(),
            data.len(),
            p.header.as_ref().map_or("-".into(), |h| h.product()),
            hex::bytes(&data[..data.len().min(24)]),
            p.container,
            p.streams.len()
        );
    }
    Ok(())
}

/// Runs one command; `Some(json)` for the commands that answer in JSON.
fn run(args: &[String]) -> Result<Option<Value>, String> {
    let pos = positional(args);
    let cmd = *pos.first().ok_or(USAGE)?;
    let file = || pos.get(1).copied().ok_or(USAGE.to_string());
    let to = |v: Result<Value, serde_json::Error>| v.map(Some).map_err(|e| e.to_string());
    match cmd {
        "info" => {
            let data = read(file()?)?;
            to(serde_json::to_value(vwx_read::open(&data).map(|d| d.info).unwrap_or_else(|_| vwx_read::info(&data))))
        }
        "document" => {
            let path = file()?;
            let doc = vwx_read::open(&read(path)?).map_err(|e| format!("{path}: {e}"))?;
            to(serde_json::to_value(doc))
        }
        "read" => {
            let path = file()?;
            let scene = vwx_read::read(&read(path)?).map_err(|e| format!("{path}: {e}"))?;
            // --sheet <number>: that sheet in full
            if let Some(number) = opt(args, "--sheet") {
                return match scene.sheets.iter().find(|s| s.number == number) {
                    Some(sheet) => to(serde_json::to_value(sheet)),
                    None => Err(format!("{path}: no sheet {number}")),
                };
            }
            // --props <text>: the properties of the objects that have that text in them
            if let Some(text) = opt(args, "--props") {
                for props in scene.meshes.iter().flat_map(|m| &m.props).filter(|p| p.1.contains(text)) {
                    println!("{}", props.1);
                }
                return Ok(None);
            }
            Ok(Some(json!({
                "info": scene.info,
                "meshes": scene.meshes.iter().map(|m| json!({
                    "name": m.name, "triangles": m.indices.len() / 3, "lines": m.lines.len() / 2, "objects": m.props.len(), "hidden": m.hidden,
                })).collect::<Vec<_>>(),
                "views": scene.views,
                "texts": scene.texts,
                "sheets": scene.sheets.iter().map(|s| json!({
                    "number": s.number, "title": s.title, "lines": s.lines.len(), "viewports": s.viewports,
                    "texts": s.texts.iter().map(|t| t.text.clone()).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })))
        }
        "mesh" => {
            // bounds (metres) and triangle count of what the reader draws; --obj writes it
            let path = file()?;
            let scene = vwx_read::read(&read(path)?).map_err(|e| format!("{path}: {e}"))?;
            let (mut lo, mut hi, mut triangles) = ([f32::MAX; 3], [f32::MIN; 3], 0);
            let mut obj = String::new();
            let mut base = 1;
            for m in &scene.meshes {
                triangles += m.indices.len() / 3;
                for p in m.positions.chunks(3) {
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                    obj.push_str(&format!(
                        "v {} {} {}
",
                        p[0], p[1], p[2]
                    ));
                }
                obj.push_str(&format!(
                    "g {}
",
                    m.name.replace(' ', "_")
                ));
                for t in m.indices.chunks(3) {
                    obj.push_str(&format!(
                        "f {} {} {}
",
                        base + t[0],
                        base + t[1],
                        base + t[2]
                    ));
                }
                base += (m.positions.len() / 3) as u32;
            }
            if let Some(out) = opt(args, "--obj") {
                std::fs::write(out, obj).map_err(|e| format!("{out}: {e}"))?;
            }
            Ok(Some(
                json!({"meshes": scene.meshes.len(), "triangles": triangles, "min": if triangles > 0 { json!(lo) } else { Value::Null }, "max": if triangles > 0 { json!(hi) } else { Value::Null }}),
            ))
        }
        "xt" => {
            // the Parasolid bodies of a file (or of raw XT data): node counts, and what fails
            let path = file()?;
            let data = read(path)?;
            let (body, document) = match vwx_core::container::unpack(&data) {
                Ok(u) => (u.body, true),
                Err(_) => (data, false),
            };
            // in a document: the bodies in the records that hold solids (type 114). Elsewhere
            // bytes may look like the start of one: a record format keeps what was in memory
            // after its field names
            let mut spans: Vec<(usize, usize)> = Vec::new();
            match vwx_core::container::records(&body).ok().filter(|_| document) {
                Some(records) => {
                    for r in records.iter().filter(|r| r.kind() == 114) {
                        let starts = vwx_xt::find(r.bytes);
                        spans.extend(starts.iter().enumerate().map(|(k, &at)| (r.offset + at, r.offset + starts.get(k + 1).copied().unwrap_or(r.bytes.len()))));
                    }
                }
                None => {
                    let starts = vwx_xt::find(&body);
                    spans.extend(starts.iter().enumerate().map(|(k, &at)| (at, starts.get(k + 1).copied().unwrap_or(body.len()))));
                }
            }
            let mut kinds = std::collections::BTreeMap::<String, usize>::new();
            let mut errors = std::collections::BTreeMap::<String, usize>::new();
            let mut schemas = std::collections::BTreeMap::<String, usize>::new();
            let (mut ok, mut nodes) = (0usize, 0usize);
            let (mut faces, mut approximate, mut skipped, mut triangles) = (0usize, 0usize, 0usize, 0usize);
            let mut straight = std::collections::BTreeMap::<String, usize>::new();
            let mut approximate_by = std::collections::BTreeMap::<String, usize>::new();
            let mut triangles_by = std::collections::BTreeMap::<String, usize>::new();
            // edges between triangles: used once each way (as in a closed, consistently turned
            // skin), twice the same way (one of the two faces is inside out), or otherwise
            let (mut paired, mut unpaired) = (0usize, 0usize);
            let mut same_way = std::collections::BTreeMap::<String, usize>::new();
            let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            let mut obj = String::new();
            let mut obj_base = 1u32;
            for (k, &(at, end)) in spans.iter().enumerate() {
                match vwx_xt::parse(&body[at..end]) {
                    Ok(f) => {
                        ok += 1;
                        nodes += f.nodes.len();
                        *schemas.entry(f.schema.clone()).or_default() += 1;
                        for n in f.nodes.values() {
                            *kinds.entry(n.def.name.clone()).or_default() += 1;
                        }
                        let mesh = vwx_xt::tessellate(&f);
                        faces += mesh.faces;
                        approximate += mesh.approximate;
                        skipped += mesh.skipped;
                        for code in &mesh.straight_edges {
                            *straight.entry(vwx_xt::schema::base(*code).map_or("none", |b| b.0).to_string()).or_default() += 1;
                        }
                        for (code, why) in &mesh.approximate_surfaces {
                            let name = vwx_xt::schema::base(*code).map_or("none", |b| b.0);
                            *approximate_by.entry(format!("{name}: {why}")).or_default() += 1;
                        }
                        triangles += mesh.triangles.len();
                        for (k, face) in mesh.face_starts.iter().enumerate() {
                            let end = mesh.face_starts.get(k + 1).map_or(mesh.triangles.len(), |f| f.0);
                            *triangles_by.entry(vwx_xt::schema::base(face.1).map_or("none", |b| b.0).to_string()).or_default() += end - face.0;
                        }
                        {
                            let (mut blo, mut bhi) = ([f64::MAX; 3], [f64::MIN; 3]);
                            for p in &mesh.positions {
                                for a in 0..3 {
                                    blo[a] = blo[a].min(p[a]);
                                    bhi[a] = bhi[a].max(p[a]);
                                }
                            }
                            let grain = (0..3).map(|a| bhi[a] - blo[a]).fold(0.0, f64::max).max(1e-12) * 1e-7;
                            let mut ids = std::collections::HashMap::<[i64; 3], u32>::new();
                            let welded: Vec<u32> = mesh
                                .positions
                                .iter()
                                .map(|p| {
                                    let n = ids.len() as u32;
                                    *ids.entry(p.map(|c| (c / grain).round() as i64)).or_insert(n)
                                })
                                .collect();
                            let name = |t: usize| {
                                let k = mesh.face_starts.partition_point(|f| f.0 <= t).saturating_sub(1);
                                vwx_xt::schema::base(mesh.face_starts.get(k).map_or(0, |f| f.1)).map_or("none", |b| b.0)
                            };
                            let mut uses = std::collections::HashMap::<(u32, u32), (Vec<usize>, Vec<usize>)>::new();
                            for (t, tri) in mesh.triangles.iter().enumerate() {
                                for k in 0..3 {
                                    let (a, b) = (welded[tri[k] as usize], welded[tri[(k + 1) % 3] as usize]);
                                    if a == b {
                                        continue;
                                    }
                                    let e = uses.entry((a.min(b), a.max(b))).or_default();
                                    if a < b {
                                        e.0.push(t)
                                    } else {
                                        e.1.push(t)
                                    }
                                }
                            }
                            for (f, b) in uses.values() {
                                match (f.len(), b.len()) {
                                    (1, 1) => paired += 1,
                                    (2, 0) | (0, 2) => {
                                        let both = if f.len() == 2 { f } else { b };
                                        let mut names = [name(both[0]), name(both[1])];
                                        names.sort();
                                        *same_way.entry(names.join(" | ")).or_default() += 1;
                                    }
                                    _ => unpaired += 1,
                                }
                            }
                        }
                        for p in &mesh.positions {
                            for a in 0..3 {
                                lo[a] = lo[a].min(p[a]);
                                hi[a] = hi[a].max(p[a]);
                            }
                            obj.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2]));
                        }
                        for t in &mesh.triangles {
                            obj.push_str(&format!("f {} {} {}\n", obj_base + t[0], obj_base + t[1], obj_base + t[2]));
                        }
                        obj_base += mesh.positions.len() as u32;
                    }
                    Err(e) => {
                        if flag(args, "--errors") {
                            eprintln!("{path}: body {k} at {at:#x}: {e}");
                        }
                        *errors.entry(e.what).or_default() += 1;
                    }
                }
            }
            if let Some(out) = opt(args, "--obj") {
                std::fs::write(out, obj).map_err(|e| format!("{out}: {e}"))?;
            }
            let bounds = if triangles > 0 { json!([lo, hi]) } else { Value::Null };
            Ok(Some(json!({
                "bodies": spans.len(), "parsed": ok, "nodes": nodes, "schemas": schemas, "types": kinds, "errors": errors,
                "faces": faces, "approximate_faces": approximate, "skipped_faces": skipped, "triangles": triangles, "bounds": bounds, "straight_edges_by_curve": straight, "approximate_by_surface": approximate_by, "triangles_by_surface": triangles_by,
                "edges": {"paired": paired, "unpaired": unpaired, "same_way_by_surfaces": same_way},
            })))
        }
        "probe" => to(serde_json::to_value(Probe::of(&read(file()?)?))),
        "survey" => survey(Path::new(file()?)).map(|_| None),
        "hex" => {
            let data = bytes_of(file()?, args)?;
            let at = number(pos.get(2).ok_or(USAGE)?)?.min(data.len());
            let len = pos.get(3).map_or(Ok(256), |s| number(s))?;
            print!("{}", hex::dump(&data[at..(at + len).min(data.len())], at));
            Ok(None)
        }
        "strings" => {
            let data = bytes_of(file()?, args)?;
            let min = opt(args, "--min").map_or(Ok(4), number)?;
            for t in scan::strings(&data, min) {
                println!("0x{:08x} {:<7} {}", t.offset, format!("{:?}", t.encoding).to_lowercase(), t.text);
            }
            Ok(None)
        }
        "streams" => {
            let path = file()?;
            let streams = scan::zlib_streams(&read(path)?, 16);
            let out = opt(args, "--out").map(PathBuf::from);
            if let Some(dir) = &out {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            for (k, s) in streams.iter().enumerate() {
                println!("{k:>4}  at 0x{:08x}  {:>9} -> {:>9} bytes  {}", s.offset, s.compressed, s.data.len(), hex::bytes(&s.data[..s.data.len().min(16)]));
                if let Some(dir) = &out {
                    let name = dir.join(format!("{k:04}_{:08x}.bin", s.offset));
                    std::fs::write(&name, &s.data).map_err(|e| format!("{}: {e}", name.display()))?;
                }
            }
            Ok(None)
        }
        "unpack" => {
            let path = file()?;
            let u = vwx_core::container::unpack(&read(path)?).map_err(|e| format!("{path}: {e}"))?;
            let h = &u.header;
            println!(
                "{}: made with VW{}.{}.{}, saved by {} (VW{}.{}.{}, build {}); document header {} bytes; {} blocks, {} bytes unpacked",
                path,
                h.created.major,
                h.created.minor,
                h.created.patch,
                h.product(),
                h.saved.major,
                h.saved.minor,
                h.saved.patch,
                h.build,
                u.document.len(),
                u.blocks,
                u.body.len()
            );
            if let Some(out) = opt(args, "--out") {
                std::fs::write(out, &u.body).map_err(|e| format!("{out}: {e}"))?;
            }
            if let Some(out) = opt(args, "--document") {
                std::fs::write(out, &u.document).map_err(|e| format!("{out}: {e}"))?;
            }
            Ok(None)
        }
        "records" | "tree" => {
            let path = file()?;
            let u = vwx_core::container::unpack(&read(path)?).map_err(|e| format!("{path}: {e}"))?;
            let recs = vwx_core::container::records(&u.body).map_err(|e| format!("{path}: {e}"))?;
            if cmd == "records" {
                let mut by_kind = std::collections::BTreeMap::<u8, (usize, usize, usize)>::new();
                for r in &recs {
                    let e = by_kind.entry(r.kind()).or_insert((0, usize::MAX, 0));
                    *e = (e.0 + 1, e.1.min(r.bytes.len()), e.2.max(r.bytes.len()));
                }
                println!("{} records", recs.len());
                for (kind, (n, min, max)) in by_kind {
                    println!("{kind:>4} {:<28} {n:>7}  {min}..{max} bytes", vwx_read::kinds::name(kind));
                }
            } else {
                let parents = vwx_core::container::parents(&recs);
                let mut depth = vec![0usize; recs.len()];
                let limit = opt(args, "--depth").map_or(Ok(usize::MAX), number)?;
                for (i, r) in recs.iter().enumerate() {
                    depth[i] = parents[i].map_or(0, |p| depth[p] + 1);
                    // user data (type 76) fills the tree; shown only with --all
                    if depth[i] <= limit && (r.kind() != 76 || flag(args, "--all")) {
                        println!("{:indent$}{} {} (0x{:x}, {} bytes)", "", r.kind(), vwx_read::kinds::name(r.kind()), r.offset, r.bytes.len(), indent = depth[i] * 2);
                    }
                }
            }
            Ok(None)
        }
        "diff" => {
            let (a, b) = (read(file()?)?, read(pos.get(2).ok_or(USAGE)?)?);
            let gap = opt(args, "--gap").map_or(Ok(4), number)?;
            print_diff("file", &a, &b, gap);
            if flag(args, "--streams") {
                let (sa, sb) = (scan::zlib_streams(&a, 16), scan::zlib_streams(&b, 16));
                if sa.len() != sb.len() {
                    println!("streams: {} -> {}; compared pairwise up to the shorter list", sa.len(), sb.len());
                }
                for (k, (x, y)) in sa.iter().zip(&sb).enumerate() {
                    if x.data != y.data {
                        print_diff(&format!("stream {k} (a 0x{:x}, b 0x{:x})", x.offset, y.offset), &x.data, &y.data, gap);
                    }
                }
            }
            Ok(None)
        }
        _ => Err(USAGE.into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(None) => ExitCode::SUCCESS,
        Ok(Some(v)) => {
            let text = if flag(&args, "--pretty") { serde_json::to_string_pretty(&v) } else { serde_json::to_string(&v) }.unwrap_or_default();
            match opt(&args, "-o") {
                Some(path) => match std::fs::write(path, text) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("{path}: {e}");
                        ExitCode::FAILURE
                    }
                },
                None => {
                    println!("{text}");
                    ExitCode::SUCCESS
                }
            }
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
