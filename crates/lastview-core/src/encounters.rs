//! Détection des rencontres (section 4.6 du cahier) : le moteur lit les
//! paquets décodés, tient la mémoire silencieuse à jour et sort une
//! [`Encounter`] à chaque interaction réelle avec un autre joueur.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::capture::GamePacket;
use crate::protocol::messages::{EntityId, Message};
use crate::protocol::{decode, Decoded};
use crate::world::{Player, World};

/// Durée pendant laquelle une rencontre avec un id encore sans nom attend son pseudo.
pub const PENDING_TTL_MS: i64 = 10 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Mort,
    Kill,
    Combat,
    Cible,
    Groupe,
    Buff,
}

impl Kind {
    pub const ALL: [Kind; 6] = [Kind::Mort, Kind::Kill, Kind::Combat, Kind::Cible, Kind::Groupe, Kind::Buff];

    /// Priorité d'affichage si simultané : Mort > Kill > Combat > Ciblé > Groupe > Buff.
    pub fn priority(self) -> u8 {
        match self {
            Kind::Mort => 6,
            Kind::Kill => 5,
            Kind::Combat => 4,
            Kind::Cible => 3,
            Kind::Groupe => 2,
            Kind::Buff => 1,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Mort => "mort",
            Kind::Kill => "kill",
            Kind::Combat => "combat",
            Kind::Cible => "cible",
            Kind::Groupe => "groupe",
            Kind::Buff => "buff",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// Libellé affiché dans la bulle.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Mort => "Mort",
            Kind::Kill => "Kill",
            Kind::Combat => "Combat",
            Kind::Cible => "Ciblé",
            Kind::Groupe => "Groupe",
            Kind::Buff => "Buff",
        }
    }
}

/// Une rencontre : le joueur tel qu'on le connaît à cet instant.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Encounter {
    pub ts_ms: i64,
    pub kind: Kind,
    pub player: Player,
}

/// Compteurs pour le mode diagnostic.
#[derive(Debug, Default, Clone, Serialize)]
pub struct Stats {
    pub packets: u64,
    pub decoded: BTreeMap<String, u64>,
    pub unreadable: BTreeMap<String, u64>,
    pub encounters: u64,
}

struct Pending {
    key: EntityId,
    kind: Kind,
    ts: i64,
}

pub struct Engine {
    pub world: World,
    pub stats: Stats,
    enabled: HashMap<Kind, bool>,
    pending: Vec<Pending>,
    last_purge: i64,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

fn hex(op: [u8; 2]) -> String {
    format!("{:02X} {:02X}", op[0], op[1])
}

impl Engine {
    pub fn new() -> Self {
        Self { world: World::new(), stats: Stats::default(), enabled: Kind::ALL.iter().map(|k| (*k, true)).collect(), pending: Vec::new(), last_purge: 0 }
    }

    /// Active ou coupe une source (paramètres).
    pub fn set_enabled(&mut self, kind: Kind, on: bool) {
        self.enabled.insert(kind, on);
    }

    pub fn handle(&mut self, p: &GamePacket) -> Vec<Encounter> {
        let mut out = Vec::new();
        self.stats.packets += 1;
        let ts = p.ts_ms;
        match decode(&p.data) {
            Decoded::Ignored => {}
            Decoded::Unreadable { opcode, at } => {
                let n = self.stats.unreadable.entry(hex(opcode)).or_default();
                *n += 1;
                // Le premier de chaque type est toujours signalé ; les suivants en mode diagnostic.
                if *n == 1 {
                    log::warn!("paquet {} illisible à l'octet {at} ({} octets) : structure à revoir", hex(opcode), p.data.len());
                } else {
                    log::debug!("paquet {} illisible à l'octet {at} ({} octets)", hex(opcode), p.data.len());
                }
            }
            Decoded::Message(m) => {
                *self.stats.decoded.entry(hex([p.data[0], p.data[1]])).or_default() += 1;
                self.on_message(m, ts, &mut out);
            }
        }
        if ts - self.last_purge > 60_000 {
            self.world.purge(ts);
            self.pending.retain(|x| ts - x.ts < PENDING_TTL_MS);
            self.last_purge = ts;
        }
        out.retain(|e| *self.enabled.get(&e.kind).unwrap_or(&true));
        self.stats.encounters += out.len() as u64;
        out
    }

