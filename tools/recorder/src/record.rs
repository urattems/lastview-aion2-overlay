//! Enregistrement : trafic serveur → client des connexions du jeu, et clics.
//!
//! Fichiers produits dans le dossier donné :
//! - `capture.txt` : `ts_ms|srcip:sport>dstip:dport|seq|hex` par segment TCP
//!   serveur → client avec données (format proche d'A2Tools, + numéro de
//!   séquence pour remettre en ordre à l'analyse).
//! - `clicks.txt`  : `ts_ms|event|a|b|c|d|e`, avec `event` =
//!   - `down` / `up` / `marker` : clic gauche ou F8 (`cursor_visible|x|y|game_fg|alt`).
//!     Invisible quand le jeu a le focus s'il tourne avec plus de privilèges.
//!   - `beep` : bip du métronome, `a` = `A` ou `B` (le joueur à cibler).
//!   - `phase` : `a` = `prep`, `reference`, `test`, `end`.
//!   - `cursor` : changement d'état du curseur (`visible|x|y|game_fg|0`).
//!   - `info` : `a` = texte libre (privilèges du jeu…).
//! Les fichiers restent sur le PC (ignorés par git).
//!
//! Métronome (par défaut) : 30 s de préparation, 3 bips courts, 20 s de
//! référence sans cibler, puis `cues` bips toutes les 5 s (aigu = A, grave =
//! B), deux bips longs, et l'enregistrement s'arrête seul.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::game;
use crate::pcap::{self, Pcap};

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}

struct Shared {
    conns: RwLock<HashSet<game::Conn>>,
    game_pids: RwLock<Vec<u32>>,
    capture: Mutex<BufWriter<File>>,
    clicks: Mutex<BufWriter<File>>,
    packets: AtomicU64,
    bytes: AtomicU64,
    click_count: AtomicU64,
    stop: AtomicBool,
}

fn log_event(shared: &Shared, line: &str) {
    let mut c = shared.clicks.lock().unwrap();
    let _ = writeln!(c, "{}|{}", now_ms(), line);
    let _ = c.flush();
}

