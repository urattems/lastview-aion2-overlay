//! Journal de l'app : `%APPDATA%\LastView\logs\lastview.log`.
//!
//! Niveau normal : infos et erreurs. Mode diagnostic : tout, y compris les
//! paquets illisibles. Le fichier tourne à 5 Mo (un seul ancien gardé).
//! Aucune donnée ne quitte le PC.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

struct Logger {
    path: PathBuf,
    file: Mutex<Option<File>>,
    stderr: bool,
    diagnostic: AtomicBool,
}

static LOGGER: std::sync::OnceLock<Logger> = std::sync::OnceLock::new();

pub fn log_dir() -> PathBuf {
    crate::store::data_dir().join("logs")
}

fn open(path: &Path) -> Option<File> {
    if let Ok(m) = std::fs::metadata(path) {
        if m.len() > MAX_LOG_BYTES {
            let _ = std::fs::rename(path, path.with_extension("old.log"));
        }
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// Installe le journal. `stderr` : recopie aussi dans la console (outils).
pub fn init(dir: &Path, stderr: bool) {
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("lastview.log");
    let logger = LOGGER.get_or_init(|| Logger { file: Mutex::new(open(&path)), path, stderr, diagnostic: AtomicBool::new(false) });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

/// Mode diagnostic : niveau détaillé.
pub fn set_diagnostic(on: bool) {
    if let Some(l) = LOGGER.get() {
        l.diagnostic.store(on, Ordering::Relaxed);
    }
    log::set_max_level(if on { LevelFilter::Debug } else { LevelFilter::Info });
}

impl Log for Logger {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= Level::Info || self.diagnostic.load(Ordering::Relaxed)
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!("{} {:5} {}\n", crate::capture::now_ms(), r.level(), r.args());
        if self.stderr {
            eprint!("{line}");
        }
        let mut f = self.file.lock().unwrap();
        if let Some(file) = f.as_mut() {
            let _ = file.write_all(line.as_bytes());
            if file.metadata().map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false) {
                *f = open(&self.path);
            }
        }
    }

    fn flush(&self) {
        if let Some(f) = self.file.lock().unwrap().as_mut() {
            let _ = f.flush();
        }
    }
}