    fn on_message(&mut self, m: Message, ts: i64, out: &mut Vec<Encounter>) {
        match m {
            Message::PlayerAppear(a) => {
                let key = a.key;
                self.world.on_player_appear(&a, ts);
                // Rencontres en attente de ce pseudo.
                let (ready, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending).into_iter().partition(|x| x.key == key);
                self.pending = rest;
                for x in ready {
                    if ts - x.ts < PENDING_TTL_MS {
                        self.emit(key, x.kind, x.ts, out);
                    }
                }
            }
            Message::NpcAppear { key } => {
                self.world.on_npc_appear(key);
                self.pending.retain(|x| x.key != key);
            }
            Message::PlayerDisappear { key } => self.world.touch(key, ts),
            Message::SelfAppear(s) => self.world.on_self_appear(&s),
            Message::SelfStats { key } => self.world.on_self_stats(key),
            Message::LevelUpdate { key, level } => self.world.on_level(key, level),
            Message::GuildUpdate { key, guild } => self.world.on_guild(key, &guild.name),
            Message::TargetSelf { target } => {
                if target != 0 {
                    self.encounter_with(target, Kind::Cible, ts, out);
                }
            }
            Message::TargetOther { .. } | Message::Refused => {}
            Message::Die(d) => {
                if self.world.is_self(d.victim) && !d.killer_name.is_empty() {
                    // Tué par un joueur : le paquet donne son pseudo même s'il n'a jamais été vu.
                    let p = match self.world.player(d.killer) {
                        Some(p) if p.name == d.killer_name => p.clone(),
                        _ => Player::from_killer(&d.killer_name, d.killer_server, &d.killer_guild, ts),
                    };
                    self.push(p, Kind::Mort, ts, out);
                } else if self.world.is_self(d.killer) {
                    self.encounter_with(d.victim, Kind::Kill, ts, out);
                }
            }
            Message::SkillEffect(e) => {
                // Combat (pas encore vu en PvP réel) : seulement entre Gio et un
                // joueur identifié par son apparition `45 36`. Exclus : effets
                // sur soi, familiers et invocations, monstres, entités inconnues
                // (jamais mises en attente). Seulement de vrais dégâts : un soin
                // ou un buff d'un allié n'est pas un combat.
                if e.amount > 0 && e.damage && e.caster != e.target {
                    let other = if self.world.is_self(e.caster) {
                        Some(e.target)
                    } else if self.world.is_self(e.target) {
                        Some(e.caster)
                    } else {
                        None
                    };
                    if let Some(key) = other {
                        if self.world.player(key).is_some_and(|p| p.seen_appear) && !self.world.is_npc(key) {
                            self.emit(key, Kind::Combat, ts, out);
                        }
                    }
                }
            }
            Message::SelfCp { cp, .. } => self.world.on_self_cp(cp),
            Message::AbnormalAdd(a) => {
                if self.world.is_self(a.target) {
                    let mut seen = Vec::new();
                    for c in a.casters {
                        if c != 0 && c != a.target && !seen.contains(&c) {
                            seen.push(c);
                            // Un buff vient d'un joueur nommé ; on ne met pas en attente les lanceurs inconnus (souvent des PNJ).
                            if self.world.player(c).is_some() {
                                self.emit(c, Kind::Buff, ts, out);
                            }
                        }
                    }
                }
            }
            Message::PartyMembers { full, members } => {
                for key in self.world.on_party_members(full, &members, ts) {
                    self.emit(key, Kind::Groupe, ts, out);
                }
            }
            Message::PartyMemberUpdate(u) => {
                // Groupe formé avant le lancement de LastView : le membre est
                // reconnu par son dbid, déjà vu dans son apparition `45 36`.
                if let Some((key, true)) = self.world.on_party_update(u.dbid, u.level, u.gs, &u.guild, u.cp) {
                    self.emit(key, Kind::Groupe, ts, out);
                }
            }
            Message::PartyLeft => self.world.on_party_left(),
        }
    }

    /// Rencontre avec une entité : immédiate si c'est un joueur nommé, en
    /// attente si l'id est inconnu, ignorée si c'est un PNJ.
    fn encounter_with(&mut self, key: EntityId, kind: Kind, ts: i64, out: &mut Vec<Encounter>) {
        if self.world.is_self(key) || self.world.is_npc(key) {
            return;
        }
        if self.world.player(key).is_some() {
            self.emit(key, kind, ts, out);
        } else if !self.pending.iter().any(|x| x.key == key && x.kind == kind) {
            self.pending.push(Pending { key, kind, ts });
        }
    }

    fn emit(&mut self, key: EntityId, kind: Kind, ts: i64, out: &mut Vec<Encounter>) {
        self.world.touch(key, ts);
        if let Some(p) = self.world.player(key).cloned() {
            self.push(p, kind, ts, out);
        }
    }

