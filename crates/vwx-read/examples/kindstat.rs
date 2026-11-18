use std::collections::{BTreeMap, BTreeSet};
use vwx_core::container;

#[derive(Default)]
struct K {
    n: usize,
    files: BTreeSet<usize>,
    sizes: BTreeMap<usize, usize>,
    parents: BTreeMap<String, usize>,
    children: BTreeMap<u8, usize>,
    refd: usize,
    classd: usize,
    w2e: BTreeMap<u16, usize>,
    flags: BTreeMap<u8, usize>,
    samples: Vec<(usize, String, usize)>,
}

fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out)
        } else if matches!(p.extension().and_then(|e| e.to_str()), Some("vwx" | "sta")) {
            out.push(p)
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut files = Vec::new();
    walk(std::path::Path::new(&a[1]), &mut files);
    files.sort();
    let outdir = std::path::PathBuf::from(&a[2]);
    std::fs::create_dir_all(&outdir).unwrap();
    let mut kinds: BTreeMap<u8, K> = BTreeMap::new();
    // tags: (tag text, parent kind) -> (n, files, sizes)
    let mut tags: BTreeMap<(String, String), (usize, BTreeSet<usize>, BTreeMap<usize, usize>)> = BTreeMap::new();
    let mut tagsamples: BTreeMap<String, Vec<(usize, String, usize)>> = BTreeMap::new();
    for (fi, p) in files.iter().enumerate() {
        let Ok(data) = std::fs::read(p) else { continue };
        let Ok(u) = container::unpack(&data) else { continue };
        let Ok(recs) = container::records(&u.body) else { continue };
        let parents = container::parents(&recs);
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        for (i, r) in recs.iter().enumerate() {
            let k = r.kind();
            let pk = parents[i].map(|q| recs[q].kind().to_string()).unwrap_or("doc".into());
            let e = kinds.entry(k).or_default();
            e.n += 1;
            e.files.insert(fi);
            *e.sizes.entry(r.bytes.len()).or_default() += 1;
            *e.parents.entry(pk.clone()).or_default() += 1;
            if r.ref_id() != 0 {
                e.refd += 1
            }
            if r.class_id() != 0 {
                e.classd += 1
            }
            *e.w2e.entry(u16::from_le_bytes([r.bytes[0x2e], r.bytes[0x2f]])).or_default() += 1;
            *e.flags.entry(r.bytes[0x1f]).or_default() += 1;
            if !e.samples.iter().any(|s| s.1 == name) {
                e.samples.push((data.len(), name.clone(), r.offset));
                e.samples.sort();
                e.samples.truncate(5);
            }
            if let Some(q) = parents[i] {
                *kinds.entry(recs[q].kind()).or_default().children.entry(k).or_default() += 1;
            }
            if k == 76 && r.bytes.len() >= 0x54 {
                let t = &r.bytes[0x50..0x54];
                let mut tag: String = t.iter().rev().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
                if t == b"TDMD" || tag == "DMDT" {
                    if r.bytes.len() >= 0x5a {
                        let key: String = r.bytes[0x56..0x5a].iter().rev().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
                        tag = format!("DMDT/{key}");
                    }
                }
                let e = tags.entry((tag.clone(), pk)).or_default();
                e.0 += 1;
                e.1.insert(fi);
                *e.2.entry(r.bytes.len()).or_default() += 1;
                let s = tagsamples.entry(tag).or_default();
                if !s.iter().any(|x| x.1 == name) {
                    s.push((data.len(), name.clone(), r.offset));
                    s.sort();
                    s.truncate(4);
                }
            }
        }
        if fi % 100 == 0 {
            eprintln!("{fi}");
        }
    }
    let mut o = String::new();
    for (k, e) in &kinds {
        let sizes: Vec<_> = {
            let mut v: Vec<_> = e.sizes.iter().collect();
            v.sort_by_key(|x| std::cmp::Reverse(*x.1));
            v.into_iter().take(6).map(|(s, n)| format!("{s}x{n}")).collect()
        };
        o += &format!(
            "KIND {k} {} n={} files={} sizes(min {} max {} distinct {}) top[{}] ref={} class={}\n  parents {:?}\n  children {:?}\n  w2e {:?} flags {:?}\n  samples {:?}\n",
            vwx_read::kinds::name(*k),
            e.n,
            e.files.len(),
            e.sizes.keys().next().unwrap_or(&0),
            e.sizes.keys().last().unwrap_or(&0),
            e.sizes.len(),
            sizes.join(" "),
            e.refd,
            e.classd,
            e.parents,
            e.children,
            e.w2e,
            e.flags,
            e.samples.iter().map(|s| format!("{}@{}", s.1, s.2)).collect::<Vec<_>>()
        );
    }
    std::fs::write(outdir.join("kinds.txt"), o).unwrap();
    let mut o = String::new();
    for ((t, pk), (n, f, s)) in &tags {
        let sizes: Vec<_> = {
            let mut v: Vec<_> = s.iter().collect();
            v.sort_by_key(|x| std::cmp::Reverse(*x.1));
            v.into_iter().take(5).map(|(s, n)| format!("{s}x{n}")).collect()
        };
        o += &format!("{t}\tparent={pk}\tn={n}\tfiles={}\tsizes(min {} max {}) [{}]\n", f.len(), s.keys().next().unwrap(), s.keys().last().unwrap(), sizes.join(" "));
    }
    std::fs::write(outdir.join("tags.txt"), o).unwrap();
    let mut o = String::new();
    for (t, s) in &tagsamples {
        o += &format!("{t} {:?}\n", s.iter().map(|s| (s.1.clone(), s.2)).collect::<Vec<_>>());
    }
    std::fs::write(outdir.join("tagsamples.txt"), o).unwrap();
}
