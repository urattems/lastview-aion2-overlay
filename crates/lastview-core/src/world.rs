//! Mémoire silencieuse : tous les joueurs vus autour de Gio, en RAM.
//!
//! Rien n'est affiché à ce stade. C'est l'annuaire qui permet de remplir
//! instantanément une ligne lors d'une rencontre. Les ids d'entité ne vivent
//! que le temps d'une session de jeu : ils servent à relier les paquets entre
//! eux, jamais d'identifiant durable.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::protocol::messages::{class_name, EntityId, PartyMember, PlayerAppear, SelfAppear};

/// Entrées non revues depuis cette durée : purgées.
pub const PURGE_AFTER_MS: i64 = 30 * 60 * 1000;
/// Au-delà, les ids de PNJ les plus anciens sont oubliés.
const MAX_NPCS: usize = 20_000;
/// Votes `4A 36` nécessaires pour deviner ton id sans `33 36`.
const SELF_VOTES: u32 = 3;

/// Ce qu'on sait d'un joueur.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Player {
    pub name: String,
    /// Id de serveur (1303 = Vaizel). 0 si inconnu.
    pub server: u16,
    pub pc_id: i32,
    pub class: Option<&'static str>,
    pub level: i32,
    pub gs: i32,
    pub cp: i64,
    pub guild: String,
    pub abyss_rank: i32,
    #[serde(skip)]
    pub dbid: i64,
    pub last_seen_ms: i64,
    /// Vrai si le joueur a été identifié par son apparition (`45 36`).
    #[serde(skip)]
    pub seen_appear: bool,
}

impl Player {
    pub fn from_appear(a: &PlayerAppear, ts: i64) -> Self {
        Self {
            name: a.name.clone(),
            server: a.server,
            pc_id: a.pc_id,
            class: class_name(a.pc_id),
            level: a.level,
            gs: a.gs,
            cp: a.cp,
            guild: a.guild.as_ref().map(|g| g.name.clone()).unwrap_or_default(),
            abyss_rank: a.abyss_rank,
            dbid: a.dbid,
            last_seen_ms: ts,
            seen_appear: true,
        }
    }

    pub fn from_member(m: &PartyMember, ts: i64) -> Self {
        Self {
            name: m.name.clone(),
            server: m.server,
            pc_id: m.pc_id,
            class: class_name(m.pc_id),
            level: m.level,
            gs: m.gs,
            cp: m.cp,
            guild: m.guild.clone(),
            abyss_rank: 0,
            dbid: m.dbid,
            last_seen_ms: ts,
            seen_appear: false,
        }
    }

    /// Joueur connu seulement par un paquet de mort : pseudo, serveur, légion.
    pub fn from_killer(name: &str, server: u16, guild: &str, ts: i64) -> Self {
        Self {
            name: name.to_string(),
            server,
            pc_id: 0,
            class: None,
            level: 0,
            gs: 0,
            cp: 0,
            guild: guild.to_string(),
            abyss_rank: 0,
            dbid: 0,
            last_seen_ms: ts,
            seen_appear: false,
        }
    }
}

/// Le joueur local.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LocalPlayer {
    pub key: Option<EntityId>,
    /// Vrai si l'id vient de `33 36` ; faux s'il est deviné (`4A 36`).
    pub confirmed: bool,
    pub name: String,
    pub server: u16,
    pub level: i32,
    pub gs: i32,
    /// CP lu dans les paquets (`33 36` en entier, ou `56 36`). 0 si inconnu.
    pub cp: i64,
}

#[derive(Default)]
pub struct World {
    players: HashMap<EntityId, Player>,
    npcs: HashSet<EntityId>,
    npc_order: Vec<EntityId>,
    pub local: LocalPlayer,
    self_votes: HashMap<EntityId, u32>,
    /// Membres actuels du groupe (ids d'entité), et dbid → id.
    party: HashSet<EntityId>,
    party_dbid: HashMap<i64, EntityId>,
    /// dbid → id, appris par les apparitions `45 36`.
    dbid_key: HashMap<i64, EntityId>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    /// Nom et serveur du perso de Gio (paramètres), en repli.
    pub fn set_local_hint(&mut self, name: &str, server: u16) {
        if self.local.name.is_empty() {
            self.local.name = name.to_string();
            self.local.server = server;
        }
    }

    pub fn player(&self, key: EntityId) -> Option<&Player> {
        self.players.get(&key)
    }

