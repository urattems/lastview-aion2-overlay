//! Capture passive du trafic du jeu (Npcap), sens serveur → client.
//!
//! Verrouillage du flux du jeu, validé à la porte 0 :
//! 1. **Table TCP du système** : les connexions établies des processus
//!    `AION2.exe` (port 443 exclu). On ne touche jamais au processus du jeu.
//! 2. **Signature**, en repli quand la table ne donne rien depuis
//!    [`SIGNATURE_DELAY_MS`] (relais, VPN) : un flux candidat est verrouillé
//!    dès qu'il contient assez de paquets aux opcodes typiques du jeu.

pub mod npcap;
pub mod stream;
pub mod system;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;

pub use stream::{GamePacket, Stream};

use crate::protocol::opcodes;

/// Délai sans connexion trouvée dans la table TCP avant d'essayer la signature.
pub const SIGNATURE_DELAY_MS: i64 = 10_000;
/// Paquets aux opcodes typiques nécessaires pour verrouiller un flux par signature.
const SIGNATURE_HITS: u32 = 30;
const MAX_CANDIDATES: usize = 32;

/// (ip source, port source, ip destination, port destination)
pub type FlowKey = (u32, u16, u32, u16);

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}

pub fn flow_label(k: &FlowKey) -> String {
    format!("{}:{}", npcap::ip_to_string(k.0), k.1)
}

/// Un segment TCP avec données, copié depuis la capture.
#[derive(Debug, Clone)]
pub struct Segment {
    pub ts_ms: i64,
    pub key: FlowKey,
    pub seq: u32,
    pub data: Vec<u8>,
    /// Vrai si le flux est une connexion du jeu d'après la table TCP.
    pub known: bool,
}

/// Remet les segments en ordre, flux par flux, et sort les paquets du jeu.
#[derive(Default)]
pub struct Assembler {
    streams: HashMap<FlowKey, Stream>,
    candidates: HashMap<FlowKey, (Stream, u32)>,
    locked: HashSet<FlowKey>,
}

impl Assembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, seg: &Segment, out: &mut Vec<(FlowKey, GamePacket)>) {
        let mut tmp = Vec::new();
        if seg.known || self.locked.contains(&seg.key) {
            let s = self.streams.entry(seg.key).or_default();
            s.push(seg.ts_ms, seg.seq, &seg.data, &mut tmp);
            out.extend(tmp.into_iter().map(|p| (seg.key, p)));
            return;
        }
        // Flux inconnu : candidat à la signature.
        if !self.candidates.contains_key(&seg.key) && self.candidates.len() >= MAX_CANDIDATES {
            return;
        }
        let (s, hits) = self.candidates.entry(seg.key).or_default();
        s.push(seg.ts_ms, seg.seq, &seg.data, &mut tmp);
        *hits += tmp.iter().filter(|p| p.data.len() >= 2 && opcodes::SIGNATURE.contains(&[p.data[0], p.data[1]])).count() as u32;
        if *hits >= SIGNATURE_HITS {
            let (s, _) = self.candidates.remove(&seg.key).unwrap();
            log::info!("flux du jeu verrouillé par signature : {}", flow_label(&seg.key));
            self.locked.insert(seg.key);
            self.streams.insert(seg.key, s);
            out.extend(tmp.into_iter().map(|p| (seg.key, p)));
        }
    }

    /// Trous abandonnés, tous flux confondus (diagnostic).
    pub fn gaps(&self) -> u64 {
        self.streams.values().map(|s| s.gaps).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "etat", rename_all = "snake_case")]
pub enum CaptureStatus {
    /// Npcap absent ou inutilisable : message pour l'utilisateur.
    NpcapManquant { message: String },
    AttenteJeu,
    AttenteTrafic,
    Active { serveur: String },
}

pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    pub status: Arc<RwLock<CaptureStatus>>,
    /// Interface sur laquelle passe le trafic du jeu (panneau de diagnostic).
    pub interface_jeu: Arc<RwLock<Option<String>>>,
    enregistrement: Arc<Mutex<Option<Enregistrement>>>,
}

