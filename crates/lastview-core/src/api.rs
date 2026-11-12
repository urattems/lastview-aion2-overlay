//! Client de l'API NCSoft (lecture seule, région EU, `lang=fr-FR`). Voir `docs/api.md`.
//!
//! Règles (section 4.10) : aucun appel sans action de l'utilisateur ; une
//! seule requête à la fois et au moins 1 s entre deux ; pause de 10 min sur
//! 403 ou 429 ; pas de nouvel essai pendant 1 h sur 404 ; cache de 10 min.
//! Parsing défensif : tout champ peut manquer.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::Serialize;
use serde_json::Value;

use crate::capture::now_ms;
use crate::store::Store;

pub const CACHE_MS: i64 = 10 * 60 * 1000;
/// « Actualiser » est permis si la donnée a plus de 30 s.
pub const REFRESH_MIN_AGE_MS: i64 = 30 * 1000;
const MIN_INTERVAL: Duration = Duration::from_secs(1);
const PAUSE_ON_LIMIT: Duration = Duration::from_secs(10 * 60);
const NOT_FOUND_RETRY: Duration = Duration::from_secs(3600);
const SEARCH_URL: &str = "https://api-search.plaync.com/aion2global/search/v2/character";
const SITE: &str = "https://aion2.plaync.com";

/// Caractères laissés tels quels dans un paramètre d'URL.
const PARAM: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

fn enc(s: &str) -> String {
    utf8_percent_encode(s, PARAM).to_string()
}

/// `characterId` : la recherche le renvoie encodé (`%3D`), `info` brut (`=`). On garde la forme brute.
pub fn normalize_character_id(id: &str) -> String {
    percent_decode_str(id).decode_utf8_lossy().into_owned()
}

/// Retire les balises HTML (`<strong>` de la recherche).
pub fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "erreur", rename_all = "snake_case")]
pub enum ApiError {
    /// 404 : perso introuvable.
    Introuvable,
    /// 403/429, ou pause en cours : reprise dans `secondes`.
    Limite { secondes: u64 },
    Reseau { message: String },
    Http { code: u16 },
    Format { message: String },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Introuvable => write!(f, "Perso introuvable"),
            ApiError::Limite { secondes } => write!(f, "NCSoft limite les requêtes, reprise dans {} min", secondes.div_ceil(60)),
            ApiError::Reseau { message } => write!(f, "Réseau : {message}"),
            ApiError::Http { code } => write!(f, "Erreur HTTP {code}"),
            ApiError::Format { message } => write!(f, "Réponse inattendue : {message}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// Un résultat de recherche, nom exact.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchHit {
    pub character_id: String,
    pub name: String,
    pub server_id: u16,
    pub server_name: String,
    pub level: i64,
    pub pc_id: i64,
    pub race: i64,
    pub profile_image_url: String,
}

/// Résumé lisible du profil (`info`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    pub name: String,
    pub class: String,
    pub level: i64,
    pub server_id: i64,
    pub server: String,
    pub race: String,
    pub cp: i64,
    pub gs: i64,
    pub title: String,
    pub portrait: String,
}

#[derive(Default)]
struct Queue {
    last: Option<Instant>,
    paused_until: Option<Instant>,
    not_found: HashMap<String, Instant>,
}

pub struct ApiClient {
    agent: ureq::Agent,
    queue: Mutex<Queue>,
    cache: Option<Mutex<Store>>,
}

