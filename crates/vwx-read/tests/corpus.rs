//! Runs over the sample set: every `.vwx` and `.sta` under `VWX_SAMPLES` must open without an
//! error. The Vectorworks installation itself is a good set (`C:\Program Files\Vectorworks
//! 2027`: 787 files); the line printed at the end counts how many give meshes.
//!
//! ```text
//! VWX_SAMPLES=C:\vwx-samples cargo test -p vwx-read --test corpus -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};

fn samples() -> Vec<PathBuf> {
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
    let mut out = Vec::new();
    if let Some(dir) = std::env::var_os("VWX_SAMPLES") {
        walk(Path::new(&dir), &mut out);
    }
    out.sort();
    out
}

#[test]
#[ignore = "needs VWX_SAMPLES"]
fn every_sample_reads() {
    let files = samples();
    assert!(!files.is_empty(), "set VWX_SAMPLES to the folder of the sample files");
    let (mut failed, mut with_meshes, mut objects, mut triangles) = (0, 0, 0usize, 0usize);
    for path in &files {
        let data = std::fs::read(path).unwrap();
        let doc = match vwx_read::open(&data) {
            Ok(doc) => doc,
            Err(e) => {
                failed += 1;
                println!("FAIL  {}  {e}", path.display());
                continue;
            }
        };
        objects += doc.objects.len();
        let scene = vwx_read::mesh::scene(&doc);
        let n: usize = scene.meshes.iter().map(|m| m.indices.len() / 3).sum();
        triangles += n;
        with_meshes += usize::from(n > 0);
    }
    println!(
        "{} of {} files open, {objects} objects; {with_meshes} files with meshes, {triangles} triangles",
        files.len() - failed,
        files.len()
    );
    assert_eq!(failed, 0);
}
