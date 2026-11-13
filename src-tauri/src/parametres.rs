//! Paramètres (section 8). Un seul objet, en JSON dans la base (clé
//! `parametres`), partagé avec le moteur et la bulle. Tout s'applique à chaud :
//! la bulle et la liste se redessinent, le moteur relit sources, rétention,
//! interface et « Mon perso » à son prochain tour (250 ms), les raccourcis sont
//! réenregistrés auprès de Windows.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;

use lastview_core::capture::npcap;
use lastview_core::{diagnostics, hotkey, store, Kind, Store};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

use crate::AppState;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sources {
    pub cible: bool,
    pub combat: bool,
    pub kill: bool,
    pub mort: bool,
    pub groupe: bool,
    pub buff: bool,
}

impl Default for Sources {
    fn default() -> Self {
        Self { cible: true, combat: true, kill: true, mort: true, groupe: true, buff: true }
    }
}

impl Sources {
    pub fn active(&self, k: Kind) -> bool {
        match k {
            Kind::Cible => self.cible,
            Kind::Combat => self.combat,
            Kind::Kill => self.kill,
            Kind::Mort => self.mort,
            Kind::Groupe => self.groupe,
            Kind::Buff => self.buff,
        }
    }
}

/// Les paramètres du cahier (section 8), avec leurs valeurs par défaut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Parametres {
    // Liste
    pub retention_h: u32,
    pub max_joueurs: u32,
    pub lignes: u8,
    pub sources: Sources,
    pub tags: bool,
    pub compteur: bool,
    // Bulle
    pub opacite_repos: u8,
    pub opacite_survol: u8,
    pub masquer_hors_jeu: bool,
    pub verrouillee: bool,
    // Raccourcis (texte, « Ctrl+Maj+F9 » ; vide = aucun)
    pub raccourci_bulle: String,
    pub raccourci_fiche: String,
    pub raccourci_deplier: String,
    // Moi
    pub mon_perso: String,
    pub mon_serveur: u16,
    // Capture : nom Npcap de l'interface, vide = automatique
    pub interface: String,
    // Fiche : nom de l'écran, vide = dernier utilisé
    pub ecran_fiche: String,
    pub cache_api_min: u32,
    /// Dossier des PNG de la fiche, vide = Images\LastView.
    pub dossier_png: String,
    // Système
    pub demarrer_windows: bool,
    pub maj_auto: bool,
    pub diagnostic: bool,
}

impl Default for Parametres {
    fn default() -> Self {
        Self {
            retention_h: 4,
            max_joueurs: store::DEFAULT_MAX_PLAYERS as u32,
            lignes: 5,
            sources: Sources::default(),
            tags: true,
            compteur: true,
            opacite_repos: 30,
            opacite_survol: 100,
            masquer_hors_jeu: true,
            verrouillee: false,
            raccourci_bulle: String::new(),
            raccourci_fiche: String::new(),
            raccourci_deplier: String::new(),
            mon_perso: "Urattems".into(),
            mon_serveur: 1303,
            interface: String::new(),
            ecran_fiche: String::new(),
            cache_api_min: 10,
            dossier_png: String::new(),
            demarrer_windows: true,
            maj_auto: true,
            diagnostic: false,
        }
    }
}

fn ancien(store: &Store, key: &str) -> Option<i64> {
    store.setting(key).ok().flatten().and_then(|v| v.trim_matches('"').parse().ok())
}

impl Parametres {
    pub fn retention_ms(&self) -> i64 {
        i64::from(self.retention_h) * 3_600_000
    }

    /// Valeurs ramenées dans leurs bornes.
    pub fn normaliser(mut self) -> Self {
        self.retention_h = self.retention_h.clamp(1, 72);
        self.max_joueurs = self.max_joueurs.clamp(20, 1000);
        self.lignes = self.lignes.clamp(3, 20);
        self.opacite_repos = self.opacite_repos.clamp(10, 100);
        self.opacite_survol = self.opacite_survol.clamp(10, 100);
        self.cache_api_min = self.cache_api_min.clamp(1, 60);
        self.mon_perso = self.mon_perso.trim().chars().take(32).collect();
        for r in [&mut self.raccourci_bulle, &mut self.raccourci_fiche, &mut self.raccourci_deplier] {
            *r = r.trim().to_string();
        }
        self
    }

