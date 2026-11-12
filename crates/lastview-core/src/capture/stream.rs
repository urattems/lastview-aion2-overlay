//! Réassemblage TCP en continu (sens serveur → client) et découpage en paquets.
//!
//! Un flux reçoit ses segments dans l'ordre d'arrivée. Les doublons et
//! chevauchements sont rognés, les segments en avance attendent leur tour.
//! Si un trou ne se comble pas (segment perdu par la capture), on l'abandonne
//! au bout de [`GAP_TIMEOUT_MS`] : le découpage se resynchronise tout seul.

use crate::protocol::framing;

/// Durée d'attente maximale d'un segment manquant.
pub const GAP_TIMEOUT_MS: i64 = 1500;
/// Au-delà, les octets en attente de découpage sont jetés (flux incohérent).
const MAX_BUFFER: usize = 2 * 1024 * 1024;
/// Au-delà, les segments en avance sont jetés.
const MAX_PENDING: usize = 4 * 1024 * 1024;

/// Un paquet complet du jeu (opcode inclus) et l'heure de réception.
#[derive(Debug, Clone)]
pub struct GamePacket {
    pub ts_ms: i64,
    pub data: Vec<u8>,
}

#[derive(Default)]
pub struct Stream {
    next_seq: Option<u32>,
    /// Segments en avance : (séquence, données).
    pending: Vec<(u32, Vec<u8>)>,
    pending_bytes: usize,
    /// Heure d'arrivée du plus ancien segment en avance.
    gap_since: Option<i64>,
    buf: Vec<u8>,
    /// Trous abandonnés (segments jamais reçus).
    pub gaps: u64,
    /// Octets utiles reçus.
    pub bytes: u64,
}

impl Stream {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute un segment ; ajoute à `out` les paquets devenus complets.
    pub fn push(&mut self, ts_ms: i64, seq: u32, data: &[u8], out: &mut Vec<GamePacket>) {
        if data.is_empty() {
            return;
        }
        let next = *self.next_seq.get_or_insert(seq);
        if (seq.wrapping_sub(next) as i32) <= 0 {
            self.accept(seq, data);
            self.drain_pending();
        } else if self.pending_bytes + data.len() <= MAX_PENDING {
            self.pending_bytes += data.len();
            self.pending.push((seq, data.to_vec()));
            self.gap_since.get_or_insert(ts_ms);
        }
        if self.pending.is_empty() {
            self.gap_since = None;
        } else if let Some(since) = self.gap_since {
            if ts_ms - since > GAP_TIMEOUT_MS {
                self.skip_gap(ts_ms);
            }
        }
        self.cut(ts_ms, out);
    }

    /// Ajoute la partie nouvelle d'un segment qui commence au plus tard à `next_seq`.
    fn accept(&mut self, seq: u32, data: &[u8]) {
        let next = self.next_seq.unwrap_or(seq);
        let skip = next.wrapping_sub(seq) as usize;
        if skip < data.len() {
            self.buf.extend_from_slice(&data[skip..]);
            self.bytes += (data.len() - skip) as u64;
            self.next_seq = Some(seq.wrapping_add(data.len() as u32));
        }
    }

    /// Recase les segments en avance devenus contigus.
    fn drain_pending(&mut self) {
        while let Some(next) = self.next_seq {
            let Some(i) = self.pending.iter().position(|(s, _)| (s.wrapping_sub(next) as i32) <= 0) else { break };
            let (seq, data) = self.pending.swap_remove(i);
            self.pending_bytes -= data.len();
            self.accept(seq, &data);
        }
    }

    /// Abandonne le trou : on reprend au segment en avance le plus proche.
    fn skip_gap(&mut self, ts_ms: i64) {
        let Some(next) = self.next_seq else { return };
        let Some(&(seq, _)) = self.pending.iter().min_by_key(|(s, _)| s.wrapping_sub(next)) else { return };
        self.gaps += 1;
        self.buf.clear();
        self.next_seq = Some(seq);
        self.drain_pending();
        self.gap_since = if self.pending.is_empty() { None } else { Some(ts_ms) };
    }

    fn cut(&mut self, ts_ms: i64, out: &mut Vec<GamePacket>) {
        if self.buf.is_empty() {
            return;
        }
        let (packets, consumed) = framing::packets(&self.buf);
        for p in packets {
            out.push(GamePacket { ts_ms, data: p.body });
        }
        self.buf.drain(..consumed.min(self.buf.len()));
        if self.buf.len() > MAX_BUFFER {
            self.buf.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une trame : `<varint len> <payload>`, len = taille du payload + 4.
    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut f = vec![(payload.len() + 4) as u8];
        f.extend_from_slice(payload);
        f
    }

    #[test]
    fn dans_le_desordre_et_doublons() {
        let a = frame(&[0x34, 0x38, 0, 0, 0, 5]);
        let b = frame(&[0x47, 0x36, 7, 0, 0]);
        let mut all = a.clone();
        all.extend(&b);
        let mut out = Vec::new();
        let mid = 4;
        // Début, puis un segment en avance, puis le morceau manquant, puis un doublon.
        let mut s = Stream::new();
        s.push(0, 1000, &all[..mid], &mut out);
        s.push(1, 1000 + mid as u32 + 3, &all[mid + 3..], &mut out);
        s.push(2, 1000 + mid as u32, &all[mid..], &mut out);
        s.push(3, 1000, &all[..mid], &mut out);
        let ops: Vec<_> = out.iter().map(|p| [p.data[0], p.data[1]]).collect();
        assert_eq!(ops, vec![[0x34, 0x38], [0x47, 0x36]]);
        assert_eq!(s.gaps, 0);
    }

    #[test]
    fn trou_abandonne() {
        let a = frame(&[0x34, 0x38, 0, 0, 0, 5]);
        let b = frame(&[0x47, 0x36, 7, 0, 0]);
        let mut s = Stream::new();
        let mut out = Vec::new();
        s.push(0, 1, &a[..3], &mut out);
        // On saute 100 octets : ils n'arriveront jamais.
        s.push(10, 1 + 3 + 100, &b, &mut out);
        assert!(out.is_empty());
        s.push(10 + GAP_TIMEOUT_MS + 1, 1 + 3 + 100 + b.len() as u32, &b, &mut out);
        assert_eq!(s.gaps, 1);
        assert_eq!(out.len(), 2);
        assert_eq!(&out[0].data[..2], &[0x47, 0x36]);
    }
}
