// Scratch tool (not committed): dumps the records of some kinds of every corpus file.
// usage: dump_kinds <dir> <out-prefix> <kind,kind,...>
use std::io::Write;
use std::path::{Path, PathBuf};
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("vwx") || e.eq_ignore_ascii_case("sta")) {
            out.push(path);
        }
    }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let kinds: Vec<u8> = args[3].split(',').map(|k| k.parse().unwrap()).collect();
    let mut files = Vec::new();
    walk(Path::new(&args[1]), &mut files);
    files.sort();
    let mut list = std::fs::File::create(format!("{}.files", args[2])).unwrap();
    let mut bin = std::io::BufWriter::new(std::fs::File::create(format!("{}.bin", args[2])).unwrap());
    for (n, path) in files.iter().enumerate() {
        writeln!(list, "{}", path.display()).unwrap();
        let data = std::fs::read(path).unwrap();
        let Ok(u) = vwx_core::container::unpack(&data) else { continue };
        let Ok(raw) = vwx_read::raw::Raw::new(&u) else { continue };
        for (i, r) in raw.records.iter().enumerate() {
            if kinds.contains(&r.kind()) {
                let parent = raw.parents[i].map_or(255u8, |p| raw.records[p].kind());
                bin.write_all(&(n as u32).to_le_bytes()).unwrap();
                bin.write_all(&[parent]).unwrap();
                bin.write_all(&(i as u32).to_le_bytes()).unwrap();
                let cap: usize = args.get(4).map_or(usize::MAX, |c| c.parse().unwrap());
                let b = &r.bytes[..r.bytes.len().min(cap)];
                bin.write_all(&(b.len() as u32).to_le_bytes()).unwrap();
                bin.write_all(b).unwrap();
            }
        }
    }
}
