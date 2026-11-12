//! Relecture d'un enregistrement de `tools/recorder` (`capture.txt`), pour
//! tester toute la chaîne sans le jeu. Les enregistrements restent sur le PC
//! (ignorés par git).
//!
//! Format d'une ligne : `ts_ms|src:sport>dst:dport|seq|hex`.

use std::path::Path;

use crate::capture::Segment;

fn ip(s: &str) -> Option<u32> {
    let mut b = [0u8; 4];
    let mut parts = s.split('.');
    for x in &mut b {
        *x = parts.next()?.parse().ok()?;
    }
    Some(u32::from_be_bytes(b))
}

fn endpoint(s: &str) -> Option<(u32, u16)> {
    let (a, p) = s.rsplit_once(':')?;
    Some((ip(a)?, p.parse().ok()?))
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

pub fn parse_line(line: &str) -> Option<Segment> {
    if line.starts_with('#') || line.is_empty() {
        return None;
    }
    let mut parts = line.splitn(4, '|');
    let ts_ms = parts.next()?.parse().ok()?;
    let (src, dst) = parts.next()?.split_once('>')?;
    let seq = parts.next()?.parse().ok()?;
    let data = unhex(parts.next()?.trim())?;
    let (s, sp) = endpoint(src)?;
    let (d, dp) = endpoint(dst)?;
    Some(Segment { ts_ms, key: (s, sp, d, dp), seq, data, known: true })
}

/// Segments d'un dossier d'enregistrement, dans l'ordre de capture.
pub fn read_segments(dir: &Path) -> std::io::Result<Vec<Segment>> {
    let text = std::fs::read_to_string(dir.join("capture.txt"))?;
    Ok(text.lines().filter_map(parse_line).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ligne() {
        let s = parse_line("1700000000000|10.0.0.1:13328>192.168.1.2:50000|42|0a3438").unwrap();
        assert_eq!(s.key, (0x0A00_0001, 13328, 0xC0A8_0102, 50000));
        assert_eq!(s.seq, 42);
        assert_eq!(s.data, vec![0x0a, 0x34, 0x38]);
        assert!(parse_line("# entête").is_none());
    }
}
