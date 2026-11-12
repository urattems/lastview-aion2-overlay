//! lv-recorder — outil de l'étape 0 de LastView.
//!
//! `record <dossier>`  : enregistre le trafic serveur → client du jeu (copie
//!                       Npcap des connexions TCP du processus AION2.exe) et
//!                       l'horodatage des clics souris (sondage en lecture
//!                       seule). Aucune interaction avec le jeu.
//! `analyze <dossier>` : cherche le paquet qui suit chaque clic de ciblage.
//!
//! 100% passif : jamais de lecture mémoire, d'injection ni d'envoi de paquet.

mod analyze;
mod framing;
mod game;
mod pcap;
mod record;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let usage = "usage:\n  lv-recorder record <dossier> [--sans-bips] [--bips 12]\n  lv-recorder analyze <dossier> [--window-ms N]\n  lv-recorder dump <dossier>\n  lv-recorder devices";
    match args.get(1).map(String::as_str) {
        Some("record") => {
            let dir = args.get(2).cloned().unwrap_or_else(|| "captures".to_string());
            let metronome = !args.iter().any(|a| a == "--sans-bips");
            let cues = args
                .iter()
                .position(|a| a == "--bips")
                .and_then(|i| args.get(i + 1))
                .and_then(|v| v.parse().ok())
                .unwrap_or(12u32);
            if let Err(e) = record::run(&dir, metronome, cues) {
                eprintln!("erreur: {e}");
                std::process::exit(1);
            }
        }
        Some("analyze") => {
            let dir = args.get(2).cloned().unwrap_or_else(|| "captures".to_string());
            let window = args
                .iter()
                .position(|a| a == "--window-ms")
                .and_then(|i| args.get(i + 1))
                .and_then(|v| v.parse().ok());
            if let Err(e) = analyze::run(&dir, window) {
                eprintln!("erreur: {e}");
                std::process::exit(1);
            }
        }
        Some("dump") => {
            let dir = args.get(2).cloned().unwrap_or_else(|| "captures".to_string());
            if let Err(e) = analyze::dump(&dir) {
                eprintln!("erreur: {e}");
                std::process::exit(1);
            }
        }
        Some("devices") => match pcap::Pcap::load() {
            Ok(lib) => {
                for d in lib.devices() {
                    println!("{} | {} | loopback={} addrs={}", d.name, d.description, d.is_loopback, d.has_addresses);
                }
            }
            Err(e) => eprintln!("Npcap introuvable: {e}"),
        },
        _ => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    }
}
