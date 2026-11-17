// Scratch: record format names over the files on stdin, with field kinds.
use std::collections::HashMap;
use std::io::BufRead;
fn main() {
    let mut names: HashMap<String, (usize, String)> = HashMap::new();
    let mut kinds: HashMap<u16, (usize, usize, String)> = HashMap::new();
    for line in std::io::stdin().lock().lines() {
        let path = line.unwrap();
        let data = std::fs::read(&path).unwrap();
        let Ok(un) = vwx_core::container::unpack(&data) else { continue };
        let Ok(raw) = vwx_read::raw::Raw::new(&un) else { continue };
        let mut seen = std::collections::HashSet::new();
        for r in raw.records.iter() {
            if r.kind() != 47 {
                continue;
            }
            let name = raw.names.get(&r.ref_id()).cloned().unwrap_or_default();
            let e = names.entry(name.clone()).or_default();
            e.0 += 1;
            // raw field kinds
            let count = vwx_core::le::u32(r.bytes, 0x50).unwrap_or(0) as usize;
            for k in 0..count.min(4096) {
                let at = 0x60 + 136 * k;
                let Some(kind) = vwx_core::le::u16(r.bytes, at) else { break };
                let fname = r
                    .bytes
                    .get(at + 8..at + 136)
                    .map(|b| {
                        let u: Vec<u16> = b.chunks(2).map(|c| u16::from_le_bytes([c[0], c.get(1).copied().unwrap_or(0)])).take_while(|&x| x != 0).collect();
                        String::from_utf16_lossy(&u)
                    })
                    .unwrap_or_default();
                let ke = kinds.entry(kind).or_default();
                ke.0 += 1;
                if seen.insert(kind) {
                    ke.1 += 1;
                }
                if ke.2.len() < 120 && !ke.2.contains(&fname) {
                    ke.2 += &format!("{fname}; ");
                }
            }
        }
    }
    let mut v: Vec<_> = names.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    for (n, (c, _)) in v.iter().take(60) {
        println!("{c:5} {n}");
    }
    println!("---- field kinds: number, fields, files, examples");
    let mut k: Vec<_> = kinds.into_iter().collect();
    k.sort();
    for (kind, (n, files, ex)) in k {
        println!("kind {kind:3}: {n:7} fields in {files:4} files  e.g. {ex}");
    }
}
