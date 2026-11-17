//! Hex dumps.

use std::fmt::Write;

/// `data` as hex, bytes separated by spaces.
pub fn bytes(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 3);
    for (i, b) in data.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// `data` as text, `.` for bytes that do not print.
pub fn text(data: &[u8]) -> String {
    data.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }).collect()
}

/// A classic dump of `data`, 16 bytes a line, offsets counted from `base`.
pub fn dump(data: &[u8], base: usize) -> String {
    let mut s = String::new();
    for (k, line) in data.chunks(16).enumerate() {
        let _ = writeln!(s, "{:08x}  {:<47}  {}", base + k * 16, bytes(line), text(line));
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn dumps_a_line() {
        assert_eq!(super::dump(b"AB\x00", 0x10), format!("00000010  41 42 00{}  AB.\n", " ".repeat(39)));
    }
}
