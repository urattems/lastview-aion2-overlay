//! LastView — application Tauri : fenêtres, zone de notification, commandes.
//! Le travail (capture, décodage, rencontres, base) est dans `lastview-core`,
//! lancé en tâche de fond par [`runtime`].

mod runtime;

use std::sync::{Arc, Mutex};

use lastview_core::store::{self, Row, DEFAULT_RETENTION_MS};
use lastview_core::{capture::now_ms, diagnostics, Store};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, State};

pub struct AppState {
    store: Arc<Mutex<Store>>,
    status: Arc<Mutex<runtime::Status>>,
}

#[tauri::command]
fn liste(state: State<'_, AppState>) -> Result<Vec<Row>, String> {
    state.store.lock().unwrap().list(now_ms(), DEFAULT_RETENTION_MS).map_err(|e| e.to_string())
}

#[tauri::command]
fn etat(state: State<'_, AppState>) -> runtime::Status {
    state.status.lock().unwrap().clone()
}

fn show(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.show();
        if label == "fiche" {
            let _ = w.set_focus();
        }
    }
}

pub fn run() {
    diagnostics::init(&diagnostics::log_dir(), cfg!(debug_assertions));
    log::info!("LastView {} démarre", env!("CARGO_PKG_VERSION"));
    let db = store::default_db_path();
    let store = match Store::open(&db) {
        Ok(s) => s,
        Err(e) => {
            log::error!("base {} : {e}", db.display());
            Store::open_in_memory().expect("base en mémoire")
        }
    };
    let state = AppState { store: Arc::new(Mutex::new(store)), status: Arc::new(Mutex::new(runtime::Status::default())) };

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![liste, etat])
        .setup(move |app| {
            let handle = app.handle().clone();
            runtime::start(handle.clone(), state.store.clone(), state.status.clone());
            app.manage(state);

            // Zone de notification.
            let bulle = MenuItem::with_id(app, "bulle", "Afficher la bulle", true, None::<&str>)?;
            let fiche = MenuItem::with_id(app, "fiche", "Ouvrir la fiche", true, None::<&str>)?;
            let diag = CheckMenuItem::with_id(app, "diagnostic", "Mode diagnostic", true, false, None::<&str>)?;
            let quitter = MenuItem::with_id(app, "quitter", "Quitter", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&bulle, &fiche, &sep, &diag, &sep, &quitter])?;
            let diag_item = diag.clone();
            TrayIconBuilder::with_id("lastview")
                .icon(app.default_window_icon().cloned().expect("icône"))
                .tooltip("LastView")
                .menu(&menu)
                .on_menu_event(move |app, ev| match ev.id().as_ref() {
                    "bulle" => show(app, "bulle"),
                    "fiche" => show(app, "fiche"),
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
        .on_window_event(|w, ev| {
            // Fermer la fiche la cache : l'app continue dans la zone de notification.
            if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("LastView n'a pas pu démarrer");
}