    /// Lit les paramètres ; à défaut, reprend les anciens réglages séparés (sections 5 et 6).
    pub fn charger(store: &Store) -> Self {
        if let Some(p) = store.setting("parametres").ok().flatten().and_then(|j| serde_json::from_str::<Parametres>(&j).ok()) {
            return p.normaliser();
        }
        let d = Self::default();
        Self {
            opacite_repos: ancien(store, "opacite_repos").map_or(d.opacite_repos, |v| v as u8),
            opacite_survol: ancien(store, "opacite_survol").map_or(d.opacite_survol, |v| v as u8),
            verrouillee: ancien(store, "bulle_verrouillee").map_or(d.verrouillee, |v| v != 0),
            tags: ancien(store, "afficher_tags").map_or(d.tags, |v| v != 0),
            compteur: ancien(store, "afficher_compteur").map_or(d.compteur, |v| v != 0),
            lignes: ancien(store, "lignes_visibles").map_or(d.lignes, |v| v as u8),
            mon_perso: store.setting("mon_perso").ok().flatten().unwrap_or(d.mon_perso.clone()),
            mon_serveur: ancien(store, "mon_serveur").map_or(d.mon_serveur, |v| v as u16),
            ..d
        }
        .normaliser()
    }

    pub fn enregistrer(&self, store: &Store) -> Result<(), String> {
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        store.set_setting("parametres", &json).map_err(|e| e.to_string())
    }

    fn raccourcis(&self) -> [(i32, &'static str, &str); 3] {
        [(1, "raccourci_bulle", &self.raccourci_bulle), (2, "raccourci_fiche", &self.raccourci_fiche), (3, "raccourci_deplier", &self.raccourci_deplier)]
    }
}

pub fn dossier_png_defaut() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default().join("Pictures").join("LastView")
}

// ---------- Raccourcis ----------

struct EtatRaccourcis {
    fil: Option<u32>,
    erreurs: Vec<(String, String)>,
}

static RACCOURCIS: Mutex<EtatRaccourcis> = Mutex::new(EtatRaccourcis { fil: None, erreurs: Vec::new() });

fn arreter_raccourcis() {
    if let Some(fil) = RACCOURCIS.lock().unwrap().fil.take() {
        hotkey::stop(fil);
    }
}

/// (Ré)enregistre les raccourcis. Les erreurs (combinaison invalide, en double,
/// déjà prise par une autre application) sont gardées pour la fenêtre.
pub fn appliquer_raccourcis(app: &AppHandle, p: &Parametres) {
    arreter_raccourcis();
    let mut erreurs = HashMap::new();
    let mut touches = Vec::new();
    let mut vus: HashMap<(u32, u32), &str> = HashMap::new();
    for (id, champ, texte) in p.raccourcis() {
        if texte.is_empty() {
            continue;
        }
        match hotkey::parse(texte) {
            None => {
                erreurs.insert(champ.to_string(), "combinaison non prise en charge".to_string());
            }
            Some(k) => {
                if vus.contains_key(&k) {
                    erreurs.insert(champ.to_string(), "déjà utilisée par un autre raccourci de LastView".to_string());
                } else {
                    vus.insert(k, champ);
                    touches.push((id, k.0, k.1));
                }
            }
        }
    }
    if !touches.is_empty() {
        let (tx, rx) = std::sync::mpsc::channel();
        let app2 = app.clone();
        let _ = std::thread::Builder::new().name("lv-raccourcis".into()).spawn(move || {
            hotkey::listen_many(touches, tx, move |id| action_raccourci(&app2, id));
        });
        if let Ok((fil, resultats)) = rx.recv_timeout(Duration::from_secs(2)) {
            RACCOURCIS.lock().unwrap().fil = Some(fil);
            for (id, r) in resultats {
                let champ = p.raccourcis().iter().find(|x| x.0 == id).map(|x| x.1).unwrap_or("");
                match r {
                    Ok(()) => log::info!("raccourci {champ} enregistré"),
                    Err(e) => {
                        log::warn!("raccourci {champ} : {e}");
                        erreurs.insert(champ.to_string(), e);
                    }
                }
            }
        }
    }
    RACCOURCIS.lock().unwrap().erreurs = erreurs.into_iter().collect();
}

