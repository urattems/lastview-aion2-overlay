//! Mise à jour automatique (section 9.5) : plugin updater de Tauri, alimenté
//! par les Releases GitHub publiques (`latest.json`), signées par la clé de
//! l'updater (seule la clé publique est dans la configuration).
//!
//! Vérification une minute après le démarrage, puis toutes les 6 h, si le
//! paramètre est actif. La mise à jour est téléchargée en tâche de fond puis
//! **proposée**, jamais forcée : installation sur clic (zone de notification)
//! ou à la fermeture de LastView, et jamais pendant que le jeu tourne.

use std::sync::Mutex;
use std::time::Duration;

use lastview_core::capture::system;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::AppState;

/// Mise à jour téléchargée, prête à installer.
static PRETE: Mutex<Option<(Update, Vec<u8>)>> = Mutex::new(None);
/// Élément du menu de la zone de notification.
pub static ENTREE_MENU: std::sync::OnceLock<MenuItem<Wry>> = std::sync::OnceLock::new();

const PREMIERE: Duration = Duration::from_secs(60);
const INTERVALLE: Duration = Duration::from_secs(6 * 3600);

fn libelle(texte: &str) {
    if let Some(m) = ENTREE_MENU.get() {
        let _ = m.set_text(texte);
    }
}

/// Lance la vérification périodique (version installée ou portable seulement).
pub fn demarrer(app: AppHandle) {
    if !crate::parametres::version_installee() {
        log::info!("mise à jour automatique : coupée pour une compilation locale (dossier target)");
        libelle("Mise à jour : compilation locale");
        if let Some(m) = ENTREE_MENU.get() {
            let _ = m.set_enabled(false);
        }
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio_sleep(PREMIERE).await;
        loop {
            let actif = app.state::<AppState>().params.read().unwrap().maj_auto;
            if actif {
                verifier(&app, false).await;
            }
            tokio_sleep(INTERVALLE).await;
        }
    });
}

async fn tokio_sleep(d: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

/// Cherche une mise à jour et la télécharge. `manuel` : demandé depuis le menu.
pub async fn verifier(app: &AppHandle, manuel: bool) {
    if PRETE.lock().unwrap().is_some() {
        return;
    }
    libelle("Recherche d'une mise à jour…");
    let resultat = match app.updater() {
        Ok(u) => u.check().await,
        Err(e) => Err(e),
    };
    match resultat {
        Ok(Some(update)) => {
            let version = update.version.clone();
            log::info!("mise à jour {version} disponible, téléchargement");
            libelle(&format!("Téléchargement de la version {version}…"));
            match update.download(|_, _| {}, || {}).await {
                Ok(octets) => {
                    *PRETE.lock().unwrap() = Some((update, octets));
                    libelle(&format!("Installer la version {version}"));
                    let _ = app
                        .notification()
                        .builder()
                        .title("LastView")
                        .body(format!("La version {version} est prête. Elle s'installera à la fermeture de LastView, ou tout de suite depuis l'icône de la zone de notification."))
                        .show();
                }
                Err(e) => {
                    log::warn!("mise à jour : téléchargement échoué : {e}");
                    libelle("Rechercher une mise à jour");
                }
            }
        }
        Ok(None) => {
            log::info!("mise à jour : LastView est à jour");
            libelle(if manuel { "LastView est à jour" } else { "Rechercher une mise à jour" });
        }
        Err(e) => {
            log::warn!("mise à jour : vérification impossible : {e}");
            libelle("Rechercher une mise à jour");
        }
    }
}

pub fn prete() -> bool {
    PRETE.lock().unwrap().is_some()
}

/// Installe la mise à jour prête (l'installateur ferme LastView et le relance).
/// Jamais pendant que le jeu tourne, sauf demande explicite (`force`).
pub fn installer(force: bool) -> bool {
    if !force && !system::find_game_pids().is_empty() {
        log::info!("mise à jour gardée pour plus tard : le jeu tourne");
        return false;
    }
    let Some((update, octets)) = PRETE.lock().unwrap().take() else { return false };
    log::info!("installation de la version {}", update.version);
    match update.install(octets) {
        Ok(()) => true,
        Err(e) => {
            log::error!("mise à jour : installation échouée : {e}");
            false
        }
    }
}
