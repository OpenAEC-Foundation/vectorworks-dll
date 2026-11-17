//! The byte ranges in which two files differ.
//!
//! Most of the format is found by saving a file twice from Vectorworks with one change in
//! between (a moved point, a renamed layer, one object more) and looking at what changed. A plain
//! byte compare fails as soon as one length changes, so this is a real diff (Myers), with
//! insertions and deletions, on the bytes between the common start and end of both files.

use serde::Serialize;

/// One differing range: `a_len` bytes at `a` in the first file became `b_len` bytes at `b` in the
/// second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Change {
    pub a: usize,
    pub a_len: usize,
    pub b: usize,
    pub b_len: usize,
}

/// The default bound on the number of edited bytes [`diff`] searches for.
pub const MAX_EDITS: usize = 1500;

/// The changes from `a` to `b`, in order. When the files differ in more than `max_edits` bytes
/// (between their common start and end) the answer is that one range differs as a whole.
pub fn diff(a: &[u8], b: &[u8], max_edits: usize) -> Vec<Change> {
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (ma, mb) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    if ma.is_empty() && mb.is_empty() {
        return Vec::new();
    }
    match myers(ma, mb, max_edits) {
        Some(changes) => changes.into_iter().map(|c| Change { a: c.a + pre, b: c.b + pre, ..c }).collect(),
        None => vec![Change { a: pre, a_len: ma.len(), b: pre, b_len: mb.len() }],
    }
}

/// Joins changes less than `gap` equal bytes apart, for reading.
pub fn join_close(changes: &[Change], gap: usize) -> Vec<Change> {
    let mut out: Vec<Change> = Vec::new();
    for &c in changes {
        match out.last_mut() {
            Some(p) if c.a.saturating_sub(p.a + p.a_len) < gap && c.b.saturating_sub(p.b + p.b_len) < gap => {
                p.a_len = c.a + c.a_len - p.a;
                p.b_len = c.b + c.b_len - p.b;
            }
            _ => out.push(c),
        }
    }
    out
}

/// Myers' O(ND) diff; `None` past `max_d` edits.
fn myers(a: &[u8], b: &[u8], max_d: usize) -> Option<Vec<Change>> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    let max = (a.len() + b.len()).min(max_d) as isize;
    let off = max + 1;
    let at = |k: isize| (off + k) as usize;
    let mut v = vec![0isize; 2 * max as usize + 3];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    for d in 0..=max {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let mut x = if k == -d || (k != d && v[at(k - 1)] < v[at(k + 1)]) { v[at(k + 1)] } else { v[at(k - 1)] + 1 };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[at(k)] = x;
            if x >= n && y >= m {
                return Some(backtrack(&trace, n, m, off));
            }
            k += 2;
        }
    }
    None
}

fn backtrack(trace: &[Vec<isize>], n: isize, m: isize, off: isize) -> Vec<Change> {
    let at = |k: isize| (off + k) as usize;
    let (mut x, mut y) = (n, m);
    let mut edits = Vec::new();
    for (d, v) in trace.iter().enumerate().rev() {
        let d = d as isize;
        let k = x - y;
        let prev_k = if k == -d || (k != d && v[at(k - 1)] < v[at(k + 1)]) { k + 1 } else { k - 1 };
        let prev_x = v[at(prev_k)];
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
        }
        if d > 0 {
            edits.push(match x == prev_x {
                // a byte of `b` inserted
                true => Change { a: x as usize, a_len: 0, b: prev_y as usize, b_len: 1 },
                // a byte of `a` deleted
                false => Change { a: prev_x as usize, a_len: 1, b: y as usize, b_len: 0 },
            });
        }
        (x, y) = (prev_x, prev_y);
    }
    edits.reverse();
    join_close(&edits, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applies `changes` to `a`, which must give `b`.
    fn apply(a: &[u8], b: &[u8], changes: &[Change]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = 0;
        for c in changes {
            out.extend(&a[i..c.a]);
            out.extend(&b[c.b..c.b + c.b_len]);
            i = c.a + c.a_len;
        }
        out.extend(&a[i..]);
        out
    }

    #[test]
    fn equal_files_have_no_changes() {
        assert!(diff(b"same", b"same", MAX_EDITS).is_empty());
    }

    #[test]
    fn finds_a_replaced_value() {
        let a = b"header\x00\x00\x00\x00\x00\x00\xf0\x3ftail";
        let b = b"header\x00\x00\x00\x00\x00\x00\x00\x40tail";
        assert_eq!(diff(a, b, MAX_EDITS), vec![Change { a: 12, a_len: 2, b: 12, b_len: 2 }]);
    }

    #[test]
    fn finds_a_longer_name_and_keeps_the_rest_aligned() {
        let a = b"\x05Layer\x01\x02\x03 rest of the file".to_vec();
        let b = b"\x07Layer-2\x01\x02\x03 rest of the file and more".to_vec();
        let changes = diff(&a, &b, MAX_EDITS);
        assert_eq!(apply(&a, &b, &changes), b);
        assert!(changes.len() <= 3, "{changes:?}");
        assert_eq!(changes[0], Change { a: 0, a_len: 1, b: 0, b_len: 1 });
    }

    #[test]
    fn random_edits_round_trip() {
        let mut seed = 7u32;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for _ in 0..50 {
            let a: Vec<u8> = (0..300).map(|_| (next() % 4) as u8).collect();
            let mut b = a.clone();
            for _ in 0..(next() % 10) {
                let at = next() as usize % (b.len() + 1);
                match next() % 3 {
                    0 if at < b.len() => {
                        b.remove(at);
                    }
                    1 => b.insert(at, (next() % 4) as u8),
                    _ if at < b.len() => b[at] = (next() % 4) as u8,
                    _ => {}
                }
            }
            assert_eq!(apply(&a, &b, &diff(&a, &b, MAX_EDITS)), b);
        }
    }

    #[test]
    fn too_many_edits_give_one_range() {
        let a = vec![0u8; 100];
        let b = vec![1u8; 100];
        assert_eq!(diff(&a, &b, 10), vec![Change { a: 0, a_len: 100, b: 0, b_len: 100 }]);
    }
}