fn action_raccourci(app: &AppHandle, id: i32) {
    log::info!("raccourci {id} appuyé");
    match id {
        1 => crate::toggle_bulle(app),
        2 => {
            // Fiche du dernier joueur ciblé (à défaut, de la dernière rencontre).
            let st = app.state::<AppState>();
            let retention = st.params.read().unwrap().retention_ms();
            let rows = st.store.lock().unwrap().list(lastview_core::capture::now_ms(), retention).unwrap_or_default();
            if let Some(r) = rows.iter().find(|r| r.last_kind == Kind::Cible).or(rows.first()) {
                crate::ouvrir_fiche(app.clone(), r.id);
            }
        }
        3 => {
            let _ = app.emit_to("bulle", "basculer", ());
        }
        _ => {}
    }
}

// ---------- Démarrer avec Windows (clé Run de l'utilisateur) ----------

#[link(name = "advapi32")]
extern "system" {
    fn RegSetKeyValueW(hkey: isize, subkey: *const u16, value: *const u16, typ: u32, data: *const c_void, len: u32) -> i32;
    fn RegDeleteKeyValueW(hkey: isize, subkey: *const u16, value: *const u16) -> i32;
    fn RegGetValueW(hkey: isize, subkey: *const u16, value: *const u16, flags: u32, typ: *mut u32, data: *mut c_void, len: *mut u32) -> i32;
}

const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALEUR: &str = "LastView";

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn commande_demarrage() -> Option<String> {
    std::env::current_exe().ok().map(|e| format!("\"{}\"", e.display()))
}

fn demarrage_actuel() -> Option<String> {
    let (k, v) = (w(RUN), w(VALEUR));
    let mut buf = vec![0u16; 1024];
    let mut len = (buf.len() * 2) as u32;
    // RRF_RT_REG_SZ = 2
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr(), 2, std::ptr::null_mut(), buf.as_mut_ptr() as *mut c_void, &mut len) };
    (r == 0).then(|| String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]))
}

pub fn demarrage_actif() -> bool {
    demarrage_actuel().is_some_and(|c| Some(c) == commande_demarrage())
}

fn regler_demarrage(on: bool) {
    let (k, v) = (w(RUN), w(VALEUR));
    let r = unsafe {
        if on {
            let Some(cmd) = commande_demarrage() else { return };
            let d = w(&cmd);
            RegSetKeyValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr(), 1, d.as_ptr() as *const c_void, (d.len() * 2) as u32)
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, k.as_ptr(), v.as_ptr())
        }
    };
    log::info!("démarrer avec Windows : {} (code {r})", if on { "activé" } else { "désactivé" });
}