pub fn run(dir: &str, metronome: bool, cues: u32) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("dossier {dir}: {e}"))?;
    let lib = Arc::new(Pcap::load()?);
    let capture = File::create(format!("{dir}/capture.txt")).map_err(|e| e.to_string())?;
    let clicks = File::create(format!("{dir}/clicks.txt")).map_err(|e| e.to_string())?;
    let shared = Arc::new(Shared {
        conns: RwLock::new(HashSet::new()),
        game_pids: RwLock::new(Vec::new()),
        capture: Mutex::new(BufWriter::new(capture)),
        clicks: Mutex::new(BufWriter::new(clicks)),
        packets: AtomicU64::new(0),
        bytes: AtomicU64::new(0),
        click_count: AtomicU64::new(0),
        stop: AtomicBool::new(false),
    });
    {
        let mut w = shared.capture.lock().unwrap();
        writeln!(w, "# lv-recorder capture {} | ts_ms|src:sport>dst:dport|seq|hex", now_ms()).ok();
        w.flush().ok();
        let mut c = shared.clicks.lock().unwrap();
        writeln!(c, "# lv-recorder clicks {} | ts_ms|event|cursor_visible|x|y|game_fg|alt", now_ms()).ok();
        c.flush().ok();
    }

    println!("LastView — enregistreur (étape 0). Ctrl+C pour arrêter.");
    println!("Dossier : {dir}");

    // 1. Suivi du jeu : PID + connexions, toutes les 2 s.
    {
        let shared = shared.clone();
        thread::spawn(move || {
            let mut last_pids: Vec<u32> = Vec::new();
            loop {
                if shared.stop.load(Ordering::Relaxed) {
                    break;
                }
                let pids = game::find_game_pids();
                if pids != last_pids {
                    if pids.is_empty() {
                        println!("[jeu] AION2.exe introuvable : en attente du jeu…");
                    } else {
                        println!("[jeu] AION2.exe trouvé, PID {pids:?}");
                        for &p in &pids {
                            let info = game::integrity(p);
                            println!("[jeu] PID {p} : {info}");
                            log_event(&shared, &format!("info|PID {p} : {info}|0|0|0|0"));
                        }
                        let me = game::integrity(std::process::id());
                        println!("[jeu] lv-recorder : {me}");
                        log_event(&shared, &format!("info|lv-recorder : {me}|0|0|0|0"));
                    }
                    last_pids = pids.clone();
                }
                *shared.game_pids.write().unwrap() = pids.clone();
                let conns = if pids.is_empty() { HashSet::new() } else { game::game_connections(&pids) };
                let changed = {
                    let cur = shared.conns.read().unwrap();
                    *cur != conns
                };
                if changed {
                    for &(ip, rport, lport) in &conns {
                        println!("[jeu] connexion {}:{} ← port local {}", pcap::ip_to_string(ip), rport, lport);
                    }
                    *shared.conns.write().unwrap() = conns;
                }
                thread::sleep(Duration::from_secs(2));
            }
        });
    }

    // 2. Capture : un thread par périphérique.
    let devices: Vec<_> = lib.devices().into_iter().filter(|d| d.has_addresses || d.is_loopback).collect();
    if devices.is_empty() {
        return Err("aucune interface réseau Npcap".into());
    }
    for dev in devices {
        let lib = lib.clone();
        let shared = shared.clone();
        thread::spawn(move || {
            let mut handle = match lib.open(&dev.name, "tcp") {
                Ok(h) => h,
                Err(e) => {
                    println!("[capture] {} : non ouverte ({e})", dev.description);
                    return;
                }
            };
            println!("[capture] {} (datalink {})", if dev.description.is_empty() { &dev.name } else { &dev.description }, handle.datalink);
            let datalink = handle.datalink;
            loop {
                if shared.stop.load(Ordering::Relaxed) {
                    break;
                }
                match handle.next() {
                    pcap::Next::Packet { ts_ms, data } => {
                        let Some(seg) = pcap::parse_tcp(datalink, data) else { continue };
                        if seg.payload.is_empty() {
                            continue;
                        }
                        let is_game = {
                            let conns = shared.conns.read().unwrap();
                            conns.contains(&(seg.src, seg.sport, seg.dport))
                        };
                        if !is_game {
                            continue;
                        }
                        let mut line = String::with_capacity(seg.payload.len() * 2 + 64);
                        use std::fmt::Write as _;
                        let _ = write!(
                            line,
                            "{}|{}:{}>{}:{}|{}|",
                            ts_ms,
                            pcap::ip_to_string(seg.src),
                            seg.sport,
                            pcap::ip_to_string(seg.dst),
                            seg.dport,
                            seg.seq
                        );
                        for b in seg.payload {
                            let _ = write!(line, "{b:02x}");
                        }
                        let mut w = shared.capture.lock().unwrap();
                        let _ = writeln!(w, "{line}");
                        let _ = w.flush();
                        shared.packets.fetch_add(1, Ordering::Relaxed);
                        shared.bytes.fetch_add(seg.payload.len() as u64, Ordering::Relaxed);
                    }
                    pcap::Next::Timeout => {}
                    pcap::Next::Eof => break,
                    pcap::Next::Error(e) => {
                        println!("[capture] {} : erreur {e}", dev.description);
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        });
    }

    // 3. Souris : sondage 4 ms, lecture seule.
    {
        let shared = shared.clone();
        thread::spawn(move || {
            let mut was_down = false;
            let mut f8_down = false;
            loop {
                if shared.stop.load(Ordering::Relaxed) {
                    break;
                }
                let down = game::key_down(game::VK_LBUTTON);
                let f8 = game::key_down(game::VK_F8);
                if down != was_down || (f8 && !f8_down) {
                    let (visible, x, y) = game::cursor();
                    let game_fg = {
                        let pids = shared.game_pids.read().unwrap();
                        let fg = game::foreground_pid();
                        fg != 0 && pids.contains(&fg)
                    };
                    let alt = game::key_down(game::VK_MENU);
                    let event = if f8 && !f8_down { "marker" } else if down { "down" } else { "up" };
                    let mut c = shared.clicks.lock().unwrap();
                    let _ = writeln!(c, "{}|{}|{}|{}|{}|{}|{}", now_ms(), event, visible as u8, x, y, game_fg as u8, alt as u8);
                    let _ = c.flush();
                    if event == "down" {
                        shared.click_count.fetch_add(1, Ordering::Relaxed);
                    }
                    if event == "marker" {
                        println!("[clics] marqueur F8 posé");
                    }
                }
                was_down = down;
                f8_down = f8;
                thread::sleep(Duration::from_millis(4));
            }
        });
    }

    // 4. État du curseur (visible ou non, jeu au premier plan), sondé à ~30 Hz.
    //    GetCursorInfo n'est pas une lecture d'entrée : à vérifier qu'il reste
    //    lisible quand le jeu a le focus (c'est ce qu'utilisera la bulle).
    {
        let shared = shared.clone();
        thread::spawn(move || {
            let mut last: Option<(bool, bool)> = None;
            let mut last_log = 0i64;
            loop {
                if shared.stop.load(Ordering::Relaxed) {
                    break;
                }
                let game_fg = {
                    let pids = shared.game_pids.read().unwrap();
                    let fg = game::foreground_pid();
                    fg != 0 && pids.contains(&fg)
                };
                let (visible, x, y) = game::cursor();
                let state = (visible, game_fg);
                let now = now_ms();
                if last != Some(state) || (game_fg && visible && now - last_log > 1000) {
                    log_event(&shared, &format!("cursor|{}|{x}|{y}|{}|0", visible as u8, game_fg as u8));
                    last = Some(state);
                    last_log = now;
                }
                thread::sleep(Duration::from_millis(33));
            }
        });
    }

    // 5. Métronome : donne l'heure des ciblages sans lire la souris.
    if metronome {
        let shared = shared.clone();
        thread::spawn(move || {
            // Attendre que la connexion du jeu soit vue.
            while shared.conns.read().unwrap().is_empty() {
                thread::sleep(Duration::from_millis(500));
            }
            println!("\n=== MÉTRONOME ===");
            println!("Retourne dans le jeu maintenant. Le test démarre dans 30 s (3 bips courts).");
            log_event(&shared, "phase|prep|0|0|0|0");
            for left in (1..=30).rev() {
                if left % 5 == 0 {
                    println!("  … {left} s");
                }
                thread::sleep(Duration::from_secs(1));
            }
            log_event(&shared, "phase|reference|0|0|0|0");
            for _ in 0..3 {
                game::beep(1000, 120);
                thread::sleep(Duration::from_millis(120));
            }
            println!("[métronome] Référence : 20 s SANS rien cibler.");
            thread::sleep(Duration::from_secs(20));
            log_event(&shared, "phase|test|0|0|0|0");
            println!("[métronome] Test : à chaque bip, Alt + clic. Bip AIGU = joueur A, bip GRAVE = joueur B.");
            for i in 0..cues {
                let who = if i % 2 == 0 { "A" } else { "B" };
                log_event(&shared, &format!("beep|{who}|0|0|0|0"));
                println!("[métronome] bip {}/{} : {}", i + 1, cues, who);
                if who == "A" {
                    game::beep(1400, 250);
                } else {
                    game::beep(500, 250);
                }
                thread::sleep(Duration::from_millis(4750));
            }
            log_event(&shared, "phase|end|0|0|0|0");
            println!("[métronome] Fin du test : ne cible plus personne. Arrêt dans 10 s.");
            game::beep(800, 700);
            thread::sleep(Duration::from_millis(200));
            game::beep(800, 700);
            thread::sleep(Duration::from_secs(10));
            shared.stop.store(true, Ordering::Relaxed);
            {
                let mut w = shared.capture.lock().unwrap();
                let _ = w.flush();
            }
            println!("[métronome] Enregistrement terminé. Lance maintenant : lv-recorder analyze <dossier>");
            std::process::exit(0);
        });
    }

    // 6. État toutes les 5 s.
    let start = now_ms();
    loop {
        thread::sleep(Duration::from_secs(5));
        let elapsed = (now_ms() - start) / 1000;
        let conns = shared.conns.read().unwrap().len();
        println!(
            "[{:>4} s] paquets serveur→client : {} ({} Ko) | connexions du jeu : {} | clics : {}",
            elapsed,
            shared.packets.load(Ordering::Relaxed),
            shared.bytes.load(Ordering::Relaxed) / 1024,
            conns,
            shared.click_count.load(Ordering::Relaxed)
        );
    }
}