    fn push(&self, player: Player, kind: Kind, ts: i64, out: &mut Vec<Encounter>) {
        if player.name.is_empty() || self.world.is_self_player(&player) {
            return;
        }
        out.push(Encounter { ts_ms: ts, kind, player });
    }

    /// Rencontres en attente d'un pseudo (diagnostic).
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::messages::{AbnormalAdd, Die, PartyMember, PlayerAppear, SelfAppear, SkillEffect};

    const MOI: EntityId = 100;

    fn appear(key: EntityId, name: &str) -> Message {
        Message::PlayerAppear(PlayerAppear {
            key,
            name: name.into(),
            pc_id: 13,
            race: 1,
            server: 1303,
            guild: None,
            dbid: key as i64,
            level: 45,
            conqueror_level: 0,
            gs: 2000,
            abyss_rank: 0,
            cp: 100_000,
            target: 0,
            hp: 1,
            hp_max: 1,
        })
    }

    fn engine() -> Engine {
        let mut e = Engine::new();
        let mut out = Vec::new();
        e.on_message(Message::SelfAppear(SelfAppear { key: MOI, name: "Moi".into(), server: 1303, pc_id: 34, level: 45, gs: 2800, cp: 150_000, cp_max: 200_000 }), 0, &mut out);
        e
    }

    fn feed(e: &mut Engine, m: Message, ts: i64) -> Vec<(Kind, String)> {
        let mut out = Vec::new();
        e.on_message(m, ts, &mut out);
        out.into_iter().map(|x| (x.kind, x.player.name)).collect()
    }

    #[test]
    fn cible_immediate_ou_en_attente() {
        let mut e = engine();
        feed(&mut e, appear(7, "JoueurA"), 1);
        assert_eq!(feed(&mut e, Message::TargetSelf { target: 7 }, 2), vec![(Kind::Cible, "JoueurA".into())]);
        // Id inconnu : en attente, puis complété à l'apparition.
        assert!(feed(&mut e, Message::TargetSelf { target: 8 }, 3).is_empty());
        assert_eq!(e.pending_count(), 1);
        assert_eq!(feed(&mut e, appear(8, "JoueurB"), 4), vec![(Kind::Cible, "JoueurB".into())]);
        // Un PNJ ciblé n'est jamais une rencontre.
        feed(&mut e, Message::NpcAppear { key: 9 }, 5);
        assert!(feed(&mut e, Message::TargetSelf { target: 9 }, 6).is_empty());
        assert_eq!(e.pending_count(), 0);
    }

    #[test]
    fn mort_kill_combat_buff() {
        let mut e = engine();
        feed(&mut e, appear(7, "JoueurA"), 1);
        let die = |victim, killer, name: &str| Message::Die(Die { victim, skill: 1, killer, killer_server: 1301, killer_name: name.into(), killer_guild: "Leg".into(), killer_npc: 0 });
        // Tué par un joueur jamais vu : le paquet de mort suffit.
        assert_eq!(feed(&mut e, die(MOI, 55, "Inconnu"), 2), vec![(Kind::Mort, "Inconnu".into())]);
        // Mort tué par un monstre : pas de pseudo, pas de rencontre.
        assert!(feed(&mut e, die(MOI, 56, ""), 3).is_empty());
        assert_eq!(feed(&mut e, die(7, MOI, "Moi"), 4), vec![(Kind::Kill, "JoueurA".into())]);
        let hit = |target, caster, amount| Message::SkillEffect(SkillEffect { target, caster, skill: 1, amount, damage: true });
        assert_eq!(feed(&mut e, hit(7, MOI, 500), 5), vec![(Kind::Combat, "JoueurA".into())]);
        assert_eq!(feed(&mut e, hit(MOI, 7, 500), 6), vec![(Kind::Combat, "JoueurA".into())]);
        assert!(feed(&mut e, hit(MOI, 7, 0), 7).is_empty());
        let buff = Message::AbnormalAdd(AbnormalAdd { target: MOI, casters: vec![7, 7, MOI, 0] });
        assert_eq!(feed(&mut e, buff, 8), vec![(Kind::Buff, "JoueurA".into())]);
    }

