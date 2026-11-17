// Scratch survey: records by (parent kind, kind, user data tag) over the files named on stdin.
use std::collections::HashMap;
use std::io::BufRead;
fn main() {
    let mut stat: HashMap<(String, u8, String), (usize, usize, usize, usize)> = HashMap::new();
    for line in std::io::stdin().lock().lines() {
        let path = line.unwrap();
        let data = std::fs::read(&path).unwrap();
        let Ok(un) = vwx_core::container::unpack(&data) else { continue };
        let Ok(raw) = vwx_read::raw::Raw::new(&un) else { continue };
        let mut seen: HashMap<(String, u8, String), usize> = HashMap::new();
        for (i, r) in raw.records.iter().enumerate() {
            let pk = match raw.parents[i] {
                None => "doc".to_string(),
                Some(p) => raw.records[p].kind().to_string(),
            };
            let tag = if r.kind() == 76 {
                let p = &r.bytes[0x50..];
                let t: Vec<u8> = p[..4.min(p.len())].iter().rev().cloned().collect();
                let mut s = String::from_utf8_lossy(&t).to_string();
                if p.starts_with(b"TDMD") && p.len() >= 10 {
                    s = format!("TDMD/{}", String::from_utf8_lossy(&[p[9], p[8], p[7], p[6]]));
                }
                s
            } else {
                String::new()
            };
            let k = (pk, r.kind(), tag);
            let e = stat.entry(k.clone()).or_default();
            e.1 += 1;
            e.2 += r.bytes.len();
            *seen.entry(k).or_default() += 1;
        }
        for (k, _) in seen {
            stat.get_mut(&k).unwrap().0 += 1;
        }
    }
    let mut v: Vec<_> = stat.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(b.1 .1.cmp(&a.1 .1)));
    for ((pk, k, tag), (files, n, bytes, _)) in v {
        println!("{files:4} files {n:8} recs {bytes:10} bytes  parent={pk:4} kind={k:3} {tag}");
    }
}
