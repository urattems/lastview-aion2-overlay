//! Le moteur en tâche de fond : capture → rencontres → base, et événements
//! vers les fenêtres (`liste-modifiee`, `etat`). Le CP / GS de Gio est lu
//! dans les paquets (`33 36`, `56 36`), sans appel à l'API : les flèches ▲▼
//! ont été retirées de la bulle (choix de Gio), l'appel périodique aussi.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use lastview_core::capture::{self, now_ms, CaptureStatus};
use lastview_core::veille::VeilleProtocole;
use lastview_core::{superviseur, Engine, Kind, Store};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::parametres::Parametres;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Moi {
    pub key: Option<u32>,
    pub confirmed: bool,
    pub name: String,
    pub server: u16,
}

/// CP et GS de Gio lus dans les paquets du jeu (`33 36` / `56 36`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Reference {
    /// `paquet` ou `aucune` : affiché dans le panneau de diagnostic.
    pub source: &'static str,
    pub cp: i64,
    pub gs: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Status {
    pub capture: CaptureStatus,
    pub joueurs_en_memoire: usize,
    pub moi: Moi,
    pub reference: Reference,
    /// Aucune apparition décodée après 2 min de trafic (10.1) : point rouge sur l'œil.
    pub protocole_a_mettre_a_jour: bool,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            capture: CaptureStatus::AttenteJeu,
            joueurs_en_memoire: 0,
            moi: Moi { key: None, confirmed: false, name: String::new(), server: 0 },
            reference: Reference { source: "aucune", cp: 0, gs: 0 },
            protocole_a_mettre_a_jour: false,
        }
    }
}

/// Joueur de la mémoire silencieuse, pour les suggestions de la recherche (section 7).
#[derive(Debug, Clone, Serialize)]
pub struct Memoire {
    pub nom: String,
    pub serveur: u16,
    pub classe: Option<&'static str>,
    pub niveau: i32,
}

/// Panneau de diagnostic (10.2), mis à jour chaque seconde par le moteur.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Diagnostic {
    pub interface: Option<String>,
    pub serveur: Option<String>,
    pub paquets: u64,
    pub paquets_par_s: f64,
    pub illisibles: u64,
    pub apparitions: u64,
    pub rencontres: u64,
    pub joueurs_en_memoire: usize,
    /// Pseudo, type et heure (ms) de la dernière rencontre.
    pub derniere_rencontre: Option<(String, &'static str, i64)>,
    pub protocole: &'static str,
    pub version_protocole: &'static str,
    /// Enregistrement du trafic en cours : secondes restantes, segments écrits.
    pub enregistrement: Option<(i64, u64)>,
    /// Redémarrages du moteur après une panique.
    pub redemarrages: u32,
}

/// Ce que le moteur partage avec l'app.
#[derive(Clone)]
pub struct Partage {
    pub app: AppHandle,
    pub store: Arc<Mutex<Store>>,
    pub status: Arc<Mutex<Status>>,
    pub memoire: Arc<Mutex<Vec<Memoire>>>,
    pub params: Arc<RwLock<Parametres>>,
    pub diag: Arc<Mutex<Diagnostic>>,
    /// Demande d'enregistrement du trafic : fichier et durée (ms).
    pub enregistrer: Arc<Mutex<Option<(std::path::PathBuf, i64)>>>,
}

/// Le moteur tourne sous supervision : après une panique, il redémarre (10.3).
pub fn start(partage: Partage) {
    let mut passages = 0u32;
    superviseur::fil("moteur", move || {
        if passages > 0 {
            partage.diag.lock().unwrap_or_else(std::sync::PoisonError::into_inner).redemarrages = passages;
            log::warn!("moteur redémarré ({passages})");
        }
        passages += 1;
        run(partage.clone());
    })
    .expect("fil du moteur");
}

/// `LASTVIEW_PANIQUE_TEST=moteur` : une seule panique, 5 s après le démarrage (tests de la section 10).
static PANIQUE_FAITE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn interface(p: &Parametres) -> Option<String> {
    (!p.interface.is_empty()).then(|| p.interface.clone())
}

