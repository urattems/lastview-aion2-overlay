//! Base locale SQLite (`%APPDATA%\LastView\lastview.db`, hors du repo).
//!
//! - `players` : un joueur par ligne (clé durable pseudo + serveur, repli
//!   pseudo + classe), dernières valeurs connues, épingle.
//! - `encounters` : chaque rencontre (joueur, type, heure).
//! - `api_cache` : réponses JSON de l'API NCSoft, avec leur date.
//! - `settings` : paramètres (clé → valeur JSON).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::encounters::{Encounter, Kind};
use crate::protocol::class_name;

/// Une même rencontre ne réécrit pas la base plus d'une fois toutes les 5 s par joueur.
pub const ANTI_SPAM_MS: i64 = 5_000;
/// Deux événements séparés de moins de 10 min comptent pour une seule rencontre.
pub const SAME_ENCOUNTER_MS: i64 = 10 * 60 * 1000;
pub const DEFAULT_RETENTION_MS: i64 = 4 * 3600 * 1000;
pub const DEFAULT_MAX_PLAYERS: usize = 150;

/// Dossier de données de l'app : `%APPDATA%\LastView`.
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("LastView")
}

pub fn default_db_path() -> PathBuf {
    data_dir().join("lastview.db")
}

/// Une ligne de la liste (bulle).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    pub id: i64,
    pub name: String,
    pub server: u16,
    pub pc_id: i32,
    pub class: Option<&'static str>,
    pub level: i32,
    pub gs: i32,
    pub cp: i64,
    pub guild: String,
    pub abyss_rank: i32,
    pub character_id: Option<String>,
    pub last_seen_ms: i64,
    pub last_kind: Kind,
    pub pinned: bool,
    /// Rencontres distinctes dans la fenêtre de rétention.
    pub count: u32,
}