/// Enregistrement brut du trafic du jeu (mode diagnostic, 10.2), au format
/// de relecture (`replay`) : `ts_ms|src:sport>dst:dport|seq|hex`.
struct Enregistrement {
    fichier: std::io::BufWriter<std::fs::File>,
    chemin: std::path::PathBuf,
    fin_ms: i64,
    segments: u64,
}

impl CaptureHandle {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Enregistre le trafic du jeu pendant `duree_ms` dans `chemin`. Ces
    /// fichiers ne quittent jamais le PC et ne vont jamais dans le repo.
    pub fn enregistrer(&self, chemin: &std::path::Path, duree_ms: i64) -> std::io::Result<()> {
        if let Some(d) = chemin.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut f = std::io::BufWriter::new(std::fs::File::create(chemin)?);
        use std::io::Write;
        writeln!(f, "# LastView {} — trafic du jeu, {} s, format replay", env!("CARGO_PKG_VERSION"), duree_ms / 1000)?;
        *self.enregistrement.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Enregistrement { fichier: f, chemin: chemin.to_path_buf(), fin_ms: now_ms() + duree_ms, segments: 0 });
        log::info!("enregistrement du trafic : {} ({} s)", chemin.display(), duree_ms / 1000);
        Ok(())
    }

    /// Enregistrement en cours : secondes restantes et segments écrits.
    pub fn enregistrement(&self) -> Option<(i64, u64)> {
        self.enregistrement.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().map(|e| (((e.fin_ms - now_ms()) / 1000).max(0), e.segments))
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Shared {
    conns: RwLock<HashSet<system::Conn>>,
    /// Signature autorisée (jeu lancé, aucune connexion dans la table depuis un moment).
    signature: AtomicBool,
    stop: Arc<AtomicBool>,
    status: Arc<RwLock<CaptureStatus>>,
    interface_jeu: Arc<RwLock<Option<String>>>,
    enregistrement: Arc<Mutex<Option<Enregistrement>>>,
}

/// Démarre la capture. Les paquets du jeu arrivent sur `tx`.
/// `device` : nom Npcap d'une interface à utiliser seule (paramètre), sinon toutes.
pub fn start(device: Option<String>, tx: Sender<(FlowKey, GamePacket)>) -> CaptureHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let status = Arc::new(RwLock::new(CaptureStatus::AttenteJeu));
    let interface_jeu: Arc<RwLock<Option<String>>> = Arc::default();
    let enregistrement: Arc<Mutex<Option<Enregistrement>>> = Arc::default();
    let handle = CaptureHandle { stop: stop.clone(), status: status.clone(), interface_jeu: interface_jeu.clone(), enregistrement: enregistrement.clone() };
    let lib = match npcap::Pcap::load() {
        Ok(l) => Arc::new(l),
        Err(e) => {
            log::error!("{e}");
            *status.write().unwrap_or_else(std::sync::PoisonError::into_inner) = CaptureStatus::NpcapManquant { message: e };
            return handle;
        }
    };
    let shared = Arc::new(Shared { conns: RwLock::new(HashSet::new()), signature: AtomicBool::new(false), stop, status, interface_jeu, enregistrement });
    let (seg_tx, seg_rx) = crossbeam_channel::bounded::<Segment>(20_000);

    // 1. Suivi du jeu : processus et connexions, toutes les 2 s.
    {
        let shared = shared.clone();
        thread::Builder::new().name("lv-jeu".into()).spawn(move || crate::superviseur::superviser("jeu", Duration::from_secs(2), || track_game(&shared))).ok();
    }
    // 2. Interfaces : une par fil, nouvelles interfaces reprises toutes les 10 s
    //    (passage Wi-Fi ↔ Ethernet ↔ VPN).
    {
        let shared = shared.clone();
        thread::Builder::new()
            .name("lv-interfaces".into())
            .spawn(move || {
                // Interfaces ouvertes ; une interface perdue (VPN coupé) en sort et sera rouverte si elle revient.
                let opened: Arc<Mutex<HashSet<String>>> = Arc::default();
                while !shared.stop.load(Ordering::Relaxed) {
                    for dev in lib.devices() {
                        let wanted = match &device {
                            Some(d) => &dev.name == d,
                            None => dev.has_addresses || dev.is_loopback,
                        };
                        if wanted && opened.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(dev.name.clone()) {
                            let (lib, shared, tx, opened) = (lib.clone(), shared.clone(), seg_tx.clone(), opened.clone());
                            thread::Builder::new()
                                .name("lv-capture".into())
                                .spawn(move || {
                                    crate::superviseur::superviser("capture", Duration::from_secs(2), || capture_device(&lib, &dev, &shared, &tx));
                                    opened.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&dev.name);
                                })
                                .ok();
                        }
                    }
                    for _ in 0..10 {
                        if shared.stop.load(Ordering::Relaxed) {
                            return;
                        }
                        thread::sleep(Duration::from_secs(1));
                    }
                }
            })
            .ok();
    }
    // 3. Réassemblage : un seul fil pour tous les flux.
    {
        let shared = shared.clone();
        thread::Builder::new()
            .name("lv-assemblage".into())
            .spawn(move || crate::superviseur::superviser("assemblage", Duration::from_secs(2), || assemble(&shared, seg_rx.clone(), tx.clone())))
            .ok();
    }
    handle
}

