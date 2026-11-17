use std::io::BufRead;
fn main() {
    let pat = std::env::args().nth(1).unwrap_or("Title Block Project Data".into());
    for line in std::io::stdin().lock().lines() {
        let path = line.unwrap();
        let data = std::fs::read(&path).unwrap();
        let Ok(un) = vwx_core::container::unpack(&data) else { continue };
        let Ok(raw) = vwx_read::raw::Raw::new(&un) else { continue };
        let mut shown = 0;
        for i in 0..raw.records.len() {
            for (name, vals) in vwx_read::records::of_object(&raw, i) {
                if name == pat && shown < 1 {
                    shown += 1;
                    println!("{path}  (object {i}, parent kind {:?})", raw.parents[i].map(|p| raw.records[p].kind()));
                    for (k, v) in vals {
                        println!("     {k} = {v:?}");
                    }
                }
            }
        }
    }
}
