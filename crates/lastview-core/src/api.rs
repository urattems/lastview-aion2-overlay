//! Client de l'API NCSoft (lecture seule, région EU, `lang=fr-FR`). Voir `docs/api.md`.
//!
//! Règles (section 4.10, précisées par le chef de suivi le 12/11/2026) :
//! - aucun appel sans action de l'utilisateur (sauf la référence « Mon perso ») ;
//! - **une opération à la fois** : ouvrir une fiche, une infobulle, un plateau
//!   Daevanion, une recherche. L'ouverture d'une fiche envoie profil et
//!   équipement **ensemble** (2 requêtes simultanées au plus), sans délai ;
//! - au moins 1 s entre deux opérations qui passent par le réseau ;
//! - pause de 10 min sur 403 ou 429 ; pas de nouvel essai pendant 1 h sur 404 ;
//! - cache de 10 min ; parsing défensif : tout champ peut manquer.

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
pub const MIN_INTERVAL: Duration = Duration::from_secs(1);
pub const PAUSE_ON_LIMIT: Duration = Duration::from_secs(10 * 60);
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

/// Une fiche : profil et équipement, demandés ensemble.
#[derive(Debug)]
pub struct Fiche {
    pub info: Result<Value, ApiError>,
    pub equipment: Result<Value, ApiError>,
}

/// Résultat d'un clic sur « + ».
#[derive(Debug)]
pub struct Ouverture {
    /// Résultats de la recherche (vide si le `characterId` était connu).
    pub candidats: Vec<SearchHit>,
    /// Perso affiché : (`characterId`, serveur).
    pub choisi: Option<(String, u16)>,
    pub fiche: Option<Fiche>,
    pub erreur: Option<ApiError>,
    /// Le `characterId` en cache a renvoyé 404 : à effacer de la liste (le
    /// perso a pu être recréé ou transféré). Une nouvelle recherche a été faite.
    pub id_en_cache_invalide: bool,
}

/// Accès HTTP, remplaçable dans les tests. Renvoie (code, corps).
pub trait Transport: Send + Sync {
    fn get(&self, url: &str) -> Result<(u16, String), String>;
}

struct Ureq(ureq::Agent);

