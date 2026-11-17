// Scratch: size on disk of each field kind (gap to the next offset) and styles seen.
use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;
fn main() {
    let mut hist: BTreeMap<(u16, i64), usize> = BTreeMap::new();
    let mut styles: BTreeMap<(u16, u16), usize> = BTreeMap::new();
    for line in std::io::stdin().lock().lines() {
        let path = line.unwrap();
        let data = std::fs::read(&path).unwrap();
        let Ok(un) = vwx_core::container::unpack(&data) else { continue };
        let Ok(raw) = vwx_read::raw::Raw::new(&un) else { continue };
        let _: HashMap<u8, u8> = HashMap::new();
        for r in raw.records.iter() {
            if r.kind() != 47 {
                continue;
            }
            let count = vwx_core::le::u32(r.bytes, 0x50).unwrap_or(0) as usize;
            let mut f: Vec<(u16, u16, u32)> = Vec::new();
            for k in 0..count.min(4096) {
                let at = 0x60 + 136 * k;
                let (Some(kind), Some(off), Some(extra)) = (vwx_core::le::u16(r.bytes, at), vwx_core::le::u16(r.bytes, at + 2), vwx_core::le::u32(r.bytes, at + 4)) else { break };
                f.push((off, kind, extra));
            }
            let mut s = f.clone();
            s.sort();
            for (i, (off, kind, extra)) in s.iter().enumerate() {
                let gap = s.get(i + 1).map(|n| (n.0 - off) as i64).unwrap_or(-1);
                *hist.entry((*kind, gap)).or_default() += 1;
                *styles.entry((*kind, (*extra >> 16) as u16)).or_default() += 1;
                if (*extra & 0xffff) != 0 {
                    *styles.entry((*kind, 0xffff)).or_default() += 1;
                }
            }
        }
    }
    println!("kind gap count");
    for ((k, g), c) in hist {
        println!("{k:3} {g:4} {c}");
    }
    println!("style (extra>>16)");
    for ((k, s), c) in styles {
        println!("{k:3} {s:5} {c}");
    }
}
