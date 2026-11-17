// Scratch: dump records of a file. args: file [kind] [tag] [maxbytes]
fn tagof(r: &vwx_core::container::Record) -> String {
    if r.kind() != 76 {
        return String::new();
    }
    let p = &r.bytes[0x50..];
    if p.starts_with(b"TDMD") && p.len() >= 10 {
        return format!("TDMD/{}", String::from_utf8_lossy(&[p[9], p[8], p[7], p[6]]));
    }
    let t: Vec<u8> = p[..4.min(p.len())].iter().rev().cloned().collect();
    String::from_utf8_lossy(&t).to_string()
}
fn strings(b: &[u8]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= b.len() {
        let mut j = i;
        while j + 2 <= b.len() && b[j + 1] == 0 && (b[j] >= 0x20 && b[j] < 0x7f) {
            j += 2;
        }
        if j - i >= 8 {
            out.push(format!("@{:x}:{}", i, String::from_utf8_lossy(&b[i..j].chunks(2).map(|c| c[0]).collect::<Vec<_>>())));
            i = j;
        } else {
            i += 1;
        }
    }
    out.join(" | ")
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let kind: i32 = a.get(2).map(|s| s.parse().unwrap_or(-1)).unwrap_or(-1);
    let tag = a.get(3).cloned().unwrap_or_default();
    let max: usize = a.get(4).map(|s| s.parse().unwrap()).unwrap_or(256);
    let un = vwx_core::container::unpack(&data).unwrap();
    let raw = vwx_read::raw::Raw::new(&un).unwrap();
    for (i, r) in raw.records.iter().enumerate() {
        let t = tagof(r);
        if (kind >= 0 && r.kind() as i32 != kind) || (!tag.is_empty() && t != tag) {
            continue;
        }
        let mut chain = Vec::new();
        let mut p = raw.parents[i];
        while let Some(q) = p {
            chain.push(format!("{}#{}", raw.records[q].kind(), q));
            p = raw.parents[q];
        }
        println!("#{i} off={:#x} kind={} {} len={} ref={} parents=[{}]", r.offset, r.kind(), t, r.bytes.len(), r.ref_id(), chain.join(" "));
        let b = &r.bytes[0x50..];
        for (k, row) in b.chunks(16).take(max / 16 + 1).enumerate() {
            println!("  {:04x} {}", 0x50 + 16 * k, row.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "));
        }
        if r.kind() == 47 {
            let count = vwx_core::le::u32(r.bytes, 0x50).unwrap_or(0) as usize;
            for k in 0..count.min(4096) {
                let at = 0x60 + 136 * k;
                let kind = vwx_core::le::u16(r.bytes, at).unwrap_or(0);
                let off = vwx_core::le::u16(r.bytes, at + 2).unwrap_or(0);
                let extra = vwx_core::le::u32(r.bytes, at + 4).unwrap_or(0);
                let nm: Vec<u16> = r.bytes[at + 8..at + 136].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&x| x != 0).collect();
                println!("  field {k}: kind {kind} offset {off} extra {extra:#x} {}", String::from_utf16_lossy(&nm));
            }
        }
        let s = strings(b);
        if !s.is_empty() {
            println!("  strings: {s}");
        }
    }
}