    pub fn players(&self) -> impl Iterator<Item = (&EntityId, &Player)> {
        self.players.iter()
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn is_npc(&self, key: EntityId) -> bool {
        self.npcs.contains(&key)
    }

    pub fn is_self(&self, key: EntityId) -> bool {
        key != 0 && self.local.key == Some(key)
    }

    /// Vrai si ce joueur est Gio (même pseudo et serveur que le perso local).
    pub fn is_self_player(&self, p: &Player) -> bool {
        !self.local.name.is_empty() && p.name.eq_ignore_ascii_case(&self.local.name) && (p.server == 0 || self.local.server == 0 || p.server == self.local.server)
    }

    pub fn on_player_appear(&mut self, a: &PlayerAppear, ts: i64) {
        self.forget_npc(a.key);
        if a.dbid != 0 {
            self.dbid_key.insert(a.dbid, a.key);
        }
        self.players.insert(a.key, Player::from_appear(a, ts));
    }

    pub fn on_npc_appear(&mut self, key: EntityId) {
        self.players.remove(&key);
        if self.npcs.insert(key) {
            self.npc_order.push(key);
            if self.npc_order.len() > MAX_NPCS {
                let old: Vec<_> = self.npc_order.drain(..MAX_NPCS / 2).collect();
                for k in old {
                    self.npcs.remove(&k);
                }
            }
        }
    }

    fn forget_npc(&mut self, key: EntityId) {
        self.npcs.remove(&key);
    }

    pub fn on_self_appear(&mut self, s: &SelfAppear) {
        if self.local.key != Some(s.key) || !self.local.confirmed {
            log::info!("joueur local confirmé par 33 36 : id {}, serveur {}, niveau {}, GS {}", s.key, s.server, s.level, s.gs);
        }
        self.players.remove(&s.key);
        let cp = if s.cp > 0 { s.cp } else { self.local.cp };
        self.local = LocalPlayer { key: Some(s.key), confirmed: true, name: s.name.clone(), server: s.server, level: s.level, gs: s.gs, cp };
        self.self_votes.clear();
    }

    /// `56 36` : ton CP actuel.
    pub fn on_self_cp(&mut self, cp: i64) {
        self.local.cp = cp;
    }

    /// `4A 36` : l'entité est presque toujours Gio. Sert tant que `33 36` manque.
    pub fn on_self_stats(&mut self, key: EntityId) {
        if self.local.confirmed || self.players.contains_key(&key) || self.npcs.contains(&key) {
            return;
        }
        let v = self.self_votes.entry(key).or_default();
        *v += 1;
        if *v >= SELF_VOTES && self.local.key != Some(key) {
            let best = self.self_votes.iter().max_by_key(|(_, n)| **n).map(|(k, _)| *k);
            if best == Some(key) {
                log::info!("id du joueur local deviné (non confirmé) : {key}");
                self.local.key = Some(key);
            }
        }
    }

    pub fn on_level(&mut self, key: EntityId, level: i32) {
        if let Some(p) = self.players.get_mut(&key) {
            p.level = level;
        }
    }

    pub fn on_guild(&mut self, key: EntityId, guild: &str) {
        if let Some(p) = self.players.get_mut(&key) {
            p.guild = guild.to_string();
        }
    }

    pub fn touch(&mut self, key: EntityId, ts: i64) {
        if let Some(p) = self.players.get_mut(&key) {
            p.last_seen_ms = ts;
        }
    }

    /// Membres de groupe reçus ; renvoie les ids des nouveaux membres.
    pub fn on_party_members(&mut self, full: bool, members: &[PartyMember], ts: i64) -> Vec<EntityId> {
        if full {
            let keep: HashSet<_> = members.iter().map(|m| m.key).collect();
            self.party.retain(|k| keep.contains(k));
        }
        let mut new = Vec::new();
        for m in members {
            let mut p = Player::from_member(m, ts);
            if let Some(old) = self.players.get(&m.key) {
                p.seen_appear = old.seen_appear && old.name == p.name;
            }
            if self.is_self(m.key) || self.is_self_player(&p) {
                // Le membre « moi » donne mon id s'il n'était pas encore confirmé.
                if !self.local.confirmed {
                    self.local.key = Some(m.key);
                    self.local.name = m.name.clone();
                    self.local.server = m.server;
                }
                continue;
            }
            self.forget_npc(m.key);
            self.party_dbid.insert(m.dbid, m.key);
            self.players.insert(m.key, p);
            if self.party.insert(m.key) {
                new.push(m.key);
            }
        }
        new
    }

    /// Mise à jour d'un membre (clé : dbid). Renvoie (id, nouveau membre ?).
    /// Un dbid inconnu du groupe mais vu dans une apparition est un membre
    /// d'un groupe formé avant le lancement de LastView.
    pub fn on_party_update(&mut self, dbid: i64, level: i32, gs: i32, guild: &str, cp: i64) -> Option<(EntityId, bool)> {
        let (key, new) = match self.party_dbid.get(&dbid) {
            Some(k) => (*k, false),
            None => {
                let k = *self.dbid_key.get(&dbid)?;
                if !self.players.get(&k).is_some_and(|p| p.dbid == dbid) || self.is_self(k) {
                    return None;
                }
                self.party_dbid.insert(dbid, k);
                (k, self.party.insert(k))
            }
        };
        let p = self.players.get_mut(&key)?;
        p.level = level;
        p.gs = gs;
        p.guild = guild.to_string();
        p.cp = cp;
        Some((key, new))
    }

    pub fn on_party_left(&mut self) {
        self.party.clear();
        self.party_dbid.clear();
    }

    /// Oublie les joueurs non revus depuis [`PURGE_AFTER_MS`].
    pub fn purge(&mut self, now: i64) {
        self.players.retain(|_, p| now - p.last_seen_ms < PURGE_AFTER_MS);
    }
}
