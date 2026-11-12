//! Le moteur en tâche de fond : capture → rencontres → base, et événements
//! vers les fenêtres (`liste-modifiee`, `etat`). Le CP / GS de Gio est lu
//! dans les paquets (`33 36`, `56 36`), sans appel à l'API : les flèches ▲▼
//! ont été retirées de la bulle (choix de Gio), l'appel périodique aussi.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lastview_core::capture::{self, now_ms, CaptureStatus};
use lastview_core::store::{DEFAULT_MAX_PLAYERS, DEFAULT_RETENTION_MS};
use lastview_core::{Engine, Store};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

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

/// « Mon perso / Mon serveur » (paramètres, section 8), Urattems / Vaizel par défaut.
fn mon_perso(store: &Mutex<Store>) -> (String, u16) {
    let s = store.lock().unwrap();
    let name = s.setting("mon_perso").ok().flatten().unwrap_or_else(|| "Urattems".into());
    let server = s.setting("mon_serveur").ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(1303);
    (name, server)
}

pub fn start(app: AppHandle, store: Arc<Mutex<Store>>, status: Arc<Mutex<Status>>) {
    thread::Builder::new()
        .name("lv-moteur".into())
        .spawn(move || run(app, store, status))
        .expect("fil du moteur");
}

fn run(app: AppHandle, store: Arc<Mutex<Store>>, status: Arc<Mutex<Status>>) {
    let mut engine = Engine::new();
    let (name, server) = mon_perso(&store);
    engine.world.set_local_hint(&name, server);
    {
        let mut s = store.lock().unwrap();
        let _ = s.cache_purge(now_ms());
        if let Ok(n) = s.purge(now_ms(), DEFAULT_RETENTION_MS, DEFAULT_MAX_PLAYERS) {
            log::info!("purge au démarrage : {n} joueur(s)");
        }
    }
    let (tx, rx) = crossbeam_channel::unbounded();
    let cap = capture::start(None, tx);
    let mut changed = false;
    let mut last_tick = Instant::now();
    let mut last_purge = Instant::now();
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
        if last_purge.elapsed() >= Duration::from_secs(60) {
            last_purge = Instant::now();
            if let Ok(n) = store.lock().unwrap().purge(now_ms(), DEFAULT_RETENTION_MS, DEFAULT_MAX_PLAYERS) {
                if n > 0 {
                    let _ = app.emit("liste-modifiee", ());
                }
            }
        }
    }
}
