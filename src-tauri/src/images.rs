//! Cache disque des images NCSoft (icônes, portraits) : `%APPDATA%\LastView\img`,
//! 200 Mo au plus, les plus anciennes supprimées d'abord (section 4.10).
//!
//! La fiche charge ses images par `http://lvimg.localhost/<url encodée>`
//! (protocole `lvimg` de Tauri). On sert le fichier en cache, sinon on le
//! télécharge une fois. Seuls les domaines d'images de NCSoft sont acceptés.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;

use tauri::http::{Response, StatusCode};

pub const MAX_BYTES: u64 = 200 * 1024 * 1024;
const HOTES: &[&str] = &["assets.playnccdn.com", "profileimg.plaync.com", "fizz-download.playnccdn.com"];

pub fn dir() -> PathBuf {
    lastview_core::store::data_dir().join("img")
}

fn agent() -> &'static ureq::Agent {
    static A: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    A.get_or_init(|| ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).user_agent(concat!("LastView/", env!("CARGO_PKG_VERSION"))).build())
}

fn autorisee(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?']).next().unwrap_or("");
    HOTES.contains(&host)
}

fn type_mime(octets: &[u8]) -> &'static str {
    match octets {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xFF, 0xD8, ..] => "image/jpeg",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        [b'G', b'I', b'F', ..] => "image/gif",
        _ => "application/octet-stream",
    }
}

fn reponse(code: StatusCode, mime: &str, corps: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(code)
        .header("Content-Type", mime)
        .header("Cache-Control", "max-age=86400")
        .header("Access-Control-Allow-Origin", "*")
        .body(corps)
        .unwrap()
}

/// Sert une image : `chemin` = la partie après `http://lvimg.localhost/`.
pub fn servir(chemin: &str) -> Response<Vec<u8>> {
    let url = percent_encoding::percent_decode_str(chemin.trim_start_matches('/')).decode_utf8_lossy().into_owned();
    if !autorisee(&url) {
        return reponse(StatusCode::FORBIDDEN, "text/plain", b"domaine refuse".to_vec());
    }
    let mut h = DefaultHasher::new();
    url.hash(&mut h);
    let fichier = dir().join(format!("{:016x}.img", h.finish()));
    if let Ok(octets) = std::fs::read(&fichier) {
        // Fichier utilisé : on le « rajeunit » pour le nettoyage (le plus ancien part d'abord).
        let _ = std::fs::File::options().append(true).open(&fichier).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        return reponse(StatusCode::OK, type_mime(&octets), octets);
    }
    match agent().get(&url).call() {
        Ok(r) => {
            let mut octets = Vec::new();
            use std::io::Read;
            if r.into_reader().take(10 * 1024 * 1024).read_to_end(&mut octets).is_err() {
                return reponse(StatusCode::BAD_GATEWAY, "text/plain", Vec::new());
            }
            let _ = std::fs::create_dir_all(dir());
            let _ = std::fs::write(&fichier, &octets);
            nettoyer();
            reponse(StatusCode::OK, type_mime(&octets), octets)
        }
        Err(e) => {
            log::debug!("image {url} : {e}");
            reponse(StatusCode::BAD_GATEWAY, "text/plain", Vec::new())
        }
    }
}

/// Garde le dossier sous [`MAX_BYTES`] en supprimant les fichiers les plus anciens.
pub fn nettoyer() {
    let Ok(entrees) = std::fs::read_dir(dir()) else { return };
    let mut fichiers: Vec<(std::time::SystemTime, u64, PathBuf)> = entrees
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file().then(|| (m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = fichiers.iter().map(|f| f.1).sum();
    if total <= MAX_BYTES {
        return;
    }
    fichiers.sort_by_key(|f| f.0);
    for (_, taille, p) in fichiers {
        if total <= MAX_BYTES * 9 / 10 {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            total -= taille;
        }
    }
}
