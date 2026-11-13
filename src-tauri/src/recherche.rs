//! Recherche par pseudo (section 7) : suggestions pendant la frappe.
//! D'abord la mémoire silencieuse et la liste (instantané, sans réseau), puis
//! l'API, région EU. Une recherche n'ajoute jamais le joueur à la liste.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use lastview_core::capture::now_ms;
use lastview_core::protocol::class_name;
use lastview_core::store::DEFAULT_RETENTION_MS;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Suggestion {
    nom: String,
    serveur_id: u16,
    serveur: String,
    classe: Option<&'static str>,
    niveau: i64,
    /// `memoire`, `liste` ou `api`.
    source: &'static str,
}

/// Noms des serveurs EU (API `gameinfo/servers`, en cache 24 h), chargés une fois.
fn nom_serveur(state: &AppState, id: u16) -> String {
    static NOMS: OnceLock<Mutex<HashMap<u16, String>>> = OnceLock::new();
    let noms = NOMS.get_or_init(Default::default);
    if noms.lock().unwrap().is_empty() {
        if let Ok(v) = state.api.servers() {
            let mut m = noms.lock().unwrap();
            for s in v.get("serverList").and_then(|l| l.as_array()).into_iter().flatten() {
                if let (Some(id), Some(n)) = (s.get("serverId").and_then(|x| x.as_u64()), s.get("serverName").and_then(|x| x.as_str())) {
                    m.insert(id as u16, n.to_string());
                }
            }
        }
    }
    noms.lock().unwrap().get(&id).cloned().unwrap_or_else(|| if id == 0 { String::new() } else { format!("serveur {id}") })
}

fn correspond(nom: &str, t: &str) -> Option<u8> {
    let n = nom.to_lowercase();
    if n == t {
        Some(0)
    } else if n.starts_with(t) {
        Some(1)
    } else if n.contains(t) {
        Some(2)
    } else {
        None
    }
}

/// Suggestions locales (mémoire silencieuse et liste), sans réseau.
#[tauri::command]
pub async fn recherche_locale(state: State<'_, AppState>, texte: String) -> Result<Vec<Suggestion>, String> {
    let t = texte.trim().to_lowercase();
    if t.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let mut vus: HashMap<(String, u16), (u8, Suggestion)> = HashMap::new();
    let rows = state.store.lock().unwrap().list(now_ms(), DEFAULT_RETENTION_MS).map_err(|e| e.to_string())?;
    for r in rows {
        if let Some(rang) = correspond(&r.name, &t) {
            let s = Suggestion { nom: r.name.clone(), serveur_id: r.server, serveur: String::new(), classe: r.class, niveau: r.level as i64, source: "liste" };
            vus.insert((r.name.to_lowercase(), r.server), (rang, s));
        }
    }
    for m in state.memoire.lock().unwrap().iter() {
        if let Some(rang) = correspond(&m.nom, &t) {
            vus.entry((m.nom.to_lowercase(), m.serveur))
                .or_insert((rang, Suggestion { nom: m.nom.clone(), serveur_id: m.serveur, serveur: String::new(), classe: m.classe, niveau: m.niveau as i64, source: "memoire" }));
        }
    }
    let mut out: Vec<(u8, Suggestion)> = vus.into_values().collect();
    out.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.nom.len().cmp(&b.1.nom.len())));
    out.truncate(8);
    let st = state.inner();
    Ok(out
        .into_iter()
        .map(|(_, mut s)| {
            s.serveur = nom_serveur(st, s.serveur_id);
            s
        })
        .collect())
}

/// Suggestions de l'API NCSoft (région EU), après 300 ms sans frappe côté front.
#[tauri::command]
pub async fn recherche_api(state: State<'_, AppState>, texte: String) -> Result<Vec<Suggestion>, String> {
    let t = texte.trim().to_string();
    if t.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let api = state.api.clone();
    let hits = tauri::async_runtime::spawn_blocking(move || api.search_partiel(&t, 10)).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    Ok(hits
        .into_iter()
        .map(|h| Suggestion { nom: h.name, serveur_id: h.server_id, serveur: h.server_name, classe: class_name(h.pc_id as i32), niveau: h.level, source: "api" })
        .collect())
}

/// Ouvre la fiche d'un joueur trouvé par la recherche (badge « Recherche »).
#[tauri::command]
pub fn ouvrir_fiche_nom(app: AppHandle, nom: String, serveur: Option<u16>) {
    log::info!("fiche demandée par la recherche : {nom}");
    if let Some(w) = app.get_webview_window("fiche") {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = app.emit_to("fiche", "fiche-demandee-nom", serde_json::json!({ "nom": nom, "serveur": serveur }));
    }
}

#[cfg(test)]
mod tests {
    use super::correspond;

    #[test]
    fn classement_exact_prefixe_contenu() {
        assert_eq!(correspond("Urattems", "urattems"), Some(0));
        assert_eq!(correspond("Urattems", "urat"), Some(1));
        assert_eq!(correspond("Yasuratta", "urat"), Some(2));
        assert_eq!(correspond("Riper", "urat"), None);
    }
}
