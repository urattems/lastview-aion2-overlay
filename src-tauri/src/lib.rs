//! LastView — application Tauri : fenêtres, zone de notification, commandes.
//! Le travail (capture, décodage, rencontres, base) est dans `lastview-core`,
//! lancé en tâche de fond par [`runtime`]. La bulle est pilotée par [`overlay`].

mod fiche;
mod images;
mod overlay;
mod parametres;
mod recherche;
mod runtime;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

use lastview_core::api::ApiClient;
use lastview_core::store::{self, Row};
use lastview_core::{capture::now_ms, diagnostics, Store};
use serde::Serialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use parametres::Parametres;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State, WindowEvent, Wry};

pub struct AppState {
    store: Arc<Mutex<Store>>,
    status: Arc<Mutex<runtime::Status>>,
    /// Client API partagé (fiche, recherche) : une seule file d'attente.
    api: Arc<ApiClient>,
    /// Paramètres (section 8), relus à chaud par le moteur.
    params: Arc<RwLock<Parametres>>,
    /// Mémoire silencieuse (instantané toutes les 2 s), pour la recherche.
    memoire: Arc<Mutex<Vec<runtime::Memoire>>>,
    /// Fiche demandée au lancement (`--fiche Pseudo[:serveur]`, pour les tests).
    fiche_initiale: Option<(String, Option<u16>)>,
    overlay: Arc<overlay::Overlay>,
    /// Dernière position de la bulle, enregistrée une fois le déplacement fini.
    moved: Arc<Mutex<Option<(i32, i32, Instant)>>>,
    /// Position et taille de la fiche (x, y, l, h), enregistrées de même.
    fiche_geo: Arc<Mutex<Option<(i32, i32, u32, u32, Instant)>>>,
}

static DEMARRAGE: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Copies des références de l'état, pour la mise en route.
struct AppStateRefs {
    store: Arc<Mutex<Store>>,
    status: Arc<Mutex<runtime::Status>>,
    overlay: Arc<overlay::Overlay>,
    moved: Arc<Mutex<Option<(i32, i32, Instant)>>>,
    fiche_geo: Arc<Mutex<Option<(i32, i32, u32, u32, Instant)>>>,
}

/// Case « Mode diagnostic » du menu de la zone de notification.
static CASE_DIAGNOSTIC: OnceLock<CheckMenuItem<Wry>> = OnceLock::new();

pub(crate) fn cocher_diagnostic(_app: &AppHandle, on: bool) {
    if let Some(c) = CASE_DIAGNOSTIC.get() {
        let _ = c.set_checked(on);
    }
}