fn track_game(shared: &Shared) {
    let mut since_no_conn: Option<i64> = None;
    let mut last_pids = Vec::new();
    while !shared.stop.load(Ordering::Relaxed) {
        let pids = system::find_game_pids();
        if pids != last_pids {
            if pids.is_empty() {
                log::info!("jeu fermé ou pas encore lancé");
            } else {
                for &p in &pids {
                    log::info!("AION2.exe PID {p} : {}", system::integrity(p));
                }
            }
            last_pids = pids.clone();
        }
        let conns = if pids.is_empty() { HashSet::new() } else { system::game_connections(&pids) };
        let now = now_ms();
        if pids.is_empty() {
            *shared.status.write().unwrap_or_else(std::sync::PoisonError::into_inner) = CaptureStatus::AttenteJeu;
            since_no_conn = None;
        } else if conns.is_empty() {
            let since = *since_no_conn.get_or_insert(now);
            shared.signature.store(now - since > SIGNATURE_DELAY_MS, Ordering::Relaxed);
        } else {
            since_no_conn = None;
            shared.signature.store(false, Ordering::Relaxed);
        }
        if !pids.is_empty() && !matches!(*shared.status.read().unwrap_or_else(std::sync::PoisonError::into_inner), CaptureStatus::Active { .. }) {
            *shared.status.write().unwrap_or_else(std::sync::PoisonError::into_inner) = CaptureStatus::AttenteTrafic;
        }
        if *shared.conns.read().unwrap_or_else(std::sync::PoisonError::into_inner) != conns {
            for &(ip, rport, lport) in &conns {
                log::info!("connexion du jeu {}:{rport} (port local {lport})", npcap::ip_to_string(ip));
            }
            *shared.conns.write().unwrap_or_else(std::sync::PoisonError::into_inner) = conns;
        }
        thread::sleep(Duration::from_secs(2));
    }
}

/// Erreurs de lecture de suite avant de fermer une interface.
const ERREURS_AVANT_FERMETURE: u32 = 5;