fn s(v: &Value, path: &[&str]) -> String {
    let mut cur = v;
    for p in path {
        cur = match cur.get(p) {
            Some(x) => x,
            None => return String::new(),
        };
    }
    match cur {
        Value::String(x) => x.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn n(v: &Value, path: &[&str]) -> i64 {
    let mut cur = v;
    for p in path {
        cur = match cur.get(p) {
            Some(x) => x,
            None => return 0,
        };
    }
    cur.as_i64().or_else(|| cur.as_f64().map(|f| f as i64)).or_else(|| cur.as_str().and_then(|x| x.replace([',', ' '], "").parse().ok())).unwrap_or(0)
}

impl ApiClient {
    /// `cache_db` : base SQLite pour le cache des réponses (la même que la liste).
    pub fn new(cache_db: Option<&Path>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("LastView/", env!("CARGO_PKG_VERSION")))
            .build();
        let cache = cache_db.and_then(|p| Store::open(p).ok()).map(Mutex::new);
        Self { agent, queue: Mutex::new(Queue::default()), cache }
    }

    /// Pause en cours (403/429), en secondes.
    pub fn paused_for(&self) -> Option<u64> {
        let q = self.queue.lock().unwrap();
        q.paused_until.and_then(|t| t.checked_duration_since(Instant::now())).map(|d| d.as_secs().max(1))
    }

    /// GET JSON, avec cache. `max_age_ms` : âge maximal accepté depuis le cache.
    pub fn get_json(&self, url: &str, max_age_ms: i64) -> Result<Value, ApiError> {
        if let Some(c) = &self.cache {
            if let Ok(Some((body, _))) = c.lock().unwrap().cache_get(url, max_age_ms, now_ms()) {
                if let Ok(v) = serde_json::from_str(&body) {
                    return Ok(v);
                }
            }
        }
        // La file : on garde le verrou pendant la requête, donc une seule à la fois.
        let mut q = self.queue.lock().unwrap();
        if let Some(t) = q.paused_until {
            if let Some(left) = t.checked_duration_since(Instant::now()) {
                return Err(ApiError::Limite { secondes: left.as_secs().max(1) });
            }
            q.paused_until = None;
        }
        if let Some(t) = q.not_found.get(url) {
            if t.elapsed() < NOT_FOUND_RETRY {
                return Err(ApiError::Introuvable);
            }
        }
        if let Some(last) = q.last {
            let e = last.elapsed();
            if e < MIN_INTERVAL {
                std::thread::sleep(MIN_INTERVAL - e);
            }
        }
        log::debug!("API GET {url}");
        let res = self.agent.get(url).set("Accept", "application/json").call();
        q.last = Some(Instant::now());
        let body = match res {
            Ok(r) => r.into_string().map_err(|e| ApiError::Reseau { message: e.to_string() })?,
            Err(ureq::Error::Status(code, _)) => {
                return Err(match code {
                    403 | 429 => {
                        q.paused_until = Some(Instant::now() + PAUSE_ON_LIMIT);
                        log::warn!("API : {code}, pause de 10 min");
                        ApiError::Limite { secondes: PAUSE_ON_LIMIT.as_secs() }
                    }
                    404 => {
                        q.not_found.insert(url.to_string(), Instant::now());
                        ApiError::Introuvable
                    }
                    c => ApiError::Http { code: c },
                });
            }
            Err(e) => return Err(ApiError::Reseau { message: e.to_string() }),
        };
        drop(q);
        let v: Value = serde_json::from_str(&body).map_err(|e| ApiError::Format { message: e.to_string() })?;
        if let Some(c) = &self.cache {
            let _ = c.lock().unwrap().cache_put(url, &body, now_ms());
        }
        Ok(v)
    }

    /// Recherche par pseudo, filtrée sur le nom exact (et le serveur si connu).
    pub fn search(&self, name: &str, server: Option<u16>) -> Result<Vec<SearchHit>, ApiError> {
        let url = format!(
            "{SEARCH_URL}?keyword={}&page=1&size=100&localeInfo=en-US&region=eu&serverId={}",
            enc(name),
            server.map(|s| s.to_string()).unwrap_or_default()
        );
        let v = self.get_json(&url, CACHE_MS)?;
        let list = v.get("list").and_then(|l| l.as_array()).ok_or_else(|| ApiError::Format { message: "pas de champ list".into() })?;
        let hits: Vec<SearchHit> = list
            .iter()
            .map(|h| SearchHit {
                character_id: normalize_character_id(&s(h, &["characterId"])),
                name: strip_tags(&s(h, &["name"])),
                server_id: n(h, &["serverId"]) as u16,
                server_name: s(h, &["serverName"]),
                level: n(h, &["level"]),
                pc_id: n(h, &["pcId"]),
                race: n(h, &["race"]),
                profile_image_url: s(h, &["profileImageUrl"]),
            })
            .filter(|h| h.name.eq_ignore_ascii_case(name) && !h.character_id.is_empty())
            .filter(|h| server.map_or(true, |sv| h.server_id == sv))
            .collect();
        Ok(hits)
    }

    /// Serveur inconnu et plusieurs persos du même nom : les plus proches
    /// d'abord (même classe, puis niveau le plus proche). S'il en reste
    /// plusieurs, la fiche affiche un sélecteur. Le CP départagera une fois
    /// les profils chargés.
    pub fn rank_homonyms(mut hits: Vec<SearchHit>, pc_id: i32, level: i32) -> Vec<SearchHit> {
        hits.sort_by_key(|h| ((pc_id != 0 && h.pc_id != pc_id as i64) as u8, (h.level - level as i64).abs()));
        hits
    }

    fn char_params(character_id: &str, server_id: u16) -> String {
        format!("characterId={}&serverId={server_id}&region=eu", enc(&normalize_character_id(character_id)))
    }

    pub fn info(&self, character_id: &str, server_id: u16, max_age_ms: i64) -> Result<Value, ApiError> {
        self.get_json(&format!("{SITE}/api/character/info?lang=fr-FR&{}", Self::char_params(character_id, server_id)), max_age_ms)
    }

    pub fn equipment(&self, character_id: &str, server_id: u16, max_age_ms: i64) -> Result<Value, ApiError> {
        self.get_json(&format!("{SITE}/api/character/equipment?lang=fr-FR&{}", Self::char_params(character_id, server_id)), max_age_ms)
    }

    /// Détail d'une pièce : `enchant_level` = enchant + dépassement.
    pub fn item(&self, item_id: i64, enchant_level: i64, slot_pos: i64, character_id: &str, server_id: u16) -> Result<Value, ApiError> {
        self.get_json(
            &format!(
                "{SITE}/api/character/equipment/item?id={item_id}&enchantLevel={enchant_level}&{}&slotPos={slot_pos}&lang=fr-FR",
                Self::char_params(character_id, server_id)
            ),
            CACHE_MS,
        )
    }

    pub fn daevanion(&self, character_id: &str, server_id: u16, board_id: i64) -> Result<Value, ApiError> {
        self.get_json(&format!("{SITE}/api/character/daevanion/detail?lang=fr-FR&{}&boardId={board_id}", Self::char_params(character_id, server_id)), CACHE_MS)
    }

    pub fn servers(&self) -> Result<Value, ApiError> {
        self.get_json(&format!("{SITE}/en-us/api/gameinfo/servers?lang=en-US&region=eu"), 24 * 3600 * 1000)
    }

    /// Résumé du profil à partir de la réponse `info`.
    pub fn summary(info: &Value) -> Summary {
        let gs = info
            .pointer("/stat/statList")
            .and_then(|l| l.as_array())
            .and_then(|l| l.iter().find(|x| s(x, &["type"]) == "ItemLevel"))
            .map(|x| n(x, &["value"]))
            .unwrap_or(0);
        Summary {
            name: s(info, &["profile", "characterName"]),
            class: s(info, &["profile", "className"]),
            level: n(info, &["profile", "characterLevel"]),
            server_id: n(info, &["profile", "serverId"]),
            server: s(info, &["profile", "serverName"]),
            race: s(info, &["profile", "raceName"]),
            cp: n(info, &["profile", "combatPower"]),
            gs,
            title: s(info, &["profile", "titleName"]),
            portrait: s(info, &["profile", "profileImage"]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiants_et_balises() {
        assert_eq!(normalize_character_id("abc%3D"), "abc=");
        assert_eq!(normalize_character_id("abc="), "abc=");
        assert_eq!(enc("abc="), "abc%3D");
        assert_eq!(strip_tags("<strong>Ura</strong>ttems"), "Urattems");
    }

    #[test]
    fn homonymes() {
        let hit = |srv: u16, pc: i64, lvl: i64| SearchHit {
            character_id: format!("{srv}"),
            name: "Kira".into(),
            server_id: srv,
            server_name: String::new(),
            level: lvl,
            pc_id: pc,
            race: 1,
            profile_image_url: String::new(),
        };
        let r = ApiClient::rank_homonyms(vec![hit(1, 5, 45), hit(2, 34, 30), hit(3, 34, 44)], 34, 45);
        assert_eq!(r.iter().map(|h| h.server_id).collect::<Vec<_>>(), vec![3, 2, 1]);
    }

    #[test]
    fn resume_defensif() {
        let v: Value = serde_json::json!({
            "profile": {"characterName": "Perso", "className": "Aède", "characterLevel": 45, "combatPower": "12,345", "serverName": "Vaizel"},
            "stat": {"statList": [{"type": "STR", "value": 10}, {"type": "ItemLevel", "value": 2100}]}
        });
        let r = ApiClient::summary(&v);
        assert_eq!((r.level, r.cp, r.gs), (45, 12345, 2100));
        assert_eq!(r.title, "");
        let vide = ApiClient::summary(&serde_json::json!({}));
        assert_eq!(vide.gs, 0);
    }
}
