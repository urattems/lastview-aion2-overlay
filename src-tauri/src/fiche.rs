//! La fiche joueur (section 6) côté Rust : ouverture (recherche, profil,
//! équipement en une opération), détail d'une pièce, lien Shugo.
//! Les appels réseau tournent hors du fil de l'interface.

use std::sync::{Arc, Mutex};

use lastview_core::api::{self, ApiClient, ApiError, SearchHit};
use lastview_core::store::{Row, Store};
use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::AppState;

#[derive(Serialize)]
pub struct FicheReponse {
    /// Ligne de la liste si la fiche vient de la bulle (rencontre, ancienneté).
    joueur: Option<Row>,
    /// Homonymes, si le serveur n'était pas connu.
    candidats: Vec<SearchHit>,
    character_id: Option<String>,
    serveur: Option<u16>,
    info: Option<Value>,
    equipement: Option<Value>,
    /// Date de la donnée (ms), pour « Données API il y a … ».
    donnees_ms: Option<i64>,
    erreur: Option<String>,
    /// Pause en cours (403/429), en secondes.
    pause_s: Option<u64>,
}

fn texte(e: &ApiError) -> String {
    e.to_string()
}

/// Ouvre une fiche : par l'id d'une ligne de la liste, ou par pseudo (+ serveur).
#[tauri::command]
pub async fn fiche_ouvrir(state: State<'_, AppState>, id: Option<i64>, nom: Option<String>, serveur: Option<u16>, actualiser: bool) -> Result<FicheReponse, String> {
    let (store, api) = (state.store.clone(), state.api.clone());
    tauri::async_runtime::spawn_blocking(move || ouvrir(&store, &api, id, nom, serveur, actualiser)).await.map_err(|e| e.to_string())
}

fn ouvrir(store: &Arc<Mutex<Store>>, api: &ApiClient, id: Option<i64>, nom: Option<String>, serveur: Option<u16>, actualiser: bool) -> FicheReponse {
    let joueur = id.and_then(|i| store.lock().unwrap().row(i).ok().flatten());
    let nom = joueur.as_ref().map(|r| r.name.clone()).or(nom).unwrap_or_default();
    let serveur = joueur.as_ref().map(|r| r.server).filter(|s| *s != 0).or(serveur);
    let (pc_id, niveau) = joueur.as_ref().map(|r| (r.pc_id, r.level)).unwrap_or((0, 0));
    let cache_id = joueur.as_ref().and_then(|r| r.character_id.clone()).filter(|c| !c.is_empty()).zip(serveur);
    let mut rep = FicheReponse { joueur, candidats: Vec::new(), character_id: None, serveur, info: None, equipement: None, donnees_ms: None, erreur: None, pause_s: None };
    if nom.is_empty() {
        rep.erreur = Some("Aucun pseudo".into());
        return rep;
    }
    let o = api.ouvrir(&nom, serveur, pc_id, niveau, cache_id.as_ref().map(|(c, s)| (c.as_str(), *s)), actualiser);
    rep.candidats = o.candidats;
    if let Some(row) = &rep.joueur {
        let s = store.lock().unwrap();
        if o.id_en_cache_invalide {
            let _ = s.set_character_id(row.id, "");
        }
        if let Some((cid, _)) = &o.choisi {
            let _ = s.set_character_id(row.id, cid);
        }
    }
    if let Some((cid, srv)) = &o.choisi {
        rep.character_id = Some(cid.clone());
        rep.serveur = Some(*srv);
        rep.donnees_ms = api.fetched_at(&ApiClient::info_url(cid, *srv));
    }
    if let Some(e) = o.erreur {
        rep.erreur = Some(texte(&e));
    }
    if let Some(f) = o.fiche {
        match f.info {
            Ok(v) => {
                api::signaler_champs_inconnus("info", &v);
                if let Some(p) = v.get("profile") {
                    api::signaler_champs_inconnus("profil", p);
                }
                rep.info = Some(v);
            }
            Err(e) => rep.erreur = Some(texte(&e)),
        }
        match f.equipment {
            Ok(v) => {
                api::signaler_champs_inconnus("equipement", &v);
                if let Some(p) = v.pointer("/equipment/equipmentList/0") {
                    api::signaler_champs_inconnus("piece_liste", p);
                }
                rep.equipement = Some(v);
            }
            Err(e) => {
                rep.erreur.get_or_insert_with(|| texte(&e));
            }
        }
    }
    rep.pause_s = api.paused_for();
    rep
}

/// Détail d'une pièce (infobulle), à la demande. `niveau` = enchant + dépassement.
#[tauri::command]
pub async fn fiche_piece(state: State<'_, AppState>, id: i64, niveau: i64, emplacement: i64, character_id: String, serveur: u16) -> Result<Value, String> {
    let api = state.api.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let v = api.item(id, niveau, emplacement, &character_id, serveur).map_err(|e| e.to_string())?;
        api::signaler_champs_inconnus("piece", &v);
        Ok(v)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// « Shugo.GG ↗ » : la page du perso dans le navigateur par défaut.
/// Seules les adresses shugo.gg sont ouvertes.
#[tauri::command]
pub fn ouvrir_shugo(nom: String, serveur: String, race_id: i64) -> Result<(), String> {
    let faction = if race_id == 2 { "as" } else { "el" };
    let enc = |s: &str| percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string();
    let url = format!("https://shugo.gg/fr/character/global/{faction}-{}/{}", enc(&serveur.to_lowercase()), enc(&nom));
    std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn().map(|_| ()).map_err(|e| e.to_string())
}
