//! Robustesse : des milliers de suites d'octets aléatoires ou tronquées
//! envoyées au décodeur, au découpage et au réassemblage. Aucun plantage
//! toléré : un paquet illisible doit être ignoré, jamais faire tomber l'app.

use lastview_core::capture::{Assembler, Segment};
use lastview_core::protocol::{decode, framing, opcodes};
use lastview_core::{fixtures, Engine};

/// Générateur pseudo-aléatoire déterministe (xorshift) : les échecs se rejouent.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() as u8).collect()
    }
}

const OPCODES: &[opcodes::Opcode] = &[
    opcodes::TARGET_SELF,
    opcodes::TARGET_OTHER,
    opcodes::DIE,
    opcodes::PLAYER_APPEAR,
    opcodes::PLAYER_DISAPPEAR,
    opcodes::NPC_APPEAR,
    opcodes::SKILL_EFFECT,
    opcodes::ABNORMAL_ADD,
    opcodes::SELF_APPEAR,
    opcodes::SELF_STATS,
    opcodes::SELF_CP,
    opcodes::LEVEL_UPDATE,
    opcodes::GUILD_UPDATE,
    opcodes::PARTY_INFO,
    opcodes::PARTY_ADD_MEMBER,
    opcodes::PARTY_RECONNECT_MEMBER,
    opcodes::PARTY_UPDATE_MEMBER,
    opcodes::PARTY_LEAVE_SELF,
    opcodes::PARTY_KICKOUT,
    opcodes::PARTY_DISBAND,
    opcodes::PARTY_LEAVE,
    opcodes::FORCE_ADD_MEMBER,
    opcodes::FORCE_RECONNECT_MEMBER,
];

fn vrais_paquets() -> Vec<Vec<u8>> {
    let mut v = Vec::new();
    for nom in ["ciblage1", "ciblage3", "ciblage4"] {
        let path = format!("{}/tests/fixtures/{nom}.txt", env!("CARGO_MANIFEST_DIR"));
        v.extend(fixtures::parse(&std::fs::read_to_string(path).unwrap()).into_iter().map(|(_, p)| p));
    }
    v
}

#[test]
fn decodeur_octets_aleatoires() {
    let mut rng = Rng(0x5EED_1234_ABCD_0001);
    let mut engine = Engine::new();
    for i in 0..200_000 {
        let op = OPCODES[rng.below(OPCODES.len())];
        let len = rng.below(400);
        let mut p = op.to_vec();
        p.extend(rng.bytes(len));
        let _ = decode(&p);
        // Une partie passe aussi par le moteur entier.
        if i % 10 == 0 {
            let _ = engine.handle(&lastview_core::capture::GamePacket { ts_ms: i as i64, data: p });
        }
    }
}

#[test]
fn vrais_paquets_tronques_et_alteres() {
    let mut rng = Rng(0xC0FF_EE00_0000_0002);
    for p in vrais_paquets() {
        // Toutes les longueurs tronquées.
        for n in 0..p.len() {
            let _ = decode(&p[..n]);
        }
        // Quelques octets modifiés au hasard.
        for _ in 0..20 {
            let mut q = p.clone();
            for _ in 0..1 + rng.below(4) {
                let i = 2 + rng.below(q.len().saturating_sub(2));
                if i < q.len() {
                    q[i] = rng.next() as u8;
                }
            }
            let _ = decode(&q);
        }
    }
}

#[test]
fn decoupage_aleatoire() {
    let mut rng = Rng(0xF00D_0000_0000_0003);
    for _ in 0..20_000 {
        let n = rng.below(3000);
        let mut b = rng.bytes(n);
        // Parfois un en-tête de lot plausible, pour exercer la décompression.
        if n > 10 && rng.below(4) == 0 {
            let at = rng.below(n - 8);
            b[at] = 0x20;
            b[at + 1] = 0xFF;
            b[at + 2] = 0xFF;
        }
        let _ = framing::packets(&b);
    }
}

#[test]
fn reassemblage_aleatoire() {
    let mut rng = Rng(0xBEEF_0000_0000_0004);
    let mut asm = Assembler::new();
    let mut out = Vec::new();
    let mut ts = 0i64;
    for _ in 0..50_000 {
        ts += rng.below(200) as i64;
        let flow = rng.below(4) as u16;
        let seq = rng.next() as u32;
        let len = rng.below(1500);
        let seg = Segment { ts_ms: ts, key: (1, 13328 + flow, 2, 50000), seq, data: rng.bytes(len), known: rng.below(2) == 0 };
        asm.push(&seg, &mut out);
        out.clear();
    }
}