impl Transport for Ureq {
    fn get(&self, url: &str) -> Result<(u16, String), String> {
        match self.0.get(url).set("Accept", "application/json").call() {
            Ok(r) => {
                let code = r.status();
                r.into_string().map(|b| (code, b)).map_err(|e| e.to_string())
            }
            Err(ureq::Error::Status(code, r)) => Ok((code, r.into_string().unwrap_or_default())),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[derive(Default)]
struct Limits {
    paused_until: Option<Instant>,
    not_found: HashMap<String, Instant>,
}

pub struct ApiClient {
    transport: Box<dyn Transport>,
    /// Une opération à la fois.
    op_lock: Mutex<()>,
    /// Heure à partir de laquelle la prochaine opération réseau peut partir.
    next_allowed: Mutex<Option<Instant>>,
    limits: Mutex<Limits>,
    cache: Option<Mutex<Store>>,
    min_interval: Duration,
    pause: Duration,
    /// Durée du cache des fiches et recherches (paramètres, 10 min par défaut).
    cache_ms: std::sync::atomic::AtomicI64,
}

/// Contexte d'une opération : l'attente d'espacement n'a lieu qu'une fois,
/// et seulement si l'opération passe par le réseau.
pub struct Op<'a> {
    client: &'a ApiClient,
    spaced: Mutex<bool>,
}

impl Op<'_> {
    fn ensure_spaced(&self) {
        let mut s = self.spaced.lock().unwrap();
        if !*s {
            let next = *self.client.next_allowed.lock().unwrap();
            if let Some(d) = next.and_then(|t| t.checked_duration_since(Instant::now())) {
                std::thread::sleep(d);
            }
            *s = true;
        }
    }
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

/// Classement des suggestions de l'API (règle du chef de suivi, 13/11/2026) :
/// niveau inférieur à 10 toujours en bas ; puis nom exact, même serveur que
/// Gio, même faction, niveau décroissant, et enfin l'ordre de l'API.
pub fn classer_suggestions(mut hits: Vec<SearchHit>, texte: &str, mon_serveur: Option<u16>, ma_race: Option<i64>, max: usize) -> Vec<SearchHit> {
    let t = texte.trim().to_lowercase();
    // Tri stable : l'ordre de l'API départage les égalités.
    hits.sort_by_key(|h| {
        (
            h.level < 10,
            h.name.to_lowercase() != t,
            mon_serveur.is_some_and(|s| h.server_id != s),
            ma_race.is_some_and(|r| h.race != r),
            std::cmp::Reverse(h.level),
        )
    });
    hits.truncate(max);
    hits
}

/// Un résultat de recherche de l'API (nom débarrassé des balises `<strong>`).
fn hit(h: &Value) -> SearchHit {
    SearchHit {
        character_id: normalize_character_id(&s(h, &["characterId"])),
        name: strip_tags(&s(h, &["name"])),
        server_id: n(h, &["serverId"]) as u16,
        server_name: s(h, &["serverName"]),
        level: n(h, &["level"]),
        pc_id: n(h, &["pcId"]),
        race: n(h, &["race"]),
        profile_image_url: s(h, &["profileImageUrl"]),
    }
}

impl ApiClient {
    /// `cache_db` : base SQLite pour le cache des réponses (la même que la liste).
    pub fn new(cache_db: Option<&Path>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("LastView/", env!("CARGO_PKG_VERSION")))
            .build();
        let cache = cache_db.and_then(|p| Store::open(p).ok());
        Self::with_transport(Box::new(Ureq(agent)), cache, MIN_INTERVAL, PAUSE_ON_LIMIT)
    }

    /// Client avec un transport et des délais choisis (tests).
    pub fn with_transport(transport: Box<dyn Transport>, cache: Option<Store>, min_interval: Duration, pause: Duration) -> Self {
        Self {
            transport,
            op_lock: Mutex::new(()),
            next_allowed: Mutex::new(None),
            limits: Mutex::new(Limits::default()),
            cache: cache.map(Mutex::new),
            min_interval,
            pause,
            cache_ms: std::sync::atomic::AtomicI64::new(CACHE_MS),
        }
    }

    /// Durée du cache API (paramètres), de 1 à 60 min.
    pub fn set_cache_minutes(&self, min: u32) {
        self.cache_ms.store(i64::from(min.clamp(1, 60)) * 60_000, std::sync::atomic::Ordering::Relaxed);
    }

    fn cache_ms(&self) -> i64 {
        self.cache_ms.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Pause en cours (403/429), en secondes.
    pub fn paused_for(&self) -> Option<u64> {
        let l = self.limits.lock().unwrap();
        l.paused_until.and_then(|t| t.checked_duration_since(Instant::now())).map(|d| d.as_secs().max(1))
    }

    /// Exécute une opération : exclusive, espacée de la précédente si elle passe par le réseau.
    pub fn op<T>(&self, f: impl FnOnce(&Op<'_>) -> T) -> T {
        let _g = self.op_lock.lock().unwrap();
        let op = Op { client: self, spaced: Mutex::new(false) };
        let r = f(&op);
        if *op.spaced.lock().unwrap() {
            *self.next_allowed.lock().unwrap() = Some(Instant::now() + self.min_interval);
        }
        r
    }

    fn limited(&self) -> Option<ApiError> {
        let mut l = self.limits.lock().unwrap();
        if let Some(t) = l.paused_until {
            if let Some(left) = t.checked_duration_since(Instant::now()) {
                return Some(ApiError::Limite { secondes: left.as_secs().max(1) });
            }
            l.paused_until = None;
        }
        None
    }

    /// GET JSON dans une opération, avec cache. `max_age_ms` : âge maximal accepté depuis le cache.
    pub fn fetch(&self, op: &Op<'_>, url: &str, max_age_ms: i64) -> Result<Value, ApiError> {
        if let Some(c) = &self.cache {
            if let Ok(Some((body, _))) = c.lock().unwrap().cache_get(url, max_age_ms, now_ms()) {
                if let Ok(v) = serde_json::from_str(&body) {
                    return Ok(v);
                }
            }
        }
        if let Some(e) = self.limited() {
            return Err(e);
        }
        if self.limits.lock().unwrap().not_found.get(url).is_some_and(|t| t.elapsed() < NOT_FOUND_RETRY) {
            return Err(ApiError::Introuvable);
        }
        op.ensure_spaced();
        // L'autre requête de la même opération a pu déclencher une pause entre-temps.
        if let Some(e) = self.limited() {
            return Err(e);
        }
        let t = Instant::now();
        let res = self.transport.get(url);
        log::debug!("API GET {} ms : {url}", t.elapsed().as_millis());
        let (code, body) = res.map_err(|message| ApiError::Reseau { message })?;
        match code {
            200..=299 => {}
            403 | 429 => {
                self.limits.lock().unwrap().paused_until = Some(Instant::now() + self.pause);
                log::warn!("API : {code}, pause de {} min", self.pause.as_secs() / 60);
                return Err(ApiError::Limite { secondes: self.pause.as_secs().max(1) });
            }
            404 => {
                self.limits.lock().unwrap().not_found.insert(url.to_string(), Instant::now());
                return Err(ApiError::Introuvable);
            }
            c => return Err(ApiError::Http { code: c }),
        }
        let v: Value = serde_json::from_str(&body).map_err(|e| ApiError::Format { message: e.to_string() })?;
        if let Some(c) = &self.cache {
            let _ = c.lock().unwrap().cache_put(url, &body, now_ms());
        }
        Ok(v)
    }

    pub fn search_url(name: &str, server: Option<u16>) -> String {
        format!(
            "{SEARCH_URL}?keyword={}&page=1&size=100&localeInfo=en-US&region=eu&serverId={}",
            enc(name),
            server.map(|s| s.to_string()).unwrap_or_default()
        )
    }

    /// Recherche par pseudo (une opération), filtrée sur le nom exact (et le serveur si connu).
    pub fn search(&self, name: &str, server: Option<u16>) -> Result<Vec<SearchHit>, ApiError> {
        let v = self.op(|o| self.fetch(o, &Self::search_url(name, server), self.cache_ms()))?;
        if v.get("list").and_then(|l| l.as_array()).is_none() {
            return Err(ApiError::Format { message: "pas de champ list".into() });
        }
        Ok(Self::parse_hits(&v, name, server))
    }

    /// Suggestions pendant la frappe (section 7) : une opération de la file,
    /// deux pages de 100 (toute la région EU, puis le serveur de Gio), classées
    /// par [`classer_suggestions`].
    pub fn search_partiel(&self, texte: &str, mon_serveur: Option<u16>, ma_race: Option<i64>, max: usize) -> Result<Vec<SearchHit>, ApiError> {
        let base = format!("{SEARCH_URL}?keyword={}&page=1&size=100&localeInfo=en-US&region=eu&serverId=", enc(texte.trim()));
        let pages = self.op(|o| {
            let mut pages = vec![self.fetch(o, &base, self.cache_ms())?];
            if let Some(s) = mon_serveur {
                // La page EU peut manquer des joueurs du serveur de Gio (436 résultats pour « Urat ») : erreur ici sans gravité.
                if let Ok(v) = self.fetch(o, &format!("{base}{s}"), self.cache_ms()) {
                    pages.push(v);
                }
            }
            Ok(pages)
        })?;
        let mut hits: Vec<SearchHit> = Vec::new();
        for v in &pages {
            let Some(list) = v.get("list").and_then(|l| l.as_array()) else {
                return Err(ApiError::Format { message: "pas de champ list".into() });
            };
            for h in list.iter().map(hit) {
                if !h.character_id.is_empty() && !h.name.is_empty() && !hits.iter().any(|x| x.character_id == h.character_id) {
                    hits.push(h);
                }
            }
        }
        Ok(classer_suggestions(hits, texte, mon_serveur, ma_race, max))
    }

    fn parse_hits(v: &Value, name: &str, server: Option<u16>) -> Vec<SearchHit> {
        let Some(list) = v.get("list").and_then(|l| l.as_array()) else { return Vec::new() };
        list.iter()
            .map(hit)
            .filter(|h| h.name.eq_ignore_ascii_case(name) && !h.character_id.is_empty())
            .filter(|h| server.map_or(true, |sv| h.server_id == sv))
            .collect()
    }

    /// Serveur inconnu et plusieurs persos du même nom : les plus proches
    /// d'abord (même classe, puis niveau le plus proche). S'il en reste
    /// plusieurs, la fiche affiche un sélecteur. Le CP départagera une fois
    /// les profils chargés.
    pub fn rank_homonyms(mut hits: Vec<SearchHit>, pc_id: i32, level: i32) -> Vec<SearchHit> {
        hits.sort_by_key(|h| ((pc_id != 0 && h.pc_id != pc_id as i64) as u8, (h.level - level as i64).abs()));
        hits
    }

    /// Date (ms) de la réponse en cache pour cette URL : « Données API il y a … ».
    pub fn fetched_at(&self, url: &str) -> Option<i64> {
        let c = self.cache.as_ref()?;
        c.lock().unwrap().cache_get(url, i64::MAX / 2, now_ms()).ok().flatten().map(|(_, t)| t)
    }

    fn char_params(character_id: &str, server_id: u16) -> String {
        format!("characterId={}&serverId={server_id}&region=eu", enc(&normalize_character_id(character_id)))
    }

    pub fn info_url(character_id: &str, server_id: u16) -> String {
        format!("{SITE}/api/character/info?lang=fr-FR&{}", Self::char_params(character_id, server_id))
    }

    pub fn equipment_url(character_id: &str, server_id: u16) -> String {
        format!("{SITE}/api/character/equipment?lang=fr-FR&{}", Self::char_params(character_id, server_id))
    }

    /// Ouverture d'une fiche : profil et équipement **ensemble**, une seule opération.
    /// `force` : « Actualiser » (cache accepté seulement s'il a moins de 30 s).
    pub fn fiche(&self, character_id: &str, server_id: u16, force: bool) -> Fiche {
        let age = if force { REFRESH_MIN_AGE_MS } else { self.cache_ms() };
        let (iu, eu) = (Self::info_url(character_id, server_id), Self::equipment_url(character_id, server_id));
        self.op(|o| {
            std::thread::scope(|sc| {
                let info = sc.spawn(|| self.fetch(o, &iu, age));
                let equipment = self.fetch(o, &eu, age);
                let info = info.join().unwrap_or_else(|_| Err(ApiError::Reseau { message: "fil interrompu".into() }));
                Fiche { info, equipment }
            })
        })
    }

    /// Clic sur « + » : **une seule opération**, recherche comprise si le
    /// `characterId` n'est pas encore connu. La recherche part d'abord, puis
    /// profil et équipement ensemble (jamais plus de 2 requêtes à la fois).
    /// `character_id` connu (cache de la liste) : pas de recherche.
    pub fn ouvrir(&self, name: &str, server: Option<u16>, pc_id: i32, level: i32, character_id: Option<(&str, u16)>, force: bool) -> Ouverture {
        let age = if force { REFRESH_MIN_AGE_MS } else { self.cache_ms() };
        self.op(|o| {
            let fiche_de = |cid: &str, srv: u16| {
                let (iu, eu) = (Self::info_url(cid, srv), Self::equipment_url(cid, srv));
                std::thread::scope(|sc| {
                    let info = sc.spawn(|| self.fetch(o, &iu, age));
                    let equipment = self.fetch(o, &eu, age);
                    let info = info.join().unwrap_or_else(|_| Err(ApiError::Reseau { message: "fil interrompu".into() }));
                    Fiche { info, equipment }
                })
            };
            let recherche = || -> Result<(Vec<SearchHit>, Option<(String, u16)>), ApiError> {
                let hits = Self::rank_homonyms(Self::parse_hits(&self.fetch(o, &Self::search_url(name, server), self.cache_ms())?, name, server), pc_id, level);
                // Serveur connu : un seul résultat possible. Sinon le plus proche ; la fiche proposera les autres.
                let first = hits.first().map(|h| (h.character_id.clone(), h.server_id));
                Ok((hits, first))
            };
            let mut id_en_cache_invalide = false;
            let (mut candidats, mut choisi, mut fiche) = (Vec::new(), None, None);
            if let Some((cid, srv)) = character_id {
                let f = fiche_de(&normalize_character_id(cid), srv);
                if matches!(f.info, Err(ApiError::Introuvable)) {
                    // characterId en cache périmé : on l'oublie et on refait UNE recherche.
                    id_en_cache_invalide = true;
                } else {
                    choisi = Some((normalize_character_id(cid), srv));
                    fiche = Some(f);
                }
            }
            if fiche.is_none() {
                match recherche() {
                    Ok((hits, first)) => {
                        candidats = hits;
                        choisi = first;
                    }
                    Err(e) => return Ouverture { candidats, choisi: None, fiche: None, erreur: Some(e), id_en_cache_invalide },
                }
                let Some((cid, srv)) = choisi.clone() else {
                    return Ouverture { candidats, choisi: None, fiche: None, erreur: Some(ApiError::Introuvable), id_en_cache_invalide };
                };
                fiche = Some(fiche_de(&cid, srv));
            }
            Ouverture { candidats, choisi, fiche, erreur: None, id_en_cache_invalide }
        })
    }

    /// Profil seul (référence « Mon perso »), une opération.
    pub fn info(&self, character_id: &str, server_id: u16, max_age_ms: i64) -> Result<Value, ApiError> {
        self.op(|o| self.fetch(o, &Self::info_url(character_id, server_id), max_age_ms))
    }

    /// Détail d'une pièce (infobulle) : `enchant_level` = enchant + dépassement.
    pub fn item(&self, item_id: i64, enchant_level: i64, slot_pos: i64, character_id: &str, server_id: u16) -> Result<Value, ApiError> {
        let url = Self::item_url(item_id, enchant_level, slot_pos, character_id, server_id);
        self.op(|o| self.fetch(o, &url, self.cache_ms()))
    }

    pub fn item_url(item_id: i64, enchant_level: i64, slot_pos: i64, character_id: &str, server_id: u16) -> String {
        format!(
            "{SITE}/api/character/equipment/item?id={item_id}&enchantLevel={enchant_level}&{}&slotPos={slot_pos}&lang=fr-FR",
            Self::char_params(character_id, server_id)
        )
    }

    pub fn daevanion(&self, character_id: &str, server_id: u16, board_id: i64) -> Result<Value, ApiError> {
        let url = format!("{SITE}/api/character/daevanion/detail?lang=fr-FR&{}&boardId={board_id}", Self::char_params(character_id, server_id));
        self.op(|o| self.fetch(o, &url, self.cache_ms()))
    }

    pub fn servers(&self) -> Result<Value, ApiError> {
        self.op(|o| self.fetch(o, &format!("{SITE}/en-us/api/gameinfo/servers?lang=en-US&region=eu"), 24 * 3600 * 1000))
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

/// Page du perso sur shugo.gg, au format de leur bouton « Partager » (relevé
/// le 12/11/2026) : `https://shugo.gg/character/global/eu-<serveur>/<Pseudo>`,
/// serveur en minuscules. Aucun serveur EU n'a d'espace ni d'accent (46 vérifiés),
/// mais on les retire quand même par prudence.
pub fn shugo_url(nom: &str, serveur: &str) -> String {
    let slug: String = serveur
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' => 'o',
            'û' | 'ü' | 'ù' | 'ú' => 'u',
            'ç' => 'c',
            ' ' | '_' => '-',
            c => c,
        })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    format!("https://shugo.gg/character/global/eu-{slug}/{}", enc(nom.trim()))
}

/// Clés connues des réponses (relevées le 12/11/2026). Toute autre clé est
/// signalée une fois dans le journal de diagnostic : l'API n'est pas
/// documentée et peut changer (section 4.10, parsing défensif).
const CLES_CONNUES: &[(&str, &[&str])] = &[
    ("info", &["profile", "stat", "title", "ranking", "daevanion"]),
    (
        "profil",
        &[
            "characterId", "characterLevel", "characterName", "className", "combatPower", "gender", "genderName", "pcId", "profileImage", "raceId", "raceName",
            "regionName", "serverId", "serverName", "titleGrade", "titleId", "titleName",
        ],
    ),
    ("equipement", &["equipment", "petwing", "skill"]),
    ("piece_liste", &["enchantLevel", "exceedLevel", "grade", "icon", "id", "name", "slotPos", "slotPosName"]),
    (
        "piece",
        &[
            "categoryName", "classNames", "costumes", "decomposable", "enchantLevel", "enchantable", "equipLevel", "favorite", "godStoneSlotCount", "godStoneStat", "grade",
            "gradeName", "icon", "id", "level", "levelValue", "magicStoneSlotCount", "magicStoneStat", "mainStats", "maxEnchantLevel", "maxExceedEnchantLevel", "name",
            "raceName", "set", "soulBindRate", "sources", "storable", "subSkillCountMax", "subSkills", "subStatCount", "subStatRandom", "subStats", "surpassable", "tradable",
            "tradablePersonal", "type",
        ],
    ),
];

static CHAMPS_SIGNALES: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);

/// Signale (une seule fois par champ) les clés inconnues d'un objet de réponse.
pub fn signaler_champs_inconnus(sorte: &str, v: &Value) {
    let Some(obj) = v.as_object() else { return };
    let Some((_, connues)) = CLES_CONNUES.iter().find(|(s, _)| *s == sorte) else { return };
    let mut vus = CHAMPS_SIGNALES.lock().unwrap();
    let vus = vus.get_or_insert_with(Default::default);
    for k in obj.keys() {
        if !connues.contains(&k.as_str()) && vus.insert(format!("{sorte}.{k}")) {
            log::info!("API : champ inconnu « {sorte}.{k} » (ignoré, à regarder)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn identifiants_et_balises() {
        assert_eq!(normalize_character_id("abc%3D"), "abc=");
        assert_eq!(normalize_character_id("abc="), "abc=");
        assert_eq!(enc("abc="), "abc%3D");
        assert_eq!(strip_tags("<strong>Ura</strong>ttems"), "Urattems");
    }

    #[test]
    fn adresse_shugo() {
        assert_eq!(shugo_url("Urattems", "Vaizel"), "https://shugo.gg/character/global/eu-vaizel/Urattems");
        assert_eq!(shugo_url("Abc", "Mont Kaisinel"), "https://shugo.gg/character/global/eu-mont-kaisinel/Abc");
        assert_eq!(shugo_url("Abc", "Éréshkigal"), "https://shugo.gg/character/global/eu-ereshkigal/Abc");
        // Pseudo non latin : encodé dans l'adresse.
        assert_eq!(shugo_url("火鸡", "Siel"), "https://shugo.gg/character/global/eu-siel/%E7%81%AB%E9%B8%A1");
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

    /// Faux serveur : note l'heure de début et de fin de chaque requête.
    struct Fake {
        delay: Duration,
        code: Box<dyn Fn(&str) -> u16 + Send + Sync>,
        calls: Arc<Mutex<Vec<(String, Instant, Instant)>>>,
    }

    impl Transport for Fake {
        fn get(&self, url: &str) -> Result<(u16, String), String> {
            let start = Instant::now();
            std::thread::sleep(self.delay);
            self.calls.lock().unwrap().push((url.to_string(), start, Instant::now()));
            let body = if url.contains("api-search") {
                r#"{"list":[{"characterId":"abc%3D","name":"<strong>Perso</strong>","serverId":1303,"level":45,"pcId":34}]}"#
            } else {
                r#"{"profile":{"characterName":"Perso"}}"#
            };
            Ok(((self.code)(url), body.into()))
        }
    }

    type Calls = Arc<Mutex<Vec<(String, Instant, Instant)>>>;

    fn client(delay_ms: u64, interval_ms: u64, code: impl Fn(&str) -> u16 + Send + Sync + 'static) -> (ApiClient, Calls) {
        let calls: Calls = Arc::default();
        let fake = Fake { delay: Duration::from_millis(delay_ms), code: Box::new(code), calls: calls.clone() };
        let c = ApiClient::with_transport(Box::new(fake), Some(Store::open_in_memory().unwrap()), Duration::from_millis(interval_ms), Duration::from_secs(600));
        (c, calls)
    }

    #[test]
    fn espacement_entre_operations() {
        let (c, calls) = client(20, 300, |_| 200);
        c.search("A", None).unwrap();
        c.search("B", None).unwrap();
        let v = calls.lock().unwrap();
        assert_eq!(v.len(), 2);
        let gap = v[1].1.duration_since(v[0].2);
        assert!(gap >= Duration::from_millis(290), "écart {gap:?}");
    }

    #[test]
    fn fiche_deux_requetes_simultanees() {
        let (c, calls) = client(200, 1000, |_| 200);
        let t = Instant::now();
        let f = c.fiche("abc=", 1303, false);
        assert!(f.info.is_ok() && f.equipment.is_ok());
        // Ensemble : bien moins que 2 × 200 ms + 1 s d'espacement.
        assert!(t.elapsed() < Duration::from_millis(380), "durée {:?}", t.elapsed());
        let v = calls.lock().unwrap();
        assert_eq!(v.len(), 2);
        let starts = if v[0].1 > v[1].1 { v[0].1 - v[1].1 } else { v[1].1 - v[0].1 };
        assert!(starts < Duration::from_millis(100), "départs à {starts:?} d'écart");
        drop(v);
        // Deuxième ouverture : depuis le cache, sans réseau ni attente.
        let t = Instant::now();
        let f = c.fiche("abc=", 1303, false);
        assert!(f.info.is_ok() && f.equipment.is_ok());
        assert!(t.elapsed() < Duration::from_millis(100));
        assert_eq!(calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn clic_plus_une_seule_operation() {
        let (c, calls) = client(100, 1000, |_| 200);
        let t = Instant::now();
        let o = c.ouvrir("Perso", Some(1303), 34, 45, None, false);
        assert!(o.erreur.is_none(), "{:?}", o.erreur);
        assert_eq!(o.choisi, Some(("abc=".to_string(), 1303)));
        let f = o.fiche.unwrap();
        assert!(f.info.is_ok() && f.equipment.is_ok());
        // Recherche puis profil + équipement ensemble, sans la seconde d'espacement : ~200 ms.
        assert!(t.elapsed() < Duration::from_millis(450), "durée {:?}", t.elapsed());
        {
            let v = calls.lock().unwrap();
            assert_eq!(v.len(), 3);
            assert!(v[0].0.contains("api-search"));
            assert!(v[1].1 >= v[0].2 && v[2].1 >= v[0].2, "la fiche part après la recherche");
        }
        // Ouverture suivante (autre perso, characterId connu) : attend l'espacement.
        let end_first = calls.lock().unwrap().iter().map(|c| c.2).max().unwrap();
        let o2 = c.ouvrir("Autre", None, 0, 0, Some(("xyz%3D", 1301)), false);
        assert!(o2.fiche.is_some());
        let v = calls.lock().unwrap();
        assert_eq!(v.len(), 5);
        let start_second = v[3].1.min(v[4].1);
        assert!(start_second.duration_since(end_first) >= Duration::from_millis(990));
    }

    #[test]
    fn id_en_cache_perime_une_seule_recherche() {
        // L'ancien id renvoie 404 ; la recherche donne le nouvel id (abc=).
        let (c, calls) = client(1, 1, |u| if u.contains("ancien") { 404 } else { 200 });
        let o = c.ouvrir("Perso", Some(1303), 34, 45, Some(("ancien%3D", 1303)), false);
        assert!(o.erreur.is_none(), "{:?}", o.erreur);
        assert!(o.id_en_cache_invalide);
        assert_eq!(o.choisi, Some(("abc=".to_string(), 1303)));
        let v = calls.lock().unwrap();
        assert_eq!(v.iter().filter(|c| c.0.contains("api-search")).count(), 1, "une seule recherche");
        assert_eq!(v.len(), 5, "2 (ancien id) + 1 recherche + 2 (nouvel id)");
    }

    #[test]
    fn pause_sur_429() {
        let (c, calls) = client(1, 1, |_| 429);
        assert!(matches!(c.search("A", None), Err(ApiError::Limite { secondes: 600 })));
        // Pendant la pause : aucune requête ne part.
        assert!(matches!(c.search("B", None), Err(ApiError::Limite { .. })));
        let f = c.fiche("abc=", 1303, false);
        assert!(matches!(f.info, Err(ApiError::Limite { .. })) && matches!(f.equipment, Err(ApiError::Limite { .. })));
        assert_eq!(calls.lock().unwrap().len(), 1);
        assert!(c.paused_for().unwrap() > 590);
    }

    #[test]
    fn introuvable_sur_404() {
        let (c, calls) = client(1, 1, |u| if u.contains("info") { 404 } else { 200 });
        assert_eq!(c.info("zzz=", 1303, CACHE_MS), Err(ApiError::Introuvable));
        // Pas de nouvel essai pendant 1 h pour cette URL ; les autres passent.
        assert_eq!(c.info("zzz=", 1303, CACHE_MS), Err(ApiError::Introuvable));
        assert!(c.search("A", None).is_ok());
        assert_eq!(calls.lock().unwrap().len(), 2);
    }

    fn h(nom: &str, serveur: u16, race: i64, niveau: i64) -> SearchHit {
        SearchHit { character_id: format!("id-{nom}-{serveur}"), name: nom.into(), server_id: serveur, server_name: String::new(), level: niveau, pc_id: 0, race, profile_image_url: String::new() }
    }

    /// Cas « Urat » (noms des autres joueurs inventés) : Urattems remonte en tête
    /// malgré l'ordre de l'API, les niveaux inférieurs à 10 finissent en bas.
    #[test]
    fn classement_des_suggestions_urat() {
        let api = vec![
            h("Urata", 1002, 2, 2),
            h("Uratb", 1004, 1, 45),
            h("Uratc", 1303, 2, 45),
            h("Urat", 1010, 2, 1),
            h("Uratd", 1006, 1, 30),
            h("Urattems", 1303, 1, 45),
            h("Urate", 1303, 1, 20),
            h("Uratf", 1008, 2, 45),
        ];
        let noms: Vec<String> = classer_suggestions(api.clone(), "Urat", Some(1303), Some(1), 10).into_iter().map(|x| x.name).collect();
        assert_eq!(noms, ["Urattems", "Urate", "Uratc", "Uratb", "Uratd", "Uratf", "Urat", "Urata"]);
        // Nom exact en tête (hors niveaux < 10), maximum respecté.
        let noms: Vec<String> = classer_suggestions(api, "uratf", Some(1303), Some(1), 3).into_iter().map(|x| x.name).collect();
        assert_eq!(noms, ["Uratf", "Urattems", "Urate"]);
    }
}
