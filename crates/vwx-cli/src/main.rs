use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value};
use vwx_core::{diff, hex, scan, Probe};

const USAGE: &str = "usage:
  vwx info <file.vwx>                       document facts
  vwx read <file.vwx>                       read the file; meshes and triangles per mesh
  vwx document <file.vwx>                   the neutral model as JSON
analysis:
  vwx probe <file> [--pretty]               container, compressed streams, strings, entropy (JSON)
  vwx survey <dir>                          one line per file: size, head bytes, streams
  vwx hex <file> <offset> [len] [--stream n]
                                            hex dump (offset and len decimal or 0x..); --stream:
                                            inside inflated stream n (see `streams`)
  vwx strings <file> [--min n] [--stream n] printable runs (ASCII, UTF-16 LE/BE) with offsets
  vwx streams <file> [--out dir]            zlib streams; --out writes each one inflated
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
            skip = matches!(name, "gap" | "stream" | "min" | "out" | "o");
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
        4 => format!(
            "u32 le {} be {}; f32 le {} be {}",
            le::u32(data, 0).unwrap(),
            be::u32(data, 0).unwrap(),
            f32::from_bits(le::u32(data, 0).unwrap()),
            f32::from_bits(be::u32(data, 0).unwrap())
        ),
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
        let show = |s: &[u8]| if s.len() > 48 { format!("{} ...", hex::bytes(&s[..48])) } else { hex::bytes(s) };
        println!("    a: {}", show(sa));
        println!("    b: {}", show(sb));
        if c.a_len == c.b_len && !as_numbers(sa).is_empty() {
            println!("    a: {}\n    b: {}", as_numbers(sa), as_numbers(sb));
        }
    }
    changes.len()
}

fn survey(dir: &Path) -> Result<(), String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    for path in files {
        let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let p = Probe::of(&data);
        let share = if data.is_empty() { 0 } else { p.stream_bytes * 100 / data.len() };
        println!(
            "{:<40} {:>10}  {}  {:?}  {} streams ({share}% of file)",
            path.file_name().unwrap_or_default().to_string_lossy(),
            data.len(),
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
            Ok(Some(json!({
                "info": scene.info,
                "meshes": scene.meshes.iter().map(|m| json!({
                    "name": m.name, "triangles": m.indices.len() / 3, "objects": m.elements.len(), "hidden": m.hidden,
                })).collect::<Vec<_>>(),
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
