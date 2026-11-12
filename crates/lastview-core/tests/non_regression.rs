//! Non-régression sur de vrais paquets (extraits anonymisés de nos
//! enregistrements, `tests/fixtures/`). C'est le filet de sécurité de chaque
//! patch du jeu : si un opcode ou une structure change, ce test casse.
//!
//! Chaque extrait est rejoué dans toute la chaîne, en conditions difficiles :
//! paquets regroupés en lots LZ4, flux découpé en segments TCP dont certains
//! arrivent dans le désordre ou en double, numéros de séquence qui repassent
//! par zéro. Puis décodage, mémoire, rencontres et base.

use std::collections::BTreeMap;

use lastview_core::capture::{Assembler, Segment};
use lastview_core::fixtures;
use lastview_core::store::{Store, DEFAULT_RETENTION_MS};
use lastview_core::Engine;

fn varint(mut v: usize, out: &mut Vec<u8>) {
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

/// `<varint len> <payload>` avec len = taille du payload + 4.
fn frame(payload: &[u8], out: &mut Vec<u8>) {
    varint(payload.len() + 4, out);
    out.extend_from_slice(payload);
}

/// Le flux serveur → client : un paquet sur trois isolé, les autres en lots LZ4 de 5.
fn stream(packets: &[(i64, Vec<u8>)]) -> (Vec<u8>, Vec<(usize, i64)>) {
    let mut out = Vec::new();
    let mut marks = Vec::new();
    let mut i = 0;
    while i < packets.len() {
        marks.push((out.len(), packets[i].0));
        if i % 3 == 0 {
            frame(&packets[i].1, &mut out);
            i += 1;
        } else {
            let end = (i + 5).min(packets.len());
            let mut inner = Vec::new();
            for p in &packets[i..end] {
                frame(&p.1, &mut inner);
            }
            let mut payload = vec![0xFF, 0xFF];
            payload.extend((inner.len() as u32).to_le_bytes());
            payload.extend(lz4_flex::compress(&inner));
            frame(&payload, &mut out);
            i = end;
        }
    }
    (out, marks)
}

/// Découpe en segments de 900 octets, avec désordre, doublons et passage de la séquence par zéro.
fn segments(bytes: &[u8], marks: &[(usize, i64)]) -> Vec<Segment> {
    let ts_at = |off: usize| match marks.binary_search_by_key(&off, |m| m.0) {
        Ok(i) => marks[i].1,
        Err(0) => marks[0].1,
        Err(i) => marks[i - 1].1,
    };
    let seq0: u32 = 0xFFFF_F000;
    let key = (0x0A00_0001, 13328, 0xC0A8_0102, 50000);
    let mut segs: Vec<Segment> = bytes
        .chunks(900)
        .enumerate()
        .map(|(i, c)| Segment { ts_ms: ts_at(i * 900), key, seq: seq0.wrapping_add((i * 900) as u32), data: c.to_vec(), known: true })
        .collect();
    let mut i = 3;
    while i + 1 < segs.len() {
        segs.swap(i, i + 1);
        // L'heure d'arrivée suit l'ordre d'arrivée.
        let t = segs[i + 1].ts_ms;
        segs[i].ts_ms = segs[i].ts_ms.min(t);
        i += 7;
    }
    let mut out = Vec::new();
    for (i, s) in segs.into_iter().enumerate() {
        if i % 11 == 5 {
            out.push(s.clone());
        }
        out.push(s);
    }
    out
}

struct Resultat {
    paquets: usize,
    decodes: BTreeMap<String, u64>,
    illisibles: BTreeMap<String, u64>,
    rencontres: Vec<(String, String)>,
    lignes: usize,
    moi_devine: bool,
    moi_confirme: bool,
    moi_cp: i64,
    trous: u64,
}

fn rejouer(nom: &str) -> (Vec<(i64, Vec<u8>)>, Resultat) {
    let path = format!("{}/tests/fixtures/{nom}.txt", env!("CARGO_MANIFEST_DIR"));
    let packets = fixtures::parse(&std::fs::read_to_string(&path).expect("extrait"));
    assert!(!packets.is_empty());
    let (bytes, marks) = stream(&packets);
    let mut asm = Assembler::new();
    let mut engine = Engine::new();
    let mut store = Store::open_in_memory().unwrap();
    let mut out = Vec::new();
    let mut n = 0;
    let mut rencontres = Vec::new();
    let mut last_ts = 0;
    for s in segments(&bytes, &marks) {
        asm.push(&s, &mut out);
        for (_, p) in out.drain(..) {
            n += 1;
            last_ts = p.ts_ms;
            for e in engine.handle(&p) {
                rencontres.push((e.kind.as_str().to_string(), e.player.name.clone()));
                store.record(&e).unwrap();
            }
        }
    }
    let lignes = store.list(last_ts, DEFAULT_RETENTION_MS).unwrap().len();
    (
        packets,
        Resultat {
            paquets: n,
            decodes: engine.stats.decoded.clone(),
            illisibles: engine.stats.unreadable.clone(),
            rencontres,
            lignes,
            moi_devine: engine.world.local.key.is_some(),
            moi_confirme: engine.world.local.confirmed,
            moi_cp: engine.world.local.cp,
            trous: asm.gaps(),
        },
    )
}

fn par_opcode(packets: &[(i64, Vec<u8>)]) -> BTreeMap<String, u64> {
    let mut m = BTreeMap::new();
    for (_, p) in packets {
        *m.entry(format!("{:02X} {:02X}", p[0], p[1])).or_default() += 1;
    }
    m
}

/// (ciblages, morts, buffs, apparitions)
fn comptes(r: &Resultat) -> (u64, u64, u64, u64) {
    let g = |k: &str| r.decodes.get(k).copied().unwrap_or(0);
    (g("34 38"), g("04 8D"), g("2A 38"), g("45 36"))
}

#[test]
fn ciblage1() {
    let (packets, r) = rejouer("ciblage1");
    assert_eq!(r.paquets, packets.len(), "aucun paquet perdu ni inventé");
    assert!(r.illisibles.is_empty(), "illisibles : {:?}", r.illisibles);
    assert_eq!(r.trous, 0);
    assert_eq!(r.decodes, par_opcode(&packets));
    assert_eq!(comptes(&r), (9, 45, 118, 14));
    assert!(r.moi_devine);
    // Trois ciblages de joueurs nommés (le même joueur deux fois), deux lignes en base.
    assert_eq!(r.rencontres.len(), 3, "{:?}", r.rencontres);
    assert!(r.rencontres.iter().all(|(k, n)| k == "cible" && n.starts_with("Anon")));
    assert_eq!(r.rencontres[0].1, r.rencontres[1].1);
    assert_eq!(r.lignes, 2);
}

#[test]
fn ciblage3() {
    let (packets, r) = rejouer("ciblage3");
    assert_eq!(r.paquets, packets.len());
    assert!(r.illisibles.is_empty(), "illisibles : {:?}", r.illisibles);
    assert_eq!(r.trous, 0);
    assert_eq!(r.decodes, par_opcode(&packets));
    assert_eq!(comptes(&r), (13, 0, 57, 42));
    assert!(r.moi_devine);
    assert_eq!(r.rencontres.len(), 2, "{:?}", r.rencontres);
    assert_eq!(r.lignes, 1);
}

#[test]
fn ciblage4() {
    let (packets, r) = rejouer("ciblage4");
    assert_eq!(r.paquets, packets.len());
    assert!(r.illisibles.is_empty(), "illisibles : {:?}", r.illisibles);
    assert_eq!(r.trous, 0);
    assert_eq!(r.decodes, par_opcode(&packets));
    assert_eq!(comptes(&r), (13, 0, 38, 50));
    assert!(r.moi_devine);
    // Les joueurs ciblés étaient à l'écran avant l'enregistrement : aucun pseudo, aucune rencontre.
    assert!(r.rencontres.is_empty(), "{:?}", r.rencontres);
}

/// Séance de la porte 5 (12/11/2026) : groupe de donjon déjà formé au
/// lancement, ta fiche `33 36`, PvP contre des joueurs d'un autre serveur et
/// une mort. Deux fenêtres de la séance : 0-160 s et 990-1100 s.
#[test]
fn porte5_groupe_fiche_pvp() {
    use std::collections::BTreeSet;
    let (packets, r) = rejouer("porte5");
    assert_eq!(r.paquets, packets.len());
    assert!(r.illisibles.is_empty(), "illisibles : {:?}", r.illisibles);
    assert_eq!(r.trous, 0);
    assert_eq!(r.decodes, par_opcode(&packets));
    // Ta fiche, lue jusqu'au CP.
    assert_eq!(r.decodes.get("33 36"), Some(&9));
    assert!(r.moi_confirme, "joueur local non confirmé par 33 36");
    assert!(r.moi_cp > 100_000, "CP local : {}", r.moi_cp);
    let noms = |k: &str| r.rencontres.iter().filter(|(kk, _)| kk == k).map(|(_, n)| n.clone()).collect::<BTreeSet<_>>();
    // Groupe formé avant l'enregistrement : les 4 membres reconnus par leur dbid.
    let groupe = noms("groupe");
    assert_eq!(groupe.len(), 4, "groupe : {groupe:?}");
    // PvP : des combats, jamais avec un membre du groupe (soins et buffs exclus).
    let combat = noms("combat");
    assert!(!combat.is_empty(), "aucun combat");
    assert!(combat.is_disjoint(&groupe), "membre du groupe compté en Combat : {:?}", combat.intersection(&groupe).collect::<Vec<_>>());
    // Tué par un joueur adverse.
    let morts = noms("mort");
    assert!(!morts.is_empty() && morts.is_disjoint(&groupe), "morts : {morts:?}");
}
