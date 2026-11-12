//! LastView — application Tauri : fenêtres, zone de notification, commandes.
//! Le travail (capture, décodage, rencontres, base) est dans `lastview-core`,
//! lancé en tâche de fond par [`runtime`]. La bulle est pilotée par [`overlay`].

mod overlay;
mod runtime;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lastview_core::api::ApiClient;
use lastview_core::store::{self, Row, DEFAULT_RETENTION_MS};
use lastview_core::{capture::now_ms, diagnostics, Store};
use serde::Serialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State, WindowEvent};

pub struct AppState {
    store: Arc<Mutex<Store>>,
    status: Arc<Mutex<runtime::Status>>,
    /// Client API partagé (fiche, recherche, référence) : une seule file d'attente.
    #[allow(dead_code)]
    api: Arc<ApiClient>,
    overlay: Arc<overlay::Overlay>,
    /// Dernière position de la bulle, enregistrée une fois le déplacement fini.
    moved: Arc<Mutex<Option<(i32, i32, Instant)>>>,
}

static DEMARRAGE: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Copies des références de l'état, pour la mise en route.
struct AppStateRefs {
    store: Arc<Mutex<Store>>,
    status: Arc<Mutex<runtime::Status>>,
    overlay: Arc<overlay::Overlay>,
    moved: Arc<Mutex<Option<(i32, i32, Instant)>>>,
}

fn setting_num(store: &Store, key: &str, default: i64) -> i64 {
    store.setting(key).ok().flatten().and_then(|v| v.trim_matches('"').parse().ok()).unwrap_or(default)
}

#[derive(Serialize)]
struct ReglagesBulle {
    opacite_repos: u8,
    opacite_survol: u8,
    verrouillee: bool,
    tags: bool,
    compteur: bool,
    /// Lignes visibles dans la bulle déroulée, le reste en défilant (5 par défaut, choix de Gio).
    lignes: u8,
}

#[tauri::command]
fn liste(state: State<'_, AppState>) -> Result<Vec<Row>, String> {
    state.store.lock().unwrap().list(now_ms(), DEFAULT_RETENTION_MS).map_err(|e| e.to_string())
}

