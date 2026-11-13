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
    /// CP à afficher (CP de l'API, ou dernier CP réel s'il est normalisé).
    cp: Option<lastview_core::cp::CpAffiche>,
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
    let mut rep = FicheReponse { joueur, candidats: Vec::new(), character_id: None, serveur, info: None, equipement: None, donnees_ms: None, erreur: None, pause_s: None, cp: None };
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
                rep.cp = cp_de_l_api(store, &v, rep.joueur.as_ref());
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
pub fn ouvrir_shugo(nom: String, serveur: String) -> Result<(), String> {
    let url = api::shugo_url(&nom, &serveur);
    std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Détail d'un plateau Daevanion, à l'ouverture de l'onglet ou d'un plateau (6.5).
#[tauri::command]
pub async fn fiche_daevanion(state: State<'_, AppState>, character_id: String, serveur: u16, plateau: i64) -> Result<Value, String> {
    let api = state.api.clone();
    tauri::async_runtime::spawn_blocking(move || api.daevanion(&character_id, serveur, plateau).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// « Enregistrer en PNG » (clic droit sur « Copier en image ») : dans
/// `Images\LastView`, nommé d'après le perso et l'heure. Renvoie le chemin.
#[tauri::command]
pub fn enregistrer_png(state: State<'_, AppState>, nom: String, octets: Vec<u8>) -> Result<String, String> {
    if octets.len() < 8 || octets[..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return Err("image PNG invalide".into());
    }
    // `LASTVIEW_DOSSIER_PNG` : dossier imposé (tests), jamais les dossiers personnels de Gio.
    // Sinon le dossier choisi dans les paramètres, ou Images\LastView.
    let choisi = state.params.read().unwrap().dossier_png.clone();
    let dossier = match std::env::var_os("LASTVIEW_DOSSIER_PNG") {
        Some(d) => std::path::PathBuf::from(d),
        None if !choisi.is_empty() => std::path::PathBuf::from(choisi),
        None => crate::parametres::dossier_png_defaut(),
    };
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    let propre: String = nom.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_').take(40).collect();
    let fichier = dossier.join(format!("LastView-{}-{}.png", if propre.is_empty() { "fiche" } else { &propre }, lastview_core::capture::now_ms()));
    std::fs::write(&fichier, &octets).map_err(|e| e.to_string())?;
    log::info!("fiche enregistrée en PNG : {}", fichier.display());
    Ok(fichier.display().to_string())
}

/// CP de l'API passé par la règle de normalisation (Abysses) : un CP normalisé
/// n'est jamais retenu comme réel ; la fiche montre le dernier CP réel connu.
/// Avertit en diagnostic si le CP des paquets et celui de l'API diffèrent de plus de 5 %.
fn cp_de_l_api(store: &Arc<Mutex<Store>>, info: &Value, joueur: Option<&Row>) -> Option<lastview_core::cp::CpAffiche> {
    let s = ApiClient::summary(info);
    if s.cp <= 0 {
        return None;
    }
    let st = store.lock().unwrap();
    let verdict = st.observer_cp(&s.name, s.server_id as u16, s.cp, s.gs, lastview_core::capture::now_ms()).ok()?;
    if let Some(r) = joueur.filter(|r| r.cp > 0) {
        let ecart = (r.cp - s.cp).abs() as f64 / r.cp.max(1) as f64;
        if ecart > 0.05 {
            log::warn!(
                "CP de {} : paquets {}{} / API {}{} ({:.0} % d'écart)",
                s.name,
                r.cp,
                if r.cp_normalise { " (normalisé)" } else { "" },
                s.cp,
                if verdict.normalise { " (normalisé)" } else { "" },
                ecart * 100.0
            );
        }
    }
    Some(lastview_core::cp::cp_fiche(s.cp, verdict.normalise, verdict.reel))
}
