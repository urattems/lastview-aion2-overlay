//! Panneau de diagnostic (section 10.2), dans la fenêtre des paramètres :
//! état de la capture, du protocole et de l'API, et enregistrement de
//! 2 minutes de trafic pour analyser un patch du jeu. Les enregistrements
//! restent dans `%APPDATA%\LastView\captures\` : jamais envoyés, jamais dans le repo.

use serde::Serialize;
use tauri::State;

use crate::runtime::Diagnostic;
use crate::AppState;

#[derive(Serialize)]
pub struct Vue {
    #[serde(flatten)]
    moteur: Diagnostic,
    capture: lastview_core::capture::CaptureStatus,
    api_pause_s: Option<u64>,
    api_hors_ligne: bool,
    mode_diagnostic: bool,
}

#[tauri::command]
pub fn diagnostic_etat(state: State<'_, AppState>) -> Vue {
    Vue {
        moteur: state.diag.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone(),
        capture: state.status.lock().unwrap_or_else(std::sync::PoisonError::into_inner).capture.clone(),
        api_pause_s: state.api.paused_for(),
        api_hors_ligne: state.api.hors_ligne(),
        mode_diagnostic: state.params.read().unwrap_or_else(std::sync::PoisonError::into_inner).diagnostic,
    }
}

pub fn dossier_captures() -> std::path::PathBuf {
    lastview_core::store::data_dir().join("captures")
}

/// Enregistre 2 minutes de trafic du jeu. Rend le chemin du fichier.
#[tauri::command]
pub fn enregistrer_trafic(state: State<'_, AppState>) -> Result<String, String> {
    if state.diag.lock().unwrap_or_else(std::sync::PoisonError::into_inner).enregistrement.is_some() {
        return Err("un enregistrement est déjà en cours".into());
    }
    let nom = format!("trafic-{}.txt", lastview_core::capture::now_ms());
    let chemin = dossier_captures().join(nom);
    *state.enregistrer.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((chemin.clone(), 120_000));
    Ok(chemin.display().to_string())
}