pub struct Store {
    conn: Connection,
    last_write: HashMap<i64, (i64, Kind)>,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS players (
                 id INTEGER PRIMARY KEY,
                 name TEXT NOT NULL,
                 server_id INTEGER NOT NULL DEFAULT 0,
                 pc_id INTEGER NOT NULL DEFAULT 0,
                 level INTEGER NOT NULL DEFAULT 0,
                 gs INTEGER NOT NULL DEFAULT 0,
                 cp INTEGER NOT NULL DEFAULT 0,
                 guild TEXT NOT NULL DEFAULT '',
                 abyss_rank INTEGER NOT NULL DEFAULT 0,
                 character_id TEXT,
                 first_seen INTEGER NOT NULL,
                 last_seen INTEGER NOT NULL,
                 last_kind TEXT NOT NULL,
                 pinned INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS players_name ON players(name COLLATE NOCASE, server_id);
             CREATE INDEX IF NOT EXISTS players_last_seen ON players(last_seen);
             CREATE TABLE IF NOT EXISTS encounters (
                 id INTEGER PRIMARY KEY,
                 player_id INTEGER NOT NULL REFERENCES players(id) ON DELETE CASCADE,
                 kind TEXT NOT NULL,
                 ts INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS encounters_player ON encounters(player_id, ts);
             CREATE TABLE IF NOT EXISTS api_cache (
                 url TEXT PRIMARY KEY,
                 body TEXT NOT NULL,
                 fetched_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS settings (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )?;
        Ok(Self { conn, last_write: HashMap::new() })
    }

    /// Trouve le joueur : pseudo + serveur, sinon pseudo + classe. Fusionne
    /// une entrée sans serveur dès que le serveur est appris.
    fn find_player(&self, name: &str, server: u16, pc_id: i32) -> rusqlite::Result<Option<i64>> {
        if server != 0 {
            let id: Option<i64> = self
                .conn
                .query_row("SELECT id FROM players WHERE name = ?1 COLLATE NOCASE AND server_id = ?2", params![name, server], |r| r.get(0))
                .optional()?;
            if id.is_some() {
                return Ok(id);
            }
            let id: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM players WHERE name = ?1 COLLATE NOCASE AND server_id = 0 AND (pc_id = ?2 OR pc_id = 0 OR ?2 = 0) ORDER BY last_seen DESC LIMIT 1",
                    params![name, pc_id],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(id) = id {
                self.conn.execute("UPDATE players SET server_id = ?1 WHERE id = ?2", params![server, id])?;
            }
            return Ok(id);
        }
        self.conn
            .query_row(
                "SELECT id FROM players WHERE name = ?1 COLLATE NOCASE AND (pc_id = ?2 OR ?2 = 0 OR pc_id = 0) ORDER BY server_id = 0 DESC, last_seen DESC LIMIT 1",
                params![name, pc_id],
                |r| r.get(0),
            )
            .optional()
    }

    /// Enregistre une rencontre. Renvoie l'id du joueur, ou `None` si
    /// l'anti-spam l'a écartée.
    pub fn record(&mut self, e: &Encounter) -> rusqlite::Result<Option<i64>> {
        let p = &e.player;
        let found = self.find_player(&p.name, p.server, p.pc_id)?;
        if let Some(id) = found {
            if let Some(&(ts, kind)) = self.last_write.get(&id) {
                if e.ts_ms - ts < ANTI_SPAM_MS && e.ts_ms >= ts && e.kind.priority() <= kind.priority() {
                    return Ok(None);
                }
            }
        }
        let id = match found {
            Some(id) => {
                // Les valeurs nulles (tueur jamais vu) n'écrasent pas ce qu'on sait.
                self.conn.execute(
                    "UPDATE players SET
                        pc_id = CASE WHEN ?2 <> 0 THEN ?2 ELSE pc_id END,
                        level = CASE WHEN ?3 <> 0 THEN ?3 ELSE level END,
                        gs = CASE WHEN ?4 <> 0 THEN ?4 ELSE gs END,
                        cp = CASE WHEN ?5 <> 0 THEN ?5 ELSE cp END,
                        guild = CASE WHEN ?6 <> '' OR ?2 <> 0 THEN ?6 ELSE guild END,
                        abyss_rank = CASE WHEN ?7 <> 0 THEN ?7 ELSE abyss_rank END,
                        last_seen = MAX(last_seen, ?8),
                        last_kind = ?9
                     WHERE id = ?1",
                    params![id, p.pc_id, p.level, p.gs, p.cp, p.guild, p.abyss_rank, e.ts_ms, e.kind.as_str()],
                )?;
                id
            }
            None => {
                self.conn.execute(
                    "INSERT INTO players (name, server_id, pc_id, level, gs, cp, guild, abyss_rank, first_seen, last_seen, last_kind)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10)",
                    params![p.name, p.server, p.pc_id, p.level, p.gs, p.cp, p.guild, p.abyss_rank, e.ts_ms, e.kind.as_str()],
                )?;
                self.conn.last_insert_rowid()
            }
        };
        self.conn.execute("INSERT INTO encounters (player_id, kind, ts) VALUES (?1, ?2, ?3)", params![id, e.kind.as_str(), e.ts_ms])?;
        self.last_write.insert(id, (e.ts_ms, e.kind));
        Ok(Some(id))
    }

    /// La liste, du plus récent au plus ancien, épinglés compris.
    pub fn list(&self, now: i64, retention_ms: i64) -> rusqlite::Result<Vec<Row>> {
        let since = now - retention_ms;
        let mut counts: HashMap<i64, (u32, i64)> = HashMap::new();
        {
            let mut st = self.conn.prepare("SELECT player_id, ts FROM encounters WHERE ts >= ?1 ORDER BY player_id, ts")?;
            let rows = st.query_map(params![since], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
            for row in rows {
                let (pid, ts) = row?;
                let c = counts.entry(pid).or_insert((0, i64::MIN / 2));
                if ts - c.1 >= SAME_ENCOUNTER_MS {
                    c.0 += 1;
                }
                c.1 = ts;
            }
        }
        let mut st = self.conn.prepare(
            "SELECT id, name, server_id, pc_id, level, gs, cp, guild, abyss_rank, character_id, last_seen, last_kind, pinned
             FROM players WHERE pinned = 1 OR last_seen >= ?1 ORDER BY last_seen DESC",
        )?;
        let rows = st.query_map(params![since], |r| {
            let id: i64 = r.get(0)?;
            let pc_id: i32 = r.get(3)?;
            let kind: String = r.get(11)?;
            Ok(Row {
                id,
                name: r.get(1)?,
                server: r.get(2)?,
                pc_id,
                class: class_name(pc_id),
                level: r.get(4)?,
                gs: r.get(5)?,
                cp: r.get(6)?,
                guild: r.get(7)?,
                abyss_rank: r.get(8)?,
                character_id: r.get(9)?,
                last_seen_ms: r.get(10)?,
                last_kind: Kind::parse(&kind).unwrap_or(Kind::Cible),
                pinned: r.get::<_, i64>(12)? != 0,
                count: counts.get(&id).map(|c| c.0).unwrap_or(0),
            })
        })?;
        rows.collect()
    }

    /// Purge : plus vieux que la rétention, ou au-delà de `max_players`. Les épinglés restent.
    pub fn purge(&mut self, now: i64, retention_ms: i64, max_players: usize) -> rusqlite::Result<usize> {
        let mut n = self.conn.execute("DELETE FROM players WHERE pinned = 0 AND last_seen < ?1", params![now - retention_ms])?;
        n += self.conn.execute(
            "DELETE FROM players WHERE pinned = 0 AND id NOT IN (SELECT id FROM players WHERE pinned = 0 ORDER BY last_seen DESC LIMIT ?1)",
            params![max_players as i64],
        )?;
        self.conn.execute("DELETE FROM encounters WHERE ts < ?1 AND player_id IN (SELECT id FROM players WHERE pinned = 0)", params![now - retention_ms])?;
        self.last_write.retain(|_, (ts, _)| now - *ts < ANTI_SPAM_MS * 2);
        Ok(n)
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE players SET pinned = ?2 WHERE id = ?1", params![id, pinned as i64]).map(|_| ())
    }

    /// « Vider la liste » : tout sauf les épinglés.
    pub fn clear(&mut self) -> rusqlite::Result<usize> {
        self.last_write.clear();
        self.conn.execute("DELETE FROM players WHERE pinned = 0", [])
    }

    pub fn set_character_id(&self, id: i64, character_id: &str) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE players SET character_id = ?2 WHERE id = ?1", params![id, character_id]).map(|_| ())
    }

    pub fn row(&self, id: i64) -> rusqlite::Result<Option<Row>> {
        Ok(self.list(i64::MAX / 2, i64::MAX / 2)?.into_iter().find(|r| r.id == id))
    }

    pub fn cache_get(&self, url: &str, max_age_ms: i64, now: i64) -> rusqlite::Result<Option<(String, i64)>> {
        self.conn
            .query_row("SELECT body, fetched_at FROM api_cache WHERE url = ?1 AND fetched_at >= ?2", params![url, now - max_age_ms], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
    }

    pub fn cache_put(&self, url: &str, body: &str, now: i64) -> rusqlite::Result<()> {
        self.conn
            .execute("INSERT INTO api_cache (url, body, fetched_at) VALUES (?1, ?2, ?3) ON CONFLICT(url) DO UPDATE SET body = ?2, fetched_at = ?3", params![url, body, now])
            .map(|_| ())
    }

    /// Nettoie le cache API (réponses de plus d'un jour).
    pub fn cache_purge(&self, now: i64) -> rusqlite::Result<usize> {
        self.conn.execute("DELETE FROM api_cache WHERE fetched_at < ?1", params![now - 24 * 3600 * 1000])
    }

    pub fn setting(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0)).optional()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2", params![key, value])
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Player;

    fn player(name: &str, server: u16, pc_id: i32, cp: i64) -> Player {
        Player { name: name.into(), server, pc_id, class: class_name(pc_id), level: 45, gs: 2000, cp, guild: "Légion".into(), abyss_rank: 0, dbid: 1, last_seen_ms: 0 }
    }

    fn enc(ts: i64, kind: Kind, p: Player) -> Encounter {
        Encounter { ts_ms: ts, kind, player: p }
    }

    #[test]
    fn une_ligne_par_joueur_et_compteur() {
        let mut s = Store::open_in_memory().unwrap();
        let t0 = 1_000_000_000;
        s.record(&enc(t0, Kind::Cible, player("JoueurA", 1303, 13, 100))).unwrap();
        // Anti-spam : même joueur, même priorité, 2 s plus tard → écarté.
        assert_eq!(s.record(&enc(t0 + 2_000, Kind::Cible, player("JoueurA", 1303, 13, 100))).unwrap(), None);
        // Priorité plus haute : passe quand même.
        assert!(s.record(&enc(t0 + 3_000, Kind::Kill, player("joueura", 1303, 13, 120))).unwrap().is_some());
        // 15 min plus tard : 2e rencontre distincte.
        s.record(&enc(t0 + 15 * 60_000, Kind::Combat, player("JoueurA", 1303, 13, 130))).unwrap();
        s.record(&enc(t0 + 16 * 60_000, Kind::Cible, player("JoueurB", 1301, 21, 50))).unwrap();
        let rows = s.list(t0 + 16 * 60_000, DEFAULT_RETENTION_MS).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "JoueurB");
        assert_eq!(rows[1].count, 2);
        assert_eq!(rows[1].cp, 130);
        assert_eq!(rows[1].last_kind, Kind::Combat);
    }

    #[test]
    fn fusion_quand_le_serveur_est_appris() {
        let mut s = Store::open_in_memory().unwrap();
        s.record(&enc(10, Kind::Cible, player("JoueurC", 0, 5, 0))).unwrap();
        s.record(&enc(20_000, Kind::Mort, player("JoueurC", 1303, 5, 0))).unwrap();
        let rows = s.list(20_000, DEFAULT_RETENTION_MS).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].server, 1303);
        // Un tueur connu seulement par son pseudo n'efface pas la classe ni le CP.
        assert_eq!(rows[0].pc_id, 5);
    }

    #[test]
    fn purge_et_epingles() {
        let mut s = Store::open_in_memory().unwrap();
        let t0 = 10_000_000;
        let a = s.record(&enc(t0, Kind::Cible, player("Vieux", 1303, 1, 1))).unwrap().unwrap();
        s.record(&enc(t0, Kind::Cible, player("Epingle", 1303, 1, 1))).unwrap();
        let pinned = s.list(t0, DEFAULT_RETENTION_MS).unwrap().into_iter().find(|r| r.name == "Epingle").unwrap().id;
        s.set_pinned(pinned, true).unwrap();
        let later = t0 + DEFAULT_RETENTION_MS + 1;
        s.purge(later, DEFAULT_RETENTION_MS, DEFAULT_MAX_PLAYERS).unwrap();
        let rows = s.list(later, DEFAULT_RETENTION_MS).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, pinned);
        assert!(rows.iter().all(|r| r.id != a));
    }

    #[test]
    fn plafond_de_joueurs() {
        let mut s = Store::open_in_memory().unwrap();
        for i in 0..10 {
            s.record(&enc(1000 + i * 10, Kind::Cible, player(&format!("J{i}"), 1303, 1, 1))).unwrap();
        }
        s.purge(2000, DEFAULT_RETENTION_MS, 4).unwrap();
        let rows = s.list(2000, DEFAULT_RETENTION_MS).unwrap();
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["J9", "J8", "J7", "J6"]);
    }
}