/// Affiche la fiche, sur l'écran choisi dans les paramètres s'il y en a un.
pub(crate) fn montrer_fiche(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("fiche") {
        let ecran = app.state::<AppState>().params.read().unwrap().ecran_fiche.clone();
        if !ecran.is_empty() && !w.is_visible().unwrap_or(false) {
            parametres::placer_sur_ecran(&w, &ecran);
        }
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
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
    let retention = state.params.read().unwrap().retention_ms();
    state.store.lock().unwrap().list(now_ms(), retention).map_err(|e| e.to_string())
}

#[tauri::command]
fn etat(state: State<'_, AppState>) -> runtime::Status {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
fn reglages_bulle(state: State<'_, AppState>) -> ReglagesBulle {
    let p = state.params.read().unwrap();
    ReglagesBulle { opacite_repos: p.opacite_repos, opacite_survol: p.opacite_survol, verrouillee: p.verrouillee, tags: p.tags, compteur: p.compteur, lignes: p.lignes }
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

#[derive(Serialize)]
struct FicheInitiale {
    nom: String,
    serveur: Option<u16>,
    /// `--infobulle <n>` : ouvre une infobulle d'office (captures de test) :
    /// emplacement de la pièce (Équipement), ou rang de la compétence / du nœud.
    infobulle: Option<i64>,
    /// `--onglet <0..3>` : onglet affiché (captures de test).
    onglet: Option<i64>,
    /// `--enregistrer` : enregistre la fiche en PNG une fois chargée (test de 6.8).
    enregistrer: bool,
    /// `--recherche <texte>` : tape ce texte dans la recherche (captures de la section 7).
    recherche: Option<String>,
}

/// La fiche demandée au lancement (`--fiche Pseudo[:serveur]`), une seule fois.
#[tauri::command]
fn fiche_initiale(state: State<'_, AppState>) -> Option<FicheInitiale> {
    let infobulle = std::env::args().skip_while(|a| a != "--infobulle").nth(1).and_then(|v| v.parse().ok());
    let onglet = std::env::args().skip_while(|a| a != "--onglet").nth(1).and_then(|v| v.parse().ok());
    let recherche = std::env::args().skip_while(|a| a != "--recherche").nth(1);
    state.fiche_initiale.clone().map(|(nom, serveur)| FicheInitiale { nom, serveur, infobulle, onglet, enregistrer: std::env::args().any(|a| a == "--enregistrer"), recherche })
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
    montrer_fiche(&app);
    let _ = app.emit_to("fiche", "fiche-demandee", id);
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

pub(crate) fn toggle_bulle(app: &AppHandle) {
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
    for page in ["bulle.html", "fiche.html", "parametres.html"] {
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
    let db = store::default_db_path();
    let store = match Store::open(&db) {
        Ok(s) => s,
        Err(e) => {
            log::error!("base {} : {e}", db.display());
            Store::open_in_memory().expect("base en mémoire")
        }
    };
    let params = Parametres::charger(&store);
    let diag_au_lancement = std::env::args().any(|a| a == "--diagnostic") || params.diagnostic;
    diagnostics::set_diagnostic(diag_au_lancement);
    log::info!("LastView {} démarre{}", env!("CARGO_PKG_VERSION"), if diag_au_lancement { " (mode diagnostic)" } else { "" });
    let saved = store.setting("bulle_position").ok().flatten().and_then(|v| {
        let (x, y) = v.trim_matches('"').split_once(',')?;
        Some((x.parse().ok()?, y.parse().ok()?))
    });
    let api = Arc::new(ApiClient::new(Some(&db)));
    // `--fiche Pseudo[:serveur]` : ouvre seulement la fiche (sans capture ni bulle), pour tester.
    let fiche_demandee = std::env::args().skip_while(|a| a != "--fiche").nth(1).map(|v| match v.split_once(':') {
        Some((n, s)) => (n.to_string(), s.parse().ok()),
        None => (v, None),
    });
    // `--parametres` : ouvre seulement la fenêtre des paramètres (captures de test).
    let parametres_seuls = std::env::args().any(|a| a == "--parametres");
    let fiche_seule = fiche_demandee.is_some() || parametres_seuls;
    let fiche_fenetre = store.setting("fiche_fenetre").ok().flatten().and_then(|v| {
        let p: Vec<i32> = v.trim_matches('"').split(',').filter_map(|x| x.parse().ok()).collect();
        (p.len() == 4).then(|| (p[0], p[1], p[2], p[3]))
    });
    let state = AppState {
        store: Arc::new(Mutex::new(store)),
        status: Arc::new(Mutex::new(runtime::Status::default())),
        api,
        fiche_initiale: fiche_demandee,
        memoire: Arc::default(),
        overlay: overlay::Overlay::new(params.opacite_repos, params.opacite_survol),
        params: Arc::new(RwLock::new(params)),
        moved: Arc::default(),
        fiche_geo: Arc::default(),
    };

    // L'état est enregistré avant la création des fenêtres : la bulle l'interroge dès son chargement.
    let (store_c, status_c, overlay_c, moved_c, geo_c, memoire_c, params_c) = (state.store.clone(), state.status.clone(), state.overlay.clone(), state.moved.clone(), state.fiche_geo.clone(), state.memoire.clone(), state.params.clone());
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![liste, etat, reglages_bulle, taille_bulle, epingler, ouvrir_fiche, recherche_focus, clic_bulle, journal_front, fiche_initiale, fiche::fiche_ouvrir, fiche::fiche_piece, fiche::fiche_daevanion, fiche::ouvrir_shugo, fiche::enregistrer_png, recherche::recherche_locale, recherche::recherche_api, recherche::ouvrir_fiche_nom, parametres::parametres_lire, parametres::parametres_ecrire, parametres::raccourcis_pause, parametres::vider_liste, parametres::reinitialiser_position, parametres::choisir_dossier_png, parametres::ouvrir_dossier, parametres::ouvrir_parametres])
        .register_asynchronous_uri_scheme_protocol("lvimg", |_ctx, requete, repondre| {
            // Images NCSoft, servies depuis le cache disque (téléchargées une fois).
            let chemin = requete.uri().path().to_string();
            std::thread::spawn(move || repondre.respond(images::servir(&chemin)));
        })
        .setup(move |app| {
            let state = AppStateRefs { store: store_c, status: status_c, overlay: overlay_c, moved: moved_c, fiche_geo: geo_c };
            let handle = app.handle().clone();
            {
                let p = params_c.read().unwrap().clone();
                parametres::appliquer(&handle, None, &p);
                if !fiche_seule {
                    parametres::appliquer_raccourcis(&handle, &p);
                    parametres::synchroniser_demarrage(&p);
                }
            }
            if fiche_seule {
                log::info!("mode test (--fiche / --parametres) : ni capture, ni bulle, ni raccourcis");
                if parametres_seuls {
                    if let Some(w) = app.get_webview_window("parametres") {
                        overlay::afficher_sans_activer(&w);
                    }
                }
            } else {
                runtime::start(handle.clone(), state.store.clone(), state.status.clone(), memoire_c.clone(), params_c.clone());
                if let Some(w) = app.get_webview_window("bulle") {
                    overlay::prepare(&w);
                    let _ = w.set_position(overlay::initial_position(&w, saved));
                }
                overlay::start(handle.clone(), state.overlay.clone());
            }
            // Fiche : position et taille mémorisées (6.1), si l'écran existe encore.
            if let Some(w) = app.get_webview_window("fiche") {
                if let Some((x, y, l, h)) = fiche_fenetre {
                    let visible = w.available_monitors().map(|ms| ms.iter().any(|m| {
                        let (p, s) = (m.position(), m.size());
                        x + 80 > p.x && y + 20 > p.y && x < p.x + s.width as i32 - 80 && y < p.y + s.height as i32 - 40
                    })).unwrap_or(false);
                    if visible && l >= 720 && h >= 480 {
                        let _ = w.set_size(tauri::PhysicalSize::new(l as u32, h as u32));
                        let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
                    }
                }
                if fiche_seule && !parametres_seuls {
                    overlay::afficher_sans_activer(&w);
                }
            }
            // Position de la bulle : enregistrée 500 ms après la fin du déplacement.
            {
                let (moved, geo, store) = (state.moved.clone(), state.fiche_geo.clone(), state.store.clone());
                std::thread::Builder::new()
                    .name("lv-position".into())
                    .spawn(move || loop {
                        std::thread::sleep(Duration::from_millis(250));
                        {
                            let mut m = moved.lock().unwrap();
                            if let Some((x, y, t)) = *m {
                                if t.elapsed() >= Duration::from_millis(500) {
                                    let _ = store.lock().unwrap().set_setting("bulle_position", &format!("{x},{y}"));
                                    *m = None;
                                }
                            }
                        }
                        let mut g = geo.lock().unwrap();
                        if let Some((x, y, l, h, t)) = *g {
                            if t.elapsed() >= Duration::from_millis(500) {
                                let _ = store.lock().unwrap().set_setting("fiche_fenetre", &format!("{x},{y},{l},{h}"));
                                *g = None;
                            }
                        }
                    })
                    .ok();
            }

            // Zone de notification.
            let bulle = MenuItem::with_id(app, "bulle", "Masquer / afficher la bulle", true, None::<&str>)?;
            let fiche = MenuItem::with_id(app, "fiche", "Ouvrir la fiche", true, None::<&str>)?;
            let reglages = MenuItem::with_id(app, "parametres", "Paramètres…", true, None::<&str>)?;
            let diag = CheckMenuItem::with_id(app, "diagnostic", "Mode diagnostic", true, diag_au_lancement, None::<&str>)?;
            let quitter = MenuItem::with_id(app, "quitter", "Quitter", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&bulle, &fiche, &reglages, &sep, &diag, &sep, &quitter])?;
            let diag_item = diag.clone();
            let _ = CASE_DIAGNOSTIC.set(diag.clone());
            TrayIconBuilder::with_id("lastview")
                .icon(app.default_window_icon().cloned().expect("icône"))
                .tooltip("LastView")
                .menu(&menu)
                .on_menu_event(move |app, ev| match ev.id().as_ref() {
                    "bulle" => toggle_bulle(app),
                    "fiche" => montrer_fiche(app),
                    "parametres" => parametres::ouvrir_parametres(app.clone()),
                    "diagnostic" => {
                        // Même effet que la case des paramètres : enregistré et appliqué.
                        let on = diag_item.is_checked().unwrap_or(false);
                        let st = app.state::<AppState>();
                        let mut p = st.params.read().unwrap().clone();
                        p.diagnostic = on;
                        let _ = parametres::parametres_ecrire(app.clone(), st, p);
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
            WindowEvent::Moved(_) | WindowEvent::Resized(_) if w.label() == "fiche" && DEMARRAGE.get().is_some_and(|t| t.elapsed() > Duration::from_secs(3)) => {
                if let (Ok(p), Ok(s)) = (w.outer_position(), w.outer_size()) {
                    // Fenêtre réduite : position hors écran, rien à retenir.
                    if p.x > -10_000 && s.width > 0 {
                        let st = w.app_handle().state::<AppState>();
                        *st.fiche_geo.lock().unwrap() = Some((p.x, p.y, s.width, s.height, Instant::now()));
                    }
                }
            }
            _ => {}
        })
        .run(context)
        .expect("LastView n'a pas pu démarrer");
}
