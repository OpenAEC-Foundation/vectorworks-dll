//! Runs over the private sample set: every `.vwx` under `VWX_SAMPLES` must open without a
//! panic, and once the container is decoded, without an error.
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
            } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("vwx")) {
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
    let mut failed = 0;
    for path in &files {
        let data = std::fs::read(path).unwrap();
        match vwx_read::read(&data) {
            Ok(scene) => {
                let triangles: usize = scene.meshes.iter().map(|m| m.indices.len() / 3).sum();
                println!("ok    {}  {} meshes, {triangles} triangles", path.display(), scene.meshes.len());
            }
            Err(e) => {
                failed += 1;
                println!("FAIL  {}  {e}", path.display());
            }
        }
    }
    println!("{} of {} files read", files.len() - failed, files.len());
    assert_eq!(failed, 0);
}
