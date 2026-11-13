//! Le moteur en tâche de fond : capture → rencontres → base, et événements
//! vers les fenêtres (`liste-modifiee`, `etat`). Le CP / GS de Gio est lu
//! dans les paquets (`33 36`, `56 36`), sans appel à l'API : les flèches ▲▼
//! ont été retirées de la bulle (choix de Gio), l'appel périodique aussi.

use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use lastview_core::capture::{self, now_ms, CaptureStatus};
use lastview_core::{Engine, Kind, Store};
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
}

impl Default for Status {
    fn default() -> Self {
        Self {
            capture: CaptureStatus::AttenteJeu,
            joueurs_en_memoire: 0,
            moi: Moi { key: None, confirmed: false, name: String::new(), server: 0 },
            reference: Reference { source: "aucune", cp: 0, gs: 0 },
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

pub fn start(app: AppHandle, store: Arc<Mutex<Store>>, status: Arc<Mutex<Status>>, memoire: Arc<Mutex<Vec<Memoire>>>, params: Arc<RwLock<Parametres>>) {
    thread::Builder::new()
        .name("lv-moteur".into())
        .spawn(move || run(app, store, status, memoire, params))
        .expect("fil du moteur");
}

fn interface(p: &Parametres) -> Option<String> {
    (!p.interface.is_empty()).then(|| p.interface.clone())
}

fn run(app: AppHandle, store: Arc<Mutex<Store>>, status: Arc<Mutex<Status>>, memoire: Arc<Mutex<Vec<Memoire>>>, params: Arc<RwLock<Parametres>>) {
    let mut engine = Engine::new();
    let mut p = params.read().unwrap().clone();
    // « Mon perso / Mon serveur » : repli si la détection automatique échoue.
    engine.world.set_local_hint(&p.mon_perso, p.mon_serveur);
    for k in Kind::ALL {
        engine.set_enabled(k, p.sources.active(k));
    }
    {
        let mut s = store.lock().unwrap();
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
                let mut s = store.lock().unwrap();
                for e in &encounters {
                    match s.record(e) {
                        Ok(Some(_)) => changed = true,
                        Ok(None) => {}
                        Err(err) => log::error!("base : {err}"),
                    }
                }
            }
        }
        if last_tick.elapsed() >= Duration::from_millis(250) {
            last_tick = Instant::now();
            // Paramètres modifiés (section 8) : appliqués à chaud.
            let n = params.read().unwrap().clone();
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
                    let _ = store.lock().unwrap().purge(now_ms(), n.retention_ms(), n.max_joueurs as usize);
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
            let now = Status {
                capture: cap.status.read().unwrap().clone(),
                joueurs_en_memoire: engine.world.player_count(),
                moi: Moi { key: l.key, confirmed: l.confirmed, name: l.name.clone(), server: l.server },
                reference,
            };
            let mut st = status.lock().unwrap();
            if *st != now {
                *st = now.clone();
                let _ = app.emit("etat", now);
            }
        }
        if last_memoire.elapsed() >= Duration::from_secs(2) {
            last_memoire = Instant::now();
            let m: Vec<Memoire> = engine.world.players().map(|(_, p)| Memoire { nom: p.name.clone(), serveur: p.server, classe: p.class, niveau: p.level }).collect();
            *memoire.lock().unwrap() = m;
        }
        if last_purge.elapsed() >= Duration::from_secs(60) {
            last_purge = Instant::now();
            if let Ok(n) = store.lock().unwrap().purge(now_ms(), p.retention_ms(), p.max_joueurs as usize) {
                if n > 0 {
                    let _ = app.emit("liste-modifiee", ());
                }
            }
        }
    }
}
