//! Lecture séquentielle stricte d'un paquet, champ par champ.
//!
//! Conventions du jeu (voir `docs/protocol.md`) :
//! - `vi32` / `vi64` : varint LEB128 non signé ;
//! - entiers fixes et flottants en petit-boutiste ;
//! - `str` : longueur en varint puis UTF-8, un `00` final éventuel retiré ;
//! - `bool` : regroupés bit par bit. Le premier `bool` lu réserve un octet,
//!   les 7 suivants lisent ses autres bits, puis un nouvel octet est réservé.
//!
//! Toute lecture au-delà de la fin renvoie [`Short`] : le message est alors
//! ignoré par l'appelant, sans planter.

use std::fmt;

/// Paquet plus court que prévu (ou valeur incohérente).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Short;

impl fmt::Display for Short {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("paquet tronqué ou incohérent")
    }
}

impl std::error::Error for Short {}

pub type R<T> = Result<T, Short>;

/// Au-delà, une liste est considérée comme incohérente.
const MAX_LIST: usize = 4096;

pub struct Reader<'a> {
    b: &'a [u8],
    p: usize,
    bit_at: Option<usize>,
    bit: u8,
    /// Position des chaînes lues (début du varint de longueur, fin), si demandé.
    spans: Option<Vec<(usize, usize)>>,
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, p: 0, bit_at: None, bit: 0, spans: None }
    }

    /// Lecteur qui note la position de chaque chaîne (anonymisation des extraits de test).
    pub fn recording(b: &'a [u8]) -> Self {
        Self { spans: Some(Vec::new()), ..Self::new(b) }
    }

    pub fn take_spans(&mut self) -> Vec<(usize, usize)> {
        self.spans.take().unwrap_or_default()
    }

    pub fn pos(&self) -> usize {
        self.p
    }

    pub fn remaining(&self) -> usize {
        self.b.len() - self.p
    }

    /// Vrai si tout le paquet a été lu, au dernier octet près.
    pub fn is_done(&self) -> bool {
        self.p == self.b.len()
    }

    fn take(&mut self, n: usize) -> R<&'a [u8]> {
        if self.p + n > self.b.len() {
            return Err(Short);
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }

    pub fn skip(&mut self, n: usize) -> R<()> {
        self.take(n).map(|_| ())
    }

    pub fn u8(&mut self) -> R<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> R<u16> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }

    pub fn u32(&mut self) -> R<u32> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    pub fn i32(&mut self) -> R<i32> {
        Ok(self.u32()? as i32)
    }

    pub fn i64(&mut self) -> R<i64> {
        let s = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Ok(i64::from_le_bytes(a))
    }

    pub fn f32(&mut self) -> R<f32> {
        Ok(f32::from_bits(self.u32()?))
    }

    pub fn vec3(&mut self) -> R<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    /// Varint LEB128 (jusqu'à 64 bits).
    pub fn vi64(&mut self) -> R<u64> {
        let mut v: u64 = 0;
        let mut shift = 0u32;
        loop {
            let x = self.u8()?;
            if shift >= 64 {
                return Err(Short);
            }
            v |= ((x & 0x7F) as u64) << shift;
            if x & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
        }
    }

    /// Varint LEB128 sur 32 bits (ids d'entité, compteurs).
    pub fn vi32(&mut self) -> R<u32> {
        let v = self.vi64()?;
        u32::try_from(v).map_err(|_| Short)
    }

    pub fn str(&mut self) -> R<String> {
        let start = self.p;
        let n = self.vi32()? as usize;
        let s = self.take(n)?;
        if let Some(spans) = self.spans.as_mut() {
            spans.push((start, self.p));
        }
        let s = s.strip_suffix(&[0]).unwrap_or(s);
        Ok(String::from_utf8_lossy(s).into_owned())
    }

    pub fn bool(&mut self) -> R<bool> {
        let at = match self.bit_at {
            Some(at) => at,
            None => {
                let at = self.p;
                self.skip(1)?;
                self.bit_at = Some(at);
                self.bit = 0;
                at
            }
        };
        let v = (self.b[at] >> self.bit) & 1 == 1;
        self.bit += 1;
        if self.bit > 7 {
            self.bit_at = None;
            self.bit = 0;
        }
        Ok(v)
    }

    /// Liste : nombre d'éléments en varint, puis les éléments.
    pub fn list<T>(&mut self, mut f: impl FnMut(&mut Self) -> R<T>) -> R<Vec<T>> {
        let n = self.vi32()? as usize;
        if n > MAX_LIST || n > self.remaining().saturating_mul(8) + 64 {
            return Err(Short);
        }
        let mut out = Vec::with_capacity(n.min(256));
        for _ in 0..n {
            out.push(f(self)?);
        }
        Ok(out)
    }

    /// Champ présent seulement si `cond`.
    pub fn opt<T>(&mut self, cond: bool, f: impl FnOnce(&mut Self) -> R<T>) -> R<Option<T>> {
        if cond {
            f(self).map(Some)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_et_entiers() {
        let b = [0xAC, 0x02, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12];
        let mut r = Reader::new(&b);
        assert_eq!(r.vi32().unwrap(), 300);
        assert_eq!(r.u16().unwrap(), 0x1234);
        assert_eq!(r.u32().unwrap(), 0x1234_5678);
        assert!(r.is_done());
        assert_eq!(r.u8(), Err(Short));
    }

    #[test]
    fn chaine_avec_zero_final() {
        let b = [4, b'A', b'b', b'c', 0];
        let mut r = Reader::new(&b);
        assert_eq!(r.str().unwrap(), "Abc");
        assert!(r.is_done());
    }

    #[test]
    fn booleens_groupes() {
        // 1er bool réserve l'octet 0b101, un u8 s'intercale, les bools suivants lisent les bits 1 et 2.
        let b = [0b0000_0101, 0x7F];
        let mut r = Reader::new(&b);
        assert!(r.bool().unwrap());
        assert_eq!(r.u8().unwrap(), 0x7F);
        assert!(!r.bool().unwrap());
        assert!(r.bool().unwrap());
        assert!(r.is_done());
    }

    #[test]
    fn booleens_au_dela_de_8() {
        let b = [0xFF, 0x00];
        let mut r = Reader::new(&b);
        for _ in 0..8 {
            assert!(r.bool().unwrap());
        }
        assert!(!r.bool().unwrap());
        assert!(r.is_done());
    }

    #[test]
    fn liste_incoherente_refusee() {
        let b = [0xFF, 0xFF, 0x03];
        let mut r = Reader::new(&b);
        assert!(r.list(|r| r.u8()).is_err());
    }
}