fn capture_device(lib: &npcap::Pcap, dev: &npcap::Device, shared: &Shared, tx: &Sender<Segment>) {
    let label = if dev.description.is_empty() { dev.name.clone() } else { dev.description.clone() };
    let mut handle = match lib.open(&dev.name, "tcp") {
        Ok(h) => h,
        Err(e) => {
            log::warn!("interface {label} non ouverte : {e}");
            return;
        }
    };
    log::info!("capture sur {label}");
    let datalink = handle.datalink;
    let mut erreurs = 0u32;
    while !shared.stop.load(Ordering::Relaxed) {
        let next = handle.next();
        if !matches!(next, npcap::Next::Error(_)) {
            erreurs = 0;
        }
        match next {
            npcap::Next::Packet { ts_ms, data } => {
                let Some(seg) = npcap::parse_tcp(datalink, data) else { continue };
                if seg.payload.is_empty() || seg.sport == 443 {
                    continue;
                }
                let known = shared.conns.read().unwrap_or_else(std::sync::PoisonError::into_inner).contains(&(seg.src, seg.sport, seg.dport));
                if !known && !shared.signature.load(Ordering::Relaxed) {
                    continue;
                }
                let s = Segment { ts_ms, key: (seg.src, seg.sport, seg.dst, seg.dport), seq: seg.seq, data: seg.payload.to_vec(), known };
                if known {
                    let changer = shared.interface_jeu.read().unwrap_or_else(std::sync::PoisonError::into_inner).as_deref() != Some(label.as_str());
                    if changer {
                        log::info!("trafic du jeu sur l'interface {label}");
                        *shared.interface_jeu.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(label.clone());
                    }
                }
                if tx.try_send(s).is_err() {
                    log::warn!("file de capture pleine : segment perdu");
                }
            }
            npcap::Next::Timeout => {}
            npcap::Next::Eof => break,
            // Interface disparue (VPN coupé, adaptateur retiré) : erreur à chaque lecture.
            // Avant le 14/11/2026, 24 885 lignes d'avertissement en une session :
            // désormais un seul avertissement, puis l'interface est fermée.
            npcap::Next::Error(e) => {
                erreurs += 1;
                if erreurs == 1 {
                    log::warn!("capture {label} : {e}");
                }
                if erreurs >= ERREURS_AVANT_FERMETURE {
                    log::warn!("interface {label} perdue : fermée, elle sera rouverte si elle revient");
                    return;
                }
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

fn enregistrer_segment(shared: &Shared, seg: &Segment) {
    use std::io::Write;
    let mut e = shared.enregistrement.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(en) = e.as_mut() else { return };
    let (s, sp, d, dp) = seg.key;
    let hex: String = seg.data.iter().map(|b| format!("{b:02x}")).collect();
    let ok = writeln!(en.fichier, "{}|{}:{sp}>{}:{dp}|{}|{hex}", seg.ts_ms, npcap::ip_to_string(s), npcap::ip_to_string(d), seg.seq).is_ok();
    en.segments += 1;
    if !ok || now_ms() >= en.fin_ms {
        let _ = en.fichier.flush();
        log::info!("enregistrement terminé : {} segments dans {}", en.segments, en.chemin.display());
        *e = None;
    }
}

fn assemble(shared: &Shared, rx: Receiver<Segment>, tx: Sender<(FlowKey, GamePacket)>) {
    let mut asm = Assembler::new();
    let mut out = Vec::new();
    let mut last_packet = 0i64;
    while !shared.stop.load(Ordering::Relaxed) {
        let seg = match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(s) => s,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if last_packet != 0 && now_ms() - last_packet > 5_000 {
                    let mut st = shared.status.write().unwrap_or_else(std::sync::PoisonError::into_inner);
                    if matches!(*st, CaptureStatus::Active { .. }) {
                        *st = CaptureStatus::AttenteTrafic;
                    }
                }
                continue;
            }
            Err(_) => return,
        };
        enregistrer_segment(shared, &seg);
        asm.push(&seg, &mut out);
        if !out.is_empty() {
            last_packet = now_ms();
            let serveur = flow_label(&out[0].0);
            {
                let mut st = shared.status.write().unwrap_or_else(std::sync::PoisonError::into_inner);
                if *st != (CaptureStatus::Active { serveur: serveur.clone() }) {
                    log::info!("capture active : {serveur}");
                    *st = CaptureStatus::Active { serveur };
                }
            }
            for p in out.drain(..) {
                if tx.send(p).is_err() {
                    return;
                }
            }
        }
    }
}
