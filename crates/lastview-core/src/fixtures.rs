//! Extraits de test anonymisés, tirés de vrais enregistrements.
//!
//! Règle du repo public : pseudos et légions remplacés, longueurs
//! recalculées. On ne garde que les paquets que LastView décode, et seulement
//! les octets qu'il lit : la partie non lue d'un paquet (qui pourrait contenir
//! d'autres noms) est coupée.
//!
//! Format d'un extrait : une ligne par paquet, `ts_ms<TAB>hex` (opcode inclus).

use std::collections::HashMap;

use crate::protocol::framing::read_varint;
use crate::protocol::messages::{decode, decode_with_strings, Decoded};

#[derive(Default)]
pub struct Anonymizer {
    map: HashMap<String, String>,
    /// Chaînes d'origine, pour vérifier qu'aucune ne subsiste.
    originals: Vec<Vec<u8>>,
}

fn write_varint(out: &mut Vec<u8>, mut v: usize) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

impl Anonymizer {
    pub fn new() -> Self {
        Self::default()
    }

    fn replacement(&mut self, s: &str) -> String {
        if s.is_empty() {
            return String::new();
        }
        let n = self.map.len() + 1;
        self.map.entry(s.to_string()).or_insert_with(|| format!("Anon{n:03}")).clone()
    }

    /// Paquet anonymisé, ou `None` s'il n'est pas décodé par LastView ou ne
    /// peut pas être anonymisé sûrement.
    pub fn packet(&mut self, p: &[u8]) -> Option<Vec<u8>> {
        let (d, used, spans) = decode_with_strings(p);
        if !matches!(d, Decoded::Message(_)) {
            return None;
        }
        let mut out = Vec::with_capacity(used);
        let mut pos = 0;
        for (a, b) in spans {
            out.extend_from_slice(&p[pos..a]);
            let li = read_varint(p, a);
            if li.length <= 0 {
                return None;
            }
            let raw = &p[a + li.length as usize..b];
            let raw = raw.strip_suffix(&[0]).unwrap_or(raw);
            if raw.len() >= 3 {
                self.originals.push(raw.to_vec());
            }
            let rep = self.replacement(&String::from_utf8_lossy(raw));
            write_varint(&mut out, rep.len());
            out.extend_from_slice(rep.as_bytes());
            pos = b;
        }
        out.extend_from_slice(&p[pos..used]);
        // Le paquet anonymisé doit se décoder pareil, et ne plus contenir aucun nom d'origine.
        if !matches!(decode(&out), Decoded::Message(_)) {
            return None;
        }
        if self.originals.iter().any(|o| out.windows(o.len()).any(|w| w == o.as_slice())) {
            return None;
        }
        Some(out)
    }

    /// Nombre de chaînes distinctes remplacées.
    pub fn replaced(&self) -> usize {
        self.map.len()
    }
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

/// Lit un extrait : (heure, paquet).
pub fn parse(text: &str) -> Vec<(i64, Vec<u8>)> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let (ts, h) = l.split_once('\t')?;
            Some((ts.trim().parse().ok()?, unhex(h.trim())?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mort_anonymisee() {
        let mut p = vec![0x04, 0x8D, 0x05];
        p.extend(7i32.to_le_bytes());
        p.push(0x09);
        p.extend(1303u16.to_le_bytes());
        p.extend([9, b'V', b'r', b'a', b'i', b'P', b's', b'e', b'u', b'd']);
        p.extend([6, b'L', b'e', b'g', b'i', b'o', b'n']);
        p.extend(0u16.to_le_bytes());
        p.push(0);
        p.extend(0i32.to_le_bytes());
        p.extend([0, 0]);
        let mut a = Anonymizer::new();
        let out = a.packet(&p).unwrap();
        assert!(!out.windows(9).any(|w| w == b"VraiPseud"));
        assert!(!out.windows(6).any(|w| w == b"Legion"));
        match decode(&out) {
            Decoded::Message(crate::protocol::Message::Die(d)) => {
                assert_eq!(d.killer_name, "Anon001");
                assert_eq!(d.killer_guild, "Anon002");
                assert_eq!(d.killer_server, 1303);
            }
            d => panic!("{d:?}"),
        }
        // Même nom, même remplacement.
        let out2 = a.packet(&p).unwrap();
        assert_eq!(out, out2);
    }
}
