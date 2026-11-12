//! `lastview-dump` : le cœur de LastView en ligne de commande, sans fenêtre.
//! Sert à la porte 4 (dump lisible) et au diagnostic.
//!
//! ```text
//! lastview-dump live   [--db <fichier>] [--minutes N]   écoute le jeu, affiche les rencontres
//! lastview-dump replay <dossier> [--db <fichier>]      relit un enregistrement de lv-recorder
//! lastview-dump liste  [--db <fichier>]                affiche la liste en base
//! lastview-dump api    <pseudo> [serverId]             appel API complet (recherche, info, équipement)
//! ```

use std::path::PathBuf;
use std::time::{Duration, Instant};

use lastview_core::api::{ApiClient, CACHE_MS};
use lastview_core::capture::{self, now_ms, Assembler};
use lastview_core::store::{self, Row, Store, DEFAULT_MAX_PLAYERS, DEFAULT_RETENTION_MS};
use lastview_core::{diagnostics, replay, Encounter, Engine};

fn usage() -> ! {
    eprintln!(
        "Usage :\n  lastview-dump live   [--db <fichier>] [--minutes N]\n  lastview-dump replay <dossier> [--db <fichier>]\n  lastview-dump liste  [--db <fichier>]\n  lastview-dump api    <pseudo> [serverId]"
    );
    std::process::exit(2);
}

