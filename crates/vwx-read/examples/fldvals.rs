// Scratch: values of the reference-like field kinds (16, 23..30) in records, with the names the
// values resolve to as reference numbers.
use std::collections::BTreeMap;
use std::io::BufRead;
fn main() {
    let mut vals: BTreeMap<(u16, i64), (usize, String)> = BTreeMap::new();
    for line in std::io::stdin().lock().lines() {
        let path = line.unwrap();
        let data = std::fs::read(&path).unwrap();
        let Ok(un) = vwx_core::container::unpack(&data) else { continue };
        let Ok(raw) = vwx_read::raw::Raw::new(&un) else { continue };
        for r in raw.records.iter() {
            if r.kind() != 48 {
                continue;
            }
            let Some(number) = vwx_read::records::format_of(r) else { continue };
            let Some(&fi) = raw.by_ref.get(&number) else { continue };
            let f = &raw.records[fi];
            if f.kind() != 47 {
                continue;
            }
            let Some(skip) = vwx_core::le::u16(r.bytes, 0x54) else { continue };
            let Some(body) = r.bytes.get(0x56 + skip as usize..) else { continue };
            let count = vwx_core::le::u32(f.bytes, 0x50).unwrap_or(0) as usize;
            for k in 0..count.min(4096) {
                let at = 0x60 + 136 * k;
                let kind = vwx_core::le::u16(f.bytes, at).unwrap_or(0);
                if !(kind == 16 || (23..=30).contains(&kind)) {
                    continue;
                }
                let off = vwx_core::le::u16(f.bytes, at + 2).unwrap_or(0) as usize;
                let v: i64 = match kind {
                    27 => body.get(off).map(|&b| b as i64),
                    28 => vwx_core::le::u16(body, off).map(|v| v as i64),
                    _ => vwx_core::le::u32(body, off).map(|v| v as i32 as i64),
                }
                .unwrap_or(-999999);
                let nm: Vec<u16> = f.bytes[at + 8..at + 136].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&x| x != 0).collect();
                let e = vals.entry((kind, v)).or_insert((0, String::new()));
                e.0 += 1;
                if e.1.len() < 60 {
                    let n = String::from_utf16_lossy(&nm);
                    if !e.1.contains(&n) {
                        e.1 += &format!("{n}; ");
                    }
                }
            }
        }
    }
    // top values per kind
    let mut per: BTreeMap<u16, Vec<(i64, usize, String)>> = BTreeMap::new();
    for ((k, v), (c, n)) in vals {
        per.entry(k).or_default().push((v, c, n));
    }
    for (k, mut v) in per {
        v.sort_by(|a, b| b.1.cmp(&a.1));
        println!("kind {k}: {} distinct", v.len());
        for (val, c, n) in v.iter().take(7) {
            println!("   {val:8} x{c:6}  {n}");
        }
    }
}
