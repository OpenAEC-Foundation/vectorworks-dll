//! `vwx coverage <file|dir>`: per kind of object on the design layers, how many are drawn and
//! why the others are not (see `vwx_read::coverage`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use vwx_read::coverage::{Entry, Status};

/// The `.vwx` and `.sta` files under `path` (or `path` itself), sorted.
fn files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let mut out = Vec::new();
    let mut todo = vec![path.to_path_buf()];
    while let Some(dir) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for p in entries.flatten().map(|e| e.path()) {
            if p.is_dir() {
                todo.push(p);
            } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("vwx") || e.eq_ignore_ascii_case("sta")) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Counts of one label.
#[derive(Default)]
struct Row {
    objects: usize,
    by_status: [usize; Status::ALL.len()],
    hidden: usize,
    files: usize,
    files_missing: usize,
}

/// Runs the command. Options: `--top n` rows (default 40), `--list text` the missing objects
/// whose label holds `text` (file, record, label), `--status s` with `--list`: another status.
pub fn run(path: &str, opt: impl Fn(&str) -> Option<String>) -> Result<(), String> {
    let list = files(Path::new(path));
    if list.is_empty() {
        return Err(format!("{path}: no .vwx or .sta files"));
    }
    let results: Mutex<Vec<(PathBuf, Result<Vec<Entry>, String>)>> = Mutex::new(Vec::new());
    let next = Mutex::new(0usize);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(4);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let k = {
                    let mut n = next.lock().unwrap();
                    *n += 1;
                    *n - 1
                };
                let Some(file) = list.get(k) else { break };
                let result = std::fs::read(file).map_err(|e| e.to_string()).and_then(|d| vwx_read::coverage::coverage(&d).map_err(|e| e.to_string()));
                results.lock().unwrap().push((file.clone(), result));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by(|a, b| a.0.cmp(&b.0));
    let wanted = opt("--list");
    let wanted_status = opt("--status").unwrap_or_else(|| "missing".into());
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let (mut failed, mut total) = (0usize, Row::default());
    for (file, result) in &results {
        let entries = match result {
            Ok(e) => e,
            Err(e) => {
                failed += 1;
                eprintln!("{}: {e}", file.display());
                continue;
            }
        };
        let mut in_file: BTreeMap<&str, bool> = BTreeMap::new();
        for e in entries {
            let row = rows.entry(e.label.clone()).or_default();
            row.objects += 1;
            row.by_status[e.status as usize] += 1;
            row.hidden += usize::from(e.hidden);
            total.objects += 1;
            total.by_status[e.status as usize] += 1;
            total.hidden += usize::from(e.hidden);
            *in_file.entry(&e.label).or_default() |= e.status == Status::Missing;
            if let Some(text) = &wanted {
                if (wanted_status == "all" || e.status.name() == wanted_status) && e.label.contains(text.as_str()) {
                    println!("{}\t{}\t{}\t{}\t{}\t{}", file.display(), e.record, e.label, e.status.name(), e.parent, e.hidden);
                }
            }
        }
        for (label, missing) in in_file {
            let row = rows.get_mut(label).unwrap();
            row.files += 1;
            row.files_missing += usize::from(missing);
        }
    }
    if wanted.is_some() {
        return Ok(());
    }
    let top = opt("--top").and_then(|t| t.parse().ok()).unwrap_or(40usize);
    let missing = Status::Missing as usize;
    let mut sorted: Vec<(&String, &Row)> = rows.iter().collect();
    sorted.sort_by(|a, b| b.1.by_status[missing].cmp(&a.1.by_status[missing]).then(b.1.objects.cmp(&a.1.objects)));
    println!("{} files ({failed} not read), {} objects on design layers ({} hidden)", results.len(), total.objects, total.hidden);
    let header: Vec<&str> = Status::ALL.iter().map(|s| s.name()).collect();
    println!("{:<44} {:>8} {:>6}  {}  {:>8}", "kind", "objects", "files", header.iter().map(|h| format!("{h:>8}")).collect::<Vec<_>>().join(" "), "files missing");
    let line = |label: &str, row: &Row| {
        println!(
            "{:<44} {:>8} {:>6}  {}  {:>8}",
            label.chars().take(44).collect::<String>(),
            row.objects,
            row.files,
            row.by_status.iter().map(|n| format!("{n:>8}")).collect::<Vec<_>>().join(" "),
            row.files_missing
        )
    };
    line("all", &total);
    for (label, row) in sorted.into_iter().take(top) {
        line(label, row);
    }
    Ok(())
}