#[tauri::command]
fn etat(state: State<'_, AppState>) -> runtime::Status {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
fn reglages_bulle(state: State<'_, AppState>) -> ReglagesBulle {
    let s = state.store.lock().unwrap();
    let (rest, hover) = *state.overlay.opacity.lock().unwrap();
    ReglagesBulle {
        opacite_repos: rest,
        opacite_survol: hover,
        verrouillee: setting_num(&s, "bulle_verrouillee", 0) != 0,
        tags: setting_num(&s, "afficher_tags", 1) != 0,
        compteur: setting_num(&s, "afficher_compteur", 1) != 0,
        lignes: setting_num(&s, "lignes_visibles", 5).clamp(3, 20) as u8,
    }
}

/// La bulle donne sa taille (replié / déroulé) : la fenêtre colle au contenu,
/// pour que la zone de survol soit exactement la bulle.
#[tauri::command]
fn taille_bulle(app: AppHandle, largeur: f64, hauteur: f64) {
    log::debug!("bulle : taille {largeur} × {hauteur}");
    if let Some(w) = app.get_webview_window("bulle") {
        if let Err(e) = w.set_size(LogicalSize::new(largeur.clamp(120.0, 800.0), hauteur.clamp(30.0, 1200.0))) {
            log::warn!("bulle : taille non appliquée : {e}");
        }
    }
}

/// Erreurs du front (script de la bulle ou de la fiche) : dans le journal.
#[tauri::command]
fn journal_front(fenetre: String, message: String) {
    log::warn!("front {fenetre} : {message}");
}

#[tauri::command]
fn epingler(app: AppHandle, state: State<'_, AppState>, id: i64, oui: bool) -> Result<(), String> {
    state.store.lock().unwrap().set_pinned(id, oui).map_err(|e| e.to_string())?;
    let _ = app.emit("liste-modifiee", ());
    Ok(())
}

/// Bouton « + » : ouvre la fiche (son contenu arrive à la section 6).
#[tauri::command]
fn ouvrir_fiche(app: AppHandle, id: i64) {
    log::info!("fiche demandée pour le joueur {id}");
    if let Some(w) = app.get_webview_window("fiche") {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = app.emit_to("fiche", "fiche-demandee", id);
    }
}

#[tauri::command]
fn recherche_focus(app: AppHandle, state: State<'_, AppState>, actif: bool) {
    overlay::search_focus(&app, &state.overlay, actif);
}

/// Clic reçu par la bulle : journalisé pour vérifier qu'elle n'en intercepte aucun hors survol.
#[tauri::command]
fn clic_bulle(cible: String) {
    log::info!("bulle : clic sur {cible}");
}

fn toggle_bulle(app: &AppHandle) {
    let st = app.state::<AppState>();
    let hidden = !st.overlay.user_hidden.load(Ordering::Relaxed);
    st.overlay.user_hidden.store(hidden, Ordering::Relaxed);
    log::info!("bulle {}", if hidden { "masquée" } else { "affichée" });
}

/// `lastview.exe --verifier-pages` : vérifie que les pages du front sont
/// intégrées au binaire (option `custom-protocol`), puis quitte. Code de
/// sortie 0 si oui, 3 sinon. Sert à la CI, pour que l'incident de la page
/// d'erreur « localhost » ne puisse pas revenir dans une Release.
fn verifier_pages(context: &tauri::Context<tauri::Wry>) -> ! {
    use tauri::utils::assets::AssetKey;
    let mut ok = cfg!(feature = "custom-protocol");
    let mut rapport = format!("custom-protocol : {}
", if ok { "oui" } else { "NON" });
    for page in ["bulle.html", "fiche.html"] {
        let present = context.assets().get(&AssetKey::from(page)).is_some_and(|b| !b.is_empty());
        ok &= present;
        rapport.push_str(&format!("{page} : {}
", if present { "intégrée" } else { "ABSENTE" }));
    }
    // Le programme n'a pas de console : le rapport va aussi dans un fichier à côté du binaire.
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::write(exe.with_file_name("verifier-pages.txt"), &rapport);
    }
    eprint!("{rapport}");
    std::process::exit(if ok { 0 } else { 3 });
}

pub fn run() {
    let context = tauri::generate_context!();
    if std::env::args().any(|a| a == "--verifier-pages") {
        verifier_pages(&context);
    }
    DEMARRAGE.get_or_init(Instant::now);
    diagnostics::init(&diagnostics::log_dir(), cfg!(debug_assertions));
    let diag_au_lancement = std::env::args().any(|a| a == "--diagnostic");
    diagnostics::set_diagnostic(diag_au_lancement);
    log::info!("LastView {} démarre{}", env!("CARGO_PKG_VERSION"), if diag_au_lancement { " (mode diagnostic)" } else { "" });
    let db = store::default_db_path();
    let store = match Store::open(&db) {
        Ok(s) => s,
        Err(e) => {
            log::error!("base {} : {e}", db.display());
            Store::open_in_memory().expect("base en mémoire")
        }
    };
    let rest = setting_num(&store, "opacite_repos", 30).clamp(10, 100) as u8;
    let hover = setting_num(&store, "opacite_survol", 100).clamp(10, 100) as u8;
    let saved = store.setting("bulle_position").ok().flatten().and_then(|v| {
        let (x, y) = v.trim_matches('"').split_once(',')?;
        Some((x.parse().ok()?, y.parse().ok()?))
    });
    let api = Arc::new(ApiClient::new(Some(&db)));
    let state = AppState {
        store: Arc::new(Mutex::new(store)),
        status: Arc::new(Mutex::new(runtime::Status::default())),
        api,
        overlay: overlay::Overlay::new(rest, hover),
        moved: Arc::default(),
    };

    // L'état est enregistré avant la création des fenêtres : la bulle l'interroge dès son chargement.
    let (store_c, status_c, overlay_c, moved_c) = (state.store.clone(), state.status.clone(), state.overlay.clone(), state.moved.clone());
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![liste, etat, reglages_bulle, taille_bulle, epingler, ouvrir_fiche, recherche_focus, clic_bulle, journal_front])
        .setup(move |app| {
            let state = AppStateRefs { store: store_c, status: status_c, overlay: overlay_c, moved: moved_c };
            let handle = app.handle().clone();
            runtime::start(handle.clone(), state.store.clone(), state.status.clone());
            if let Some(w) = app.get_webview_window("bulle") {
                overlay::prepare(&w);
                let _ = w.set_position(overlay::initial_position(&w, saved));
            }
            overlay::start(handle.clone(), state.overlay.clone());
            // Position de la bulle : enregistrée 500 ms après la fin du déplacement.
            {
                let (moved, store) = (state.moved.clone(), state.store.clone());
                std::thread::Builder::new()
                    .name("lv-position".into())
                    .spawn(move || loop {
                        std::thread::sleep(Duration::from_millis(250));
                        let mut m = moved.lock().unwrap();
                        if let Some((x, y, t)) = *m {
                            if t.elapsed() >= Duration::from_millis(500) {
                                let _ = store.lock().unwrap().set_setting("bulle_position", &format!("{x},{y}"));
                                *m = None;
                            }
                        }
                    })
                    .ok();
            }

            // Zone de notification.
            let bulle = MenuItem::with_id(app, "bulle", "Masquer / afficher la bulle", true, None::<&str>)?;
            let fiche = MenuItem::with_id(app, "fiche", "Ouvrir la fiche", true, None::<&str>)?;
            let diag = CheckMenuItem::with_id(app, "diagnostic", "Mode diagnostic", true, diag_au_lancement, None::<&str>)?;
            let quitter = MenuItem::with_id(app, "quitter", "Quitter", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&bulle, &fiche, &sep, &diag, &sep, &quitter])?;
            let diag_item = diag.clone();
            TrayIconBuilder::with_id("lastview")
                .icon(app.default_window_icon().cloned().expect("icône"))
                .tooltip("LastView")
                .menu(&menu)
                .on_menu_event(move |app, ev| match ev.id().as_ref() {
                    "bulle" => toggle_bulle(app),
                    "fiche" => {
                        if let Some(w) = app.get_webview_window("fiche") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "diagnostic" => {
                        let on = diag_item.is_checked().unwrap_or(false);
                        diagnostics::set_diagnostic(on);
                        log::info!("mode diagnostic : {}", if on { "activé" } else { "désactivé" });
                    }
                    "quitter" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|w, ev| match ev {
            // Fermer une fenêtre la cache : l'app continue dans la zone de notification.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = w.hide();
            }
            // La mise en place au démarrage n'est pas un déplacement de Gio : ignorée.
            WindowEvent::Moved(p) if w.label() == "bulle" && DEMARRAGE.get().is_some_and(|t| t.elapsed() > Duration::from_secs(3)) => {
                let st = w.app_handle().state::<AppState>();
                *st.moved.lock().unwrap() = Some((p.x, p.y, Instant::now()));
            }
            _ => {}
        })
        .run(context)
        .expect("LastView n'a pas pu démarrer");
}