    #[test]
    fn exclusions_de_combat() {
        let mut e = engine();
        let hit = |target, caster| Message::SkillEffect(SkillEffect { target, caster, skill: 1, amount: 900, damage: true });
        // Effet sur soi.
        assert!(feed(&mut e, hit(MOI, MOI), 1).is_empty());
        // Familier ou invocation : apparaît comme PNJ (41 36).
        feed(&mut e, Message::NpcAppear { key: 40 }, 2);
        assert!(feed(&mut e, hit(MOI, 40), 3).is_empty());
        assert!(feed(&mut e, hit(40, MOI), 4).is_empty());
        // Monstre : idem, et un PNJ qui réutilise l'id d'un joueur efface le joueur.
        feed(&mut e, appear(41, "JoueurD"), 5);
        feed(&mut e, Message::NpcAppear { key: 41 }, 6);
        assert!(feed(&mut e, hit(41, MOI), 7).is_empty());
        // Entité inconnue : ni rencontre, ni mise en attente, même si un joueur apparaît ensuite avec cet id.
        assert!(feed(&mut e, hit(42, MOI), 8).is_empty());
        assert_eq!(e.pending_count(), 0);
        assert!(feed(&mut e, appear(42, "JoueurE"), 9).is_empty());
        // Membre de groupe jamais vu par 45 36 : pas de Combat.
        let m = PartyMember { key: 43, dbid: 43, server: 1303, name: "JoueurF".into(), pc_id: 13, guild: String::new(), level: 45, gs: 1, cp: 1 };
        feed(&mut e, Message::PartyMembers { full: false, members: vec![m] }, 10);
        assert!(feed(&mut e, hit(43, MOI), 11).is_empty());
        // Coup entre deux autres joueurs : pas de Gio, pas de rencontre.
        feed(&mut e, appear(44, "JoueurG"), 12);
        assert!(feed(&mut e, hit(44, 42), 13).is_empty());
        // Contrôle : un vrai joueur vu par 45 36 → Combat.
        assert_eq!(feed(&mut e, hit(MOI, 44), 14), vec![(Kind::Combat, "JoueurG".into())]);
    }

    #[test]
    fn soin_d_un_allie_n_est_pas_un_combat() {
        let mut e = engine();
        feed(&mut e, appear(50, "Soigneur"), 1);
        // Sans bloc de dégâts (soin, buff) : rien.
        let soin = Message::SkillEffect(SkillEffect { target: MOI, caster: 50, skill: 1, amount: 4000, damage: false });
        assert!(feed(&mut e, soin, 2).is_empty());
        let mien = Message::SkillEffect(SkillEffect { target: 50, caster: MOI, skill: 1, amount: 4000, damage: false });
        assert!(feed(&mut e, mien, 3).is_empty());
    }

    #[test]
    fn groupe_deja_forme_reconnu_par_dbid() {
        use crate::protocol::messages::PartyMemberUpdate;
        let mut e = engine();
        feed(&mut e, appear(60, "Membre"), 1);
        let maj = |dbid| Message::PartyMemberUpdate(PartyMemberUpdate { dbid, pc_id: 13, level: 45, gs: 2100, guild: String::new(), cp: 9 });
        // dbid vu dans l'apparition (dbid = id dans `appear`) : nouveau membre, une seule fois.
        assert_eq!(feed(&mut e, maj(60), 2), vec![(Kind::Groupe, "Membre".into())]);
        assert!(feed(&mut e, maj(60), 3).is_empty());
        // dbid inconnu : rien.
        assert!(feed(&mut e, maj(999), 4).is_empty());
        assert_eq!(e.world.player(60).unwrap().gs, 2100);
    }

    #[test]
    fn cp_du_joueur_local() {
        let mut e = engine();
        assert_eq!(e.world.local.cp, 150_000);
        feed(&mut e, Message::SelfCp { cp: 160_000, cp_max: 211_000 }, 1);
        assert_eq!(e.world.local.cp, 160_000);
    }

    #[test]
    fn groupe_sans_soi() {
        let mut e = engine();
        let m = |key, name: &str| PartyMember { key, dbid: key as i64, server: 1303, name: name.into(), pc_id: 13, guild: String::new(), level: 45, gs: 2000, cp: 1 };
        let full = Message::PartyMembers { full: true, members: vec![m(MOI, "Moi"), m(20, "JoueurC")] };
        assert_eq!(feed(&mut e, full.clone(), 1), vec![(Kind::Groupe, "JoueurC".into())]);
        // Le même groupe renvoyé plus tard ne recrée pas de rencontre.
        assert!(feed(&mut e, full, 2).is_empty());
        feed(&mut e, Message::PartyLeft, 3);
        let add = Message::PartyMembers { full: false, members: vec![m(20, "JoueurC")] };
        assert_eq!(feed(&mut e, add, 4), vec![(Kind::Groupe, "JoueurC".into())]);
    }

    #[test]
    fn jamais_soi_dans_la_liste() {
        let mut e = engine();
        // Un paquet d'apparition à mon nom (autre id) : ignoré.
        feed(&mut e, appear(30, "Moi"), 1);
        assert!(feed(&mut e, Message::TargetSelf { target: 30 }, 2).is_empty());
    }
}