fn opt(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn fmt_ts(ts: i64) -> String {
    let s = ts / 1000;
    format!("{:02}:{:02}:{:02}", (s / 3600) % 24, (s / 60) % 60, s % 60)
}

fn print_encounter(e: &Encounter) {
    let p = &e.player;
    println!(
        "[{} UTC] {:7} {:14} srv {:4} {:13} niv {:2} GS {:5} CP {:7} {}",
        fmt_ts(e.ts_ms),
        e.kind.label(),
        p.name,
        p.server,
        p.class.unwrap_or("?"),
        p.level,
        p.gs,
        p.cp,
        if p.guild.is_empty() { String::new() } else { format!("<{}>", p.guild) }
    );
}

fn print_rows(rows: &[Row]) {
    println!("\n{} joueur(s) dans la liste :", rows.len());
    for r in rows {
        println!(
            "  {} {:14} srv {:4} {:13} niv {:2} GS {:5} CP {:7} {:7} ×{} {}",
            if r.pinned { '★' } else { ' ' },
            r.name,
            r.server,
            r.class.unwrap_or("?"),
            r.level,
            r.gs,
            r.cp,
            r.last_kind.label(),
            r.count,
            if r.guild.is_empty() { String::new() } else { format!("<{}>", r.guild) }
        );
    }
}

fn print_engine(engine: &Engine) {
    let l = &engine.world.local;
    println!("\nPaquets lus : {}", engine.stats.packets);
    println!("Joueurs en mémoire silencieuse : {}", engine.world.player_count());
    println!(
        "Joueur local : {}",
        match l.key {
            Some(k) => format!("id {k} {}{}", if l.name.is_empty() { String::new() } else { format!("({} srv {}) ", l.name, l.server) }, if l.confirmed { "confirmé (33 36)" } else { "deviné (4A 36), non confirmé" }),
            None => "inconnu".into(),
        }
    );
    println!("Rencontres en attente d'un pseudo : {}", engine.pending_count());
    println!("Paquets décodés par opcode : {:?}", engine.stats.decoded);
    if engine.stats.unreadable.is_empty() {
        println!("Paquets illisibles : aucun");
    } else {
        println!("Paquets illisibles : {:?}", engine.stats.unreadable);
    }
}

fn open_store(args: &[String], default: Option<PathBuf>) -> Store {
    let path = opt(args, "--db").map(PathBuf::from).or(default);
    match path {
        Some(p) => {
            println!("Base : {}", p.display());
            Store::open(&p).unwrap_or_else(|e| {
                eprintln!("base {} : {e}", p.display());
                std::process::exit(1)
            })
        }
        None => Store::open_in_memory().expect("base en mémoire"),
    }
}

fn hint_local(engine: &mut Engine, store: &Store) {
    let name = store.setting("mon_perso").ok().flatten().unwrap_or_else(|| "Urattems".into());
    let server = store.setting("mon_serveur").ok().flatten().and_then(|s| s.parse().ok()).unwrap_or(1303);
    engine.world.set_local_hint(name.trim_matches('"'), server);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(cmd) = args.get(1) else { usage() };
    match cmd.as_str() {
        "replay" => {
            let Some(dir) = args.get(2) else { usage() };
            diagnostics::init(&std::env::temp_dir().join("lastview-dump"), false);
            let mut store = open_store(&args, None);
            let mut engine = Engine::new();
            hint_local(&mut engine, &store);
            let segs = replay::read_segments(std::path::Path::new(dir)).unwrap_or_else(|e| {
                eprintln!("{dir} : {e}");
                std::process::exit(1)
            });
            println!("{} segments relus depuis {dir}\n", segs.len());
            let mut asm = Assembler::new();
            let mut out = Vec::new();
            let mut last_ts = 0;
            for s in &segs {
                asm.push(s, &mut out);
                for (_, p) in out.drain(..) {
                    last_ts = p.ts_ms;
                    for e in engine.handle(&p) {
                        print_encounter(&e);
                        let _ = store.record(&e);
                    }
                }
            }
            print_engine(&engine);
            println!("Trous TCP abandonnés : {}", asm.gaps());
            print_rows(&store.list(last_ts, DEFAULT_RETENTION_MS).unwrap_or_default());
        }
        "live" => {
            diagnostics::init(&diagnostics::log_dir(), true);
            let mut store = open_store(&args, Some(store::default_db_path()));
            let minutes: u64 = opt(&args, "--minutes").and_then(|m| m.parse().ok()).unwrap_or(0);
            let mut engine = Engine::new();
            hint_local(&mut engine, &store);
            let (tx, rx) = crossbeam_channel::unbounded();
            let cap = capture::start(opt(&args, "--interface"), tx);
            println!("Écoute du jeu. Ctrl+C pour arrêter{}.", if minutes > 0 { format!(" (arrêt seul après {minutes} min)") } else { String::new() });
            let start = Instant::now();
            let mut last_report = Instant::now();
            let mut last_status = String::new();
            loop {
                if minutes > 0 && start.elapsed() > Duration::from_secs(minutes * 60) {
                    break;
                }
                if let Ok((_, p)) = rx.recv_timeout(Duration::from_millis(500)) {
                    for e in engine.handle(&p) {
                        print_encounter(&e);
                        if let Err(err) = store.record(&e) {
                            eprintln!("base : {err}");
                        }
                    }
                }
                let st = format!("{:?}", cap.status.read().unwrap());
                if st != last_status {
                    println!("[état] {st}");
                    last_status = st;
                }
                if last_report.elapsed() > Duration::from_secs(30) {
                    last_report = Instant::now();
                    let _ = store.purge(now_ms(), DEFAULT_RETENTION_MS, DEFAULT_MAX_PLAYERS);
                    println!(
                        "[{} s] paquets {} | mémoire {} joueurs | moi {:?}{} | rencontres {} | illisibles {:?}",
                        start.elapsed().as_secs(),
                        engine.stats.packets,
                        engine.world.player_count(),
                        engine.world.local.key,
                        if engine.world.local.confirmed { " (confirmé)" } else { "" },
                        engine.stats.encounters,
                        engine.stats.unreadable
                    );
                }
            }
            cap.stop();
            print_engine(&engine);
            print_rows(&store.list(now_ms(), DEFAULT_RETENTION_MS).unwrap_or_default());
        }
        "liste" => {
            let store = open_store(&args, Some(store::default_db_path()));
            print_rows(&store.list(now_ms(), DEFAULT_RETENTION_MS).unwrap_or_default());
        }
        "api" => {
            let Some(name) = args.get(2) else { usage() };
            diagnostics::init(&std::env::temp_dir().join("lastview-dump"), true);
            let server: Option<u16> = args.get(3).and_then(|s| s.parse().ok());
            let client = ApiClient::new(Some(&store::default_db_path()));
            let t = Instant::now();
            let hits = match client.search(name, server) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Recherche : {e}");
                    std::process::exit(1)
                }
            };
            println!("Recherche « {name} » : {} résultat(s) exact(s)", hits.len());
            for h in &hits {
                println!("  {} — {} ({}) niv {} pcId {}", h.name, h.server_name, h.server_id, h.level, h.pc_id);
            }
            let Some(h) = hits.first() else { return };
            match client.info(&h.character_id, h.server_id, CACHE_MS) {
                Ok(info) => {
                    let s = ApiClient::summary(&info);
                    println!("\nProfil : {} — {} niv {} — {} — CP {} — GS {} — titre « {} »", s.name, s.class, s.level, s.server, s.cp, s.gs, s.title);
                }
                Err(e) => println!("info : {e}"),
            }
            match client.equipment(&h.character_id, h.server_id, CACHE_MS) {
                Ok(eq) => {
                    let items = eq.pointer("/equipment/equipmentList").and_then(|l| l.as_array()).map(|l| l.len()).unwrap_or(0);
                    let skills = eq.pointer("/skill/skillList").and_then(|l| l.as_array()).map(|l| l.len()).unwrap_or(0);
                    println!("Équipement : {items} pièces, {skills} compétences");
                    if let Some(first) = eq.pointer("/equipment/equipmentList/0") {
                        println!(
                            "  ex. {} +{} (dépassement {}) emplacement {}",
                            first.get("name").and_then(|v| v.as_str()).unwrap_or("?"),
                            first.get("enchantLevel").and_then(|v| v.as_i64()).unwrap_or(0),
                            first.get("exceedLevel").and_then(|v| v.as_i64()).unwrap_or(0),
                            first.get("slotPosName").and_then(|v| v.as_str()).unwrap_or("?")
                        );
                    }
                }
                Err(e) => println!("équipement : {e}"),
            }
            println!("\nTerminé en {:.1} s (cache de 10 min, 1 requête par seconde au plus).", t.elapsed().as_secs_f32());
        }
        _ => usage(),
    }
}