fn run(partage: Partage) {
    let Partage { app, store, status, memoire, params, diag, enregistrer } = partage;
    let panique_test = std::env::var("LASTVIEW_PANIQUE_TEST").is_ok_and(|v| v == "moteur");
    let demarrage = Instant::now();
    let mut veille = VeilleProtocole::new();
    let mut last_diag = Instant::now();
    let mut paquets_avant = 0u64;
    let mut engine = Engine::new();
    let mut p = params.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    // « Mon perso / Mon serveur » : repli si la détection automatique échoue.
    engine.world.set_local_hint(&p.mon_perso, p.mon_serveur);
    for k in Kind::ALL {
        engine.set_enabled(k, p.sources.active(k));
    }
    {
        let mut s = store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = s.cache_purge(now_ms());
        if let Ok(n) = s.purge(now_ms(), p.retention_ms(), p.max_joueurs as usize) {
            log::info!("purge au démarrage : {n} joueur(s)");
        }
    }
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut cap = capture::start(interface(&p), tx.clone());
    let mut changed = false;
    let mut last_tick = Instant::now();
    let mut last_purge = Instant::now();
    let mut last_memoire = Instant::now();
    loop {
        if let Ok((_, p)) = rx.recv_timeout(Duration::from_millis(250)) {
            let encounters = engine.handle(&p);
            if !encounters.is_empty() {
                let mut s = store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(e) = encounters.last() {
                    diag.lock().unwrap_or_else(std::sync::PoisonError::into_inner).derniere_rencontre = Some((e.player.name.clone(), e.kind.label(), e.ts_ms));
                }
                for e in &encounters {
                    match s.record(e) {
                        Ok(Some(_)) => changed = true,
                        Ok(None) => {}
                        Err(err) => log::error!("base : {err}"),
                    }
                }
            }
        }
        if panique_test && demarrage.elapsed() > Duration::from_secs(5) && !PANIQUE_FAITE.swap(true, std::sync::atomic::Ordering::Relaxed) {
            panic!("panique simulée dans le moteur (LASTVIEW_PANIQUE_TEST)");
        }
        if let Some((chemin, duree)) = enregistrer.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() {
            if let Err(e) = cap.enregistrer(&chemin, duree) {
                log::error!("enregistrement du trafic impossible : {e}");
            }
        }
        if last_tick.elapsed() >= Duration::from_millis(250) {
            last_tick = Instant::now();
            // Paramètres modifiés (section 8) : appliqués à chaud.
            let n = params.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
            if n != p {
                if n.sources != p.sources {
                    for k in Kind::ALL {
                        engine.set_enabled(k, n.sources.active(k));
                    }
                }
                if n.interface != p.interface {
                    log::info!("interface de capture : {}", if n.interface.is_empty() { "automatique" } else { &n.interface });
                    cap.stop();
                    cap = capture::start(interface(&n), tx.clone());
                }
                if (n.mon_perso != p.mon_perso || n.mon_serveur != p.mon_serveur) && !engine.world.local.confirmed {
                    engine.world.local.name = n.mon_perso.clone();
                    engine.world.local.server = n.mon_serveur;
                }
                if n.retention_h != p.retention_h || n.max_joueurs != p.max_joueurs {
                    let _ = store.lock().unwrap_or_else(std::sync::PoisonError::into_inner).purge(now_ms(), n.retention_ms(), n.max_joueurs as usize);
                    changed = true;
                }
                p = n;
            }
            if changed {
                changed = false;
                let _ = app.emit("liste-modifiee", ());
            }
            let l = &engine.world.local;
            let reference = if l.cp > 0 { Reference { source: "paquet", cp: l.cp, gs: l.gs as i64 } } else { Reference { source: "aucune", cp: 0, gs: l.gs as i64 } };
            let capture_etat = cap.status.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
            if matches!(capture_etat, CaptureStatus::AttenteJeu) {
                veille.remettre_a_zero(engine.stats.packets, engine.stats.spawns);
            }
            let alerte = veille.observer(now_ms(), engine.stats.packets, engine.stats.spawns);
            if last_diag.elapsed() >= Duration::from_secs(1) {
                let dt = last_diag.elapsed().as_secs_f64();
                last_diag = Instant::now();
                let mut d = diag.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                d.interface = cap.interface_jeu.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
                d.serveur = match &capture_etat {
                    CaptureStatus::Active { serveur } => Some(serveur.clone()),
                    _ => None,
                };
                d.paquets_par_s = (engine.stats.packets - paquets_avant) as f64 / dt;
                paquets_avant = engine.stats.packets;
                d.paquets = engine.stats.packets;
                d.illisibles = engine.stats.unreadable.values().sum();
                d.apparitions = engine.stats.spawns;
                d.rencontres = engine.stats.encounters;
                d.joueurs_en_memoire = engine.world.player_count();
                d.protocole = if alerte { "à mettre à jour" } else if engine.stats.spawns > 0 { "reconnu" } else { "en attente de trafic" };
                d.version_protocole = lastview_core::protocol::opcodes::VERSION_PROTOCOLE;
                d.enregistrement = cap.enregistrement();
            }
            let now = Status {
                capture: capture_etat,
                joueurs_en_memoire: engine.world.player_count(),
                moi: Moi { key: l.key, confirmed: l.confirmed, name: l.name.clone(), server: l.server },
                reference,
                protocole_a_mettre_a_jour: alerte,
            };
            let mut st = status.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if *st != now {
                *st = now.clone();
                let _ = app.emit("etat", now);
            }
        }
        if last_memoire.elapsed() >= Duration::from_secs(2) {
            last_memoire = Instant::now();
            let m: Vec<Memoire> = engine.world.players().map(|(_, p)| Memoire { nom: p.name.clone(), serveur: p.server, classe: p.class, niveau: p.level }).collect();
            *memoire.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = m;
        }
        if last_purge.elapsed() >= Duration::from_secs(60) {
            last_purge = Instant::now();
            if let Ok(n) = store.lock().unwrap_or_else(std::sync::PoisonError::into_inner).purge(now_ms(), p.retention_ms(), p.max_joueurs as usize) {
                if n > 0 {
                    let _ = app.emit("liste-modifiee", ());
                }
            }
        }
    }
}
