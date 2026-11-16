//! Veille du protocole (section 10.1) sur un vrai enregistrement (extrait
//! anonymisé `porte5`) étalé sur 3 minutes :
//! - intact, les apparitions sont décodées : aucune alerte ;
//! - altéré comme après un patch du jeu (opcodes d'apparition changés) :
//!   « Protocole à mettre à jour » après 2 minutes, sans plantage.

use lastview_core::capture::GamePacket;
use lastview_core::protocol::opcodes;
use lastview_core::veille::{VeilleProtocole, DELAI_MS};
use lastview_core::{fixtures, Engine};

fn rejouer(alterer: bool) -> (bool, u64, Option<i64>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/porte5.txt");
    let paquets = fixtures::parse(&std::fs::read_to_string(path).expect("extrait"));
    let n = paquets.len() as i64;
    let mut moteur = Engine::new();
    let mut veille = VeilleProtocole::new();
    let mut alerte_a = None;
    for (i, (_, mut data)) in paquets.into_iter().enumerate() {
        if alterer && data.len() >= 2 && ([data[0], data[1]] == opcodes::PLAYER_APPEAR || [data[0], data[1]] == opcodes::NPC_APPEAR) {
            // Patch simulé : l'opcode a changé, plus rien n'est reconnu comme apparition.
            data[0] = 0xEE;
            data[1] = 0xEE;
        }
        let ts = 1_700_000_000_000 + i as i64 * 180_000 / n;
        moteur.handle(&GamePacket { ts_ms: ts, data });
        if veille.observer(ts, moteur.stats.packets, moteur.stats.spawns) && alerte_a.is_none() {
            alerte_a = Some(ts - 1_700_000_000_000);
        }
    }
    (veille.alerte(), moteur.stats.spawns, alerte_a)
}

#[test]
fn enregistrement_intact_sans_alerte() {
    let (alerte, spawns, _) = rejouer(false);
    assert!(spawns > 0, "l'extrait contient des apparitions");
    assert!(!alerte);
}

#[test]
fn enregistrement_altere_protocole_a_mettre_a_jour() {
    let (alerte, spawns, quand) = rejouer(true);
    assert_eq!(spawns, 0);
    assert!(alerte, "alerte attendue après 2 min sans apparition");
    let quand = quand.unwrap();
    assert!((DELAI_MS..DELAI_MS + 2_000).contains(&quand), "alerte à {quand} ms");
}