/// Version installée (hors dossier de compilation) : seule celle-ci s'inscrit
/// d'elle-même au démarrage de Windows, pour qu'une compilation de test ne le fasse jamais.
fn version_installee() -> bool {
    std::env::current_exe().map(|e| !e.to_string_lossy().to_lowercase().contains(r"\target\")).unwrap_or(false)
}

/// Au lancement de l'app : la clé Run suit le paramètre (version installée seulement).
pub fn synchroniser_demarrage(p: &Parametres) {
    if version_installee() && p.demarrer_windows != demarrage_actif() {
        regler_demarrage(p.demarrer_windows);
    }
}

// ---------- Écran de la fiche ----------

/// Place la fiche au centre de l'écran choisi, si elle n'y est pas déjà.
pub fn placer_sur_ecran(fenetre: &WebviewWindow, ecran: &str) {
    let Ok(ecrans) = fenetre.available_monitors() else { return };
    let Some(m) = ecrans.iter().find(|m| m.name().map(String::as_str) == Some(ecran)) else { return };
    if fenetre.current_monitor().ok().flatten().and_then(|c| c.name().cloned()).as_deref() == Some(ecran) {
        return;
    }
    if let Ok(t) = fenetre.outer_size() {
        let (p, s) = (m.position(), m.size());
        let x = p.x + (s.width.saturating_sub(t.width) / 2) as i32;
        let y = p.y + (s.height.saturating_sub(t.height) / 2) as i32;
        let _ = fenetre.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

// ---------- Application et commandes ----------

/// Applique les paramètres qui ne passent pas par le moteur.
pub fn appliquer(app: &AppHandle, avant: Option<&Parametres>, p: &Parametres) {
    let st = app.state::<AppState>();
    *st.overlay.opacity.lock().unwrap() = (p.opacite_repos, p.opacite_survol);
    st.overlay.masquer_hors_jeu.store(p.masquer_hors_jeu, Ordering::Relaxed);
    st.api.set_cache_minutes(p.cache_api_min);
    if let Some(a) = avant {
        if a.diagnostic != p.diagnostic {
            diagnostics::set_diagnostic(p.diagnostic);
            log::info!("mode diagnostic : {}", if p.diagnostic { "activé" } else { "désactivé" });
            crate::cocher_diagnostic(app, p.diagnostic);
        }
        if a.demarrer_windows != p.demarrer_windows {
            regler_demarrage(p.demarrer_windows);
        }
        if a.raccourcis() != p.raccourcis() {
            appliquer_raccourcis(app, p);
        }
    }
    let _ = app.emit("parametres-modifies", ());
    let _ = app.emit("liste-modifiee", ());
}

#[derive(Serialize)]
pub struct Choix {
    valeur: String,
    libelle: String,
}

#[derive(Serialize)]
pub struct Vue {
    parametres: Parametres,
    interfaces: Vec<Choix>,
    ecrans: Vec<Choix>,
    serveurs: Vec<Choix>,
    dossier_png_defaut: String,
    demarrage_actif: bool,
    erreurs_raccourcis: HashMap<String, String>,
    version: &'static str,
    /// `--essai` : la fenêtre modifie elle-même quelques réglages (test, données isolées).
    essai: bool,
}

/// Serveurs EU (API `gameinfo/servers`, en cache 24 h), triés par nom.
fn serveurs(state: &AppState) -> Vec<Choix> {
    let mut out: Vec<(u64, String)> = Vec::new();
    if let Ok(v) = state.api.servers() {
        for s in v.get("serverList").and_then(|l| l.as_array()).into_iter().flatten() {
            if let (Some(id), Some(n)) = (s.get("serverId").and_then(|x| x.as_u64()), s.get("serverName").and_then(|x| x.as_str())) {
                out.push((id, n.to_string()));
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out.into_iter().map(|(id, n)| Choix { valeur: id.to_string(), libelle: n }).collect()
}

fn interfaces() -> Vec<Choix> {
    let Ok(lib) = npcap::Pcap::load() else { return Vec::new() };
    lib.devices()
        .into_iter()
        .filter(|d| d.has_addresses || d.is_loopback)
        .map(|d| Choix { libelle: if d.description.is_empty() { d.name.clone() } else { d.description.clone() }, valeur: d.name })
        .collect()
}

fn ecrans(app: &AppHandle) -> Vec<Choix> {
    let Some(w) = app.get_webview_window("parametres").or_else(|| app.get_webview_window("fiche")) else { return Vec::new() };
    let principal = w.primary_monitor().ok().flatten().and_then(|m| m.name().cloned());
    w.available_monitors()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter_map(|(i, m)| {
            let nom = m.name()?.clone();
            let s = m.size();
            let p = if Some(&nom) == principal.as_ref() { " (principal)" } else { "" };
            Some(Choix { libelle: format!("Écran {} — {} × {}{p}", i + 1, s.width, s.height), valeur: nom })
        })
        .collect()
}

#[tauri::command]
pub async fn parametres_lire(app: AppHandle, state: State<'_, AppState>) -> Result<Vue, String> {
    let parametres = state.params.read().unwrap().clone();
    let a = app.clone();
    let serveurs = tauri::async_runtime::spawn_blocking(move || serveurs(a.state::<AppState>().inner())).await.unwrap_or_default();
    Ok(Vue {
        parametres,
        interfaces: interfaces(),
        ecrans: ecrans(&app),
        serveurs,
        dossier_png_defaut: dossier_png_defaut().display().to_string(),
        demarrage_actif: demarrage_actif(),
        erreurs_raccourcis: RACCOURCIS.lock().unwrap().erreurs.iter().cloned().collect(),
        version: env!("CARGO_PKG_VERSION"),
        essai: std::env::args().any(|a| a == "--essai"),
    })
}

/// Enregistre et applique tout de suite. Rend les erreurs des raccourcis.
#[tauri::command]
pub fn parametres_ecrire(app: AppHandle, state: State<'_, AppState>, parametres: Parametres) -> Result<HashMap<String, String>, String> {
    let p = parametres.normaliser();
    let avant = state.params.read().unwrap().clone();
    if p == avant {
        return Ok(RACCOURCIS.lock().unwrap().erreurs.iter().cloned().collect());
    }
    p.enregistrer(&state.store.lock().unwrap())?;
    *state.params.write().unwrap() = p.clone();
    log::info!("paramètres modifiés");
    appliquer(&app, Some(&avant), &p);
    Ok(RACCOURCIS.lock().unwrap().erreurs.iter().cloned().collect())
}

/// Pendant qu'on saisit un raccourci, ceux de LastView sont suspendus (sinon
/// Windows les intercepte avant la fenêtre).
#[tauri::command]
pub fn raccourcis_pause(app: AppHandle, state: State<'_, AppState>, actif: bool) {
    if actif {
        arreter_raccourcis();
    } else {
        appliquer_raccourcis(&app, &state.params.read().unwrap());
    }
}

#[tauri::command]
pub fn vider_liste(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
    let n = state.store.lock().unwrap().clear().map_err(|e| e.to_string())?;
    log::info!("liste vidée : {n} joueur(s), les épinglés restent");
    let _ = app.emit("liste-modifiee", ());
    Ok(n)
}

#[tauri::command]
pub fn reinitialiser_position(app: AppHandle, state: State<'_, AppState>) {
    if let Some(w) = app.get_webview_window("bulle") {
        let p = crate::overlay::initial_position(&w, None);
        let _ = w.set_position(p);
        let _ = state.store.lock().unwrap().set_setting("bulle_position", "");
        *state.moved.lock().unwrap() = None;
        log::info!("position de la bulle réinitialisée");
    }
}

#[tauri::command]
pub async fn choisir_dossier_png(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let actuel = state.params.read().unwrap().dossier_png.clone();
    let depart = if actuel.is_empty() { dossier_png_defaut() } else { PathBuf::from(actuel) };
    let choix = tauri::async_runtime::spawn_blocking(move || {
        let mut d = rfd::FileDialog::new().set_title("Dossier des images de la fiche");
        if depart.is_dir() {
            d = d.set_directory(&depart);
        }
        d.pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(choix.map(|p| p.display().to_string()))
}

/// Ouvre un dossier dans l'Explorateur : `donnees`, `logs` ou `png`.
#[tauri::command]
pub fn ouvrir_dossier(state: State<'_, AppState>, quel: String) -> Result<(), String> {
    let d = match quel.as_str() {
        "donnees" => store::data_dir(),
        "logs" => diagnostics::log_dir(),
        "png" => {
            let p = state.params.read().unwrap().dossier_png.clone();
            if p.is_empty() {
                dossier_png_defaut()
            } else {
                PathBuf::from(p)
            }
        }
        _ => return Err("dossier inconnu".into()),
    };
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer.exe").arg(&d).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Ouvre la fenêtre des paramètres (zone de notification, bulle).
#[tauri::command]
pub fn ouvrir_parametres(app: AppHandle) {
    if let Some(w) = app.get_webview_window("parametres") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valeurs_par_defaut_du_cahier() {
        let p = Parametres::default();
        assert_eq!((p.retention_h, p.max_joueurs, p.lignes), (4, 150, 5));
        assert_eq!((p.opacite_repos, p.opacite_survol), (30, 100));
        assert!(p.masquer_hors_jeu && !p.verrouillee && p.tags && p.compteur);
        assert!(p.demarrer_windows && p.maj_auto && !p.diagnostic);
        assert_eq!((p.mon_perso.as_str(), p.mon_serveur, p.cache_api_min), ("Urattems", 1303, 10));
        assert!(Kind::ALL.iter().all(|k| p.sources.active(*k)));
    }

    #[test]
    fn bornes_et_champs_absents() {
        let p: Parametres = serde_json::from_str(r#"{"lignes": 50, "opacite_repos": 0, "retention_h": 0}"#).unwrap();
        let p = p.normaliser();
        assert_eq!((p.lignes, p.opacite_repos, p.retention_h), (20, 10, 1));
        // Champs absents : valeurs par défaut.
        assert_eq!(p.max_joueurs, 150);
    }

    #[test]
    fn reprise_des_anciens_reglages() {
        let s = Store::open_in_memory().unwrap();
        s.set_setting("opacite_repos", "45").unwrap();
        s.set_setting("lignes_visibles", "8").unwrap();
        let p = Parametres::charger(&s);
        assert_eq!((p.opacite_repos, p.lignes), (45, 8));
        p.enregistrer(&s).unwrap();
        s.set_setting("opacite_repos", "60").unwrap();
        // Une fois enregistrés, les paramètres priment sur les anciennes clés.
        assert_eq!(Parametres::charger(&s).opacite_repos, 45);
    }
}
