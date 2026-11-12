//! Test de contrat de l'API NCSoft sur Urattems (Vaizel), le perso de Gio.
//! Passe par le réseau : ignoré par défaut. À lancer à la main avant chaque release :
//!
//! ```text
//! cargo test -p lastview-core --test contrat_api -- --ignored
//! ```

use lastview_core::api::ApiClient;

#[test]
#[ignore = "réseau : à lancer à la main avant une release"]
fn contrat_urattems() {
    let c = ApiClient::new(None);
    let hits = c.search("Urattems", Some(1303)).expect("recherche");
    assert_eq!(hits.len(), 1, "un seul Urattems sur Vaizel");
    let h = &hits[0];
    assert!(h.character_id.ends_with('='), "characterId brut attendu : {}", h.character_id);
    assert_eq!(h.server_id, 1303);
    let f = c.fiche(&h.character_id, h.server_id, true);
    let info = f.info.expect("info");
    let s = ApiClient::summary(&info);
    assert_eq!(s.name, "Urattems");
    assert!(s.level >= 1 && s.cp > 0 && s.gs > 0, "profil incomplet : {s:?}");
    assert!(!s.class.is_empty() && !s.server.is_empty());
    let eq = f.equipment.expect("équipement");
    let items = eq.pointer("/equipment/equipmentList").and_then(|l| l.as_array()).expect("equipmentList");
    assert!(items.len() >= 10, "{} pièces seulement", items.len());
    for it in items {
        for champ in ["id", "name", "grade", "enchantLevel", "icon", "slotPos"] {
            assert!(it.get(champ).is_some(), "champ {champ} absent d'une pièce");
        }
    }
    let skills = eq.pointer("/skill/skillList").and_then(|l| l.as_array()).expect("skillList");
    assert!(skills.iter().any(|s| s.get("category").and_then(|c| c.as_str()) == Some("Dp")), "aucun stigma (category Dp)");
}
