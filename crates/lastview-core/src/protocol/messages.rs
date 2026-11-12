//! Décodage des messages utiles à LastView. Structures : `docs/protocol.md`.
//!
//! Chaque décodeur lit le corps du paquet **sans** l'opcode. Les messages
//! vérifiés sur nos captures sont lus jusqu'au dernier octet ; les autres
//! (fiche perso, groupe) sont lus de façon tolérante : on prend ce qui sert,
//! et un échec est signalé dans les logs de diagnostic.

use super::opcodes::{self as op, Opcode};
use super::reader::{Reader, Short, R};

pub type EntityId = u32;

/// Classe d'un personnage à partir de son `pcId` (4 variantes par classe).
pub fn class_name(pc_id: i32) -> Option<&'static str> {
    const CLASSES: [Option<&str>; 12] = [
        None,
        Some("Gladiateur"),
        Some("Templier"),
        Some("Rôdeur"),
        Some("Assassin"),
        Some("Spiritualiste"),
        Some("Sorcier"),
        Some("Clerc"),
        Some("Aède"),
        None,
        None,
        Some("Combattant"),
    ];
    if pc_id < 1 {
        return None;
    }
    CLASSES.get(((pc_id - 1) / 4) as usize).copied().flatten()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Guild {
    pub id: i64,
    pub name: String,
}

/// Apparition d'un joueur (`45 36`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerAppear {
    pub key: EntityId,
    pub name: String,
    pub pc_id: i32,
    pub race: u8,
    pub server: u16,
    pub guild: Option<Guild>,
    pub dbid: i64,
    pub level: i32,
    pub conqueror_level: i32,
    pub gs: i32,
    pub abyss_rank: i32,
    pub cp: i64,
    pub target: i32,
    pub hp: u64,
    pub hp_max: u64,
}

/// Ta fiche (`33 36`), début seulement.
#[derive(Debug, Clone, PartialEq)]
pub struct SelfAppear {
    pub key: EntityId,
    pub name: String,
    pub server: u16,
    pub pc_id: i32,
    pub level: i32,
    pub gs: i32,
    /// CP actuel et CP max, 0 si la fin du paquet n'a pas pu être lue.
    pub cp: i64,
    pub cp_max: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Die {
    pub victim: EntityId,
    pub skill: i32,
    pub killer: EntityId,
    pub killer_server: u16,
    pub killer_name: String,
    pub killer_guild: String,
    pub killer_npc: EntityId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkillEffect {
    pub target: EntityId,
    pub caster: EntityId,
    pub skill: i32,
    /// Valeur portée par le bit 2 des indicateurs (dégâts ou soin), 0 si absente.
    pub amount: u64,
    /// Bit 1 des indicateurs : bloc « détail des dégâts » présent. Absent sur
    /// les soins et buffs (vérifié en jeu le 12/11/2026 : 6 sur les coups,
    /// 4 sur tout ce que lance le groupe).
    pub damage: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AbnormalAdd {
    pub target: EntityId,
    /// Lanceur de chaque effet ajouté (0 si non précisé).
    pub casters: Vec<EntityId>,
}

/// Membre de groupe ou d'alliance.
#[derive(Debug, Clone, PartialEq)]
pub struct PartyMember {
    pub key: EntityId,
    pub dbid: i64,
    pub server: u16,
    pub name: String,
    pub pc_id: i32,
    pub guild: String,
    pub level: i32,
    pub gs: i32,
    pub cp: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartyMemberUpdate {
    pub dbid: i64,
    pub pc_id: i32,
    pub level: i32,
    pub gs: i32,
    pub guild: String,
    pub cp: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    TargetSelf { target: EntityId },
    TargetOther { who: EntityId, target: EntityId },
    Die(Die),
    PlayerAppear(PlayerAppear),
    PlayerDisappear { key: EntityId },
    NpcAppear { key: EntityId },
    SkillEffect(SkillEffect),
    AbnormalAdd(AbnormalAdd),
    SelfAppear(SelfAppear),
    SelfStats { key: EntityId },
    /// Ton CP (`56 36`) : actuel et max.
    SelfCp { cp: i64, cp_max: i64 },
    LevelUpdate { key: EntityId, level: i32 },
    GuildUpdate { key: EntityId, guild: Guild },
    /// Groupe complet (`00 92`) ou nouveaux membres (ajout, reconnexion, alliance).
    PartyMembers { full: bool, members: Vec<PartyMember> },
    PartyMemberUpdate(PartyMemberUpdate),
    /// Tu n'es plus dans un groupe (départ, exclusion, dissolution).
    PartyLeft,
    /// Demande refusée par le serveur (ciblage impossible…) : rien à faire.
    Refused,
}

/// Résultat du décodage d'un paquet.
#[derive(Debug)]
pub enum Decoded {
    Message(Message),
    /// Opcode sans intérêt pour LastView.
    Ignored,
    /// Opcode connu mais paquet illisible : à signaler en diagnostic.
    Unreadable { opcode: Opcode, at: usize },
}

/// Décode un paquet complet (opcode inclus).
pub fn decode(packet: &[u8]) -> Decoded {
    decode_inner(packet, false).0
}

/// Décode et renvoie aussi les octets réellement lus et la position des
/// chaînes (relatives au paquet) : sert à anonymiser les extraits de test.
pub fn decode_with_strings(packet: &[u8]) -> (Decoded, usize, Vec<(usize, usize)>) {
    decode_inner(packet, true)
}

fn decode_inner(packet: &[u8], record: bool) -> (Decoded, usize, Vec<(usize, usize)>) {
    if packet.len() < 2 {
        return (Decoded::Ignored, packet.len(), Vec::new());
    }
    let opcode: Opcode = [packet[0], packet[1]];
    let body = &packet[2..];
    let mut r = if record { Reader::recording(body) } else { Reader::new(body) };
    let res: Option<R<Message>> = match opcode {
        op::TARGET_SELF => Some(strict(&mut r, target_self)),
        op::TARGET_OTHER => Some(strict(&mut r, target_other)),
        op::DIE => Some(strict(&mut r, |r| die(r).map(Message::Die))),
        op::PLAYER_APPEAR => Some(strict(&mut r, |r| player_appear(r).map(Message::PlayerAppear))),
        op::SKILL_EFFECT => Some(strict(&mut r, |r| skill_effect(r).map(Message::SkillEffect))),
        op::ABNORMAL_ADD => Some(strict(&mut r, |r| abnormal_add(r).map(Message::AbnormalAdd))),
        op::PLAYER_DISAPPEAR => Some(r.vi32().map(|key| Message::PlayerDisappear { key })),
        op::NPC_APPEAR => Some(r.vi32().map(|key| Message::NpcAppear { key })),
        op::SELF_STATS => Some(r.vi32().map(|key| Message::SelfStats { key })),
        op::SELF_CP => Some(strict(&mut r, |r| {
            let (cp, cp_max) = (r.i64()?, r.i64()?);
            if cp <= 0 || cp > cp_max {
                return Err(Short);
            }
            Ok(Message::SelfCp { cp, cp_max })
        })),
        op::LEVEL_UPDATE => Some((|| Ok(Message::LevelUpdate { key: r.vi32()?, level: r.i32()? }))()),
        op::GUILD_UPDATE => Some((|| Ok(Message::GuildUpdate { key: r.vi32()?, guild: guild(&mut r)? }))()),
        // Non vérifiés en capture : lecture tolérante (pas d'exigence de fin exacte).
        op::SELF_APPEAR => Some(self_appear(&mut r).map(Message::SelfAppear)),
        op::PARTY_INFO => Some(party_info(&mut r).map(|members| Message::PartyMembers { full: true, members })),
        op::PARTY_ADD_MEMBER | op::PARTY_RECONNECT_MEMBER => Some(party_member(&mut r).map(|m| Message::PartyMembers { full: false, members: vec![m] })),
        op::FORCE_ADD_MEMBER => Some((|| {
            r.i64()?;
            Ok(Message::PartyMembers { full: false, members: vec![force_member(&mut r)?] })
        })()),
        op::FORCE_RECONNECT_MEMBER => Some(force_member(&mut r).map(|m| Message::PartyMembers { full: false, members: vec![m] })),
        op::PARTY_UPDATE_MEMBER => Some(party_member_update(&mut r).map(Message::PartyMemberUpdate)),
        // Un résultat non nul est un refus du serveur : on ne quitte rien.
        op::PARTY_LEAVE_SELF => Some(r.u16().map(|result| if result == 0 { Message::PartyLeft } else { Message::Refused })),
        op::PARTY_KICKOUT | op::PARTY_DISBAND | op::PARTY_LEAVE => Some(Ok(Message::PartyLeft)),
        _ => None,
    };
    let used = 2 + r.pos();
    let spans = r.take_spans().into_iter().map(|(a, b)| (a + 2, b + 2)).collect();
    let d = match res {
        None => Decoded::Ignored,
        Some(Ok(m)) => Decoded::Message(m),
        Some(Err(_)) => Decoded::Unreadable { opcode, at: r.pos() },
    };
    (d, used, spans)
}

/// Lecture d'un message vérifié : il doit se lire jusqu'au dernier octet.
fn strict(r: &mut Reader<'_>, f: impl FnOnce(&mut Reader<'_>) -> R<Message>) -> R<Message> {
    let m = f(r)?;
    if r.is_done() {
        Ok(m)
    } else {
        Err(Short)
    }
}

fn target_self(r: &mut Reader<'_>) -> R<Message> {
    let flags = r.u8()?;
    let result = r.u16()?;
    let target = r.vi32()?;
    r.opt(flags & 1 != 0, |r| r.i32())?;
    if result != 0 {
        return Ok(Message::Refused);
    }
    Ok(Message::TargetSelf { target })
}

fn target_other(r: &mut Reader<'_>) -> R<Message> {
    let who = r.vi32()?;
    let flags = r.u8()?;
    let target = r.vi32()?;
    r.opt(flags & 1 != 0, |r| r.i32())?;
    Ok(Message::TargetOther { who, target })
}

fn die(r: &mut Reader<'_>) -> R<Die> {
    let victim = r.vi32()?;
    let skill = r.i32()?;
    let killer = r.vi32()?;
    let killer_server = r.u16()?;
    let killer_name = r.str()?;
    let killer_guild = r.str()?;
    r.u16()?; // emblème
    let killer_npc = r.vi32()?;
    r.i32()?; // action
    r.u8()?; // raison
    r.u8()?; // type
    Ok(Die { victim, skill, killer, killer_server, killer_name, killer_guild, killer_npc })
}

fn guild(r: &mut Reader<'_>) -> R<Guild> {
    let id = r.i64()?;
    let name = r.str()?;
    r.u16()?;
    r.u8()?;
    r.i32()?;
    r.i64()?;
    r.u16()?;
    r.str()?;
    Ok(Guild { id, name })
}

struct Desc {
    name: String,
    pc_id: i32,
    race: u8,
}

fn desc(r: &mut Reader<'_>) -> R<Desc> {
    let t = r.u8()?;
    let name = if t & 1 != 0 { r.str()? } else { String::new() };
    let pc_id = r.i32()?;
    let race = if t & 2 != 0 { r.u8()? } else { 0 };
    if t & 4 != 0 {
        r.u8()?;
    }
    if t & 8 != 0 {
        r.u8()?;
        r.i32()?;
    }
    if t & 16 != 0 {
        r.u8()?;
    }
    Ok(Desc { name, pc_id, race })
}

struct State {
    target: i32,
    hp: u64,
    hp_max: u64,
}

fn state(r: &mut Reader<'_>) -> R<State> {
    let t = r.u16()?;
    r.vec3()?;
    r.f32()?;
    r.u16()?;
    if t & 1 != 0 {
        r.vec3()?;
    }
    if t & 2 != 0 {
        r.f32()?;
    }
    if t & 4 != 0 {
        r.u16()?;
    }
    if t & 8 != 0 {
        r.vec3()?;
    }
    if t & 16 != 0 {
        r.vec3()?;
    }
    let target = if t & 32 != 0 { r.i32()? } else { 0 };
    r.u8()?;
    let hp = r.vi64()?;
    let hp_max = r.vi64()?;
    for _ in 0..13 {
        r.i32()?;
    }
    if t & 64 != 0 {
        r.u8()?;
    }
    if t & 128 != 0 {
        r.i32()?;
    }
    if t & 256 != 0 {
        r.u8()?;
    }
    if t & 512 != 0 {
        r.u8()?;
    }
    if t & 1024 != 0 {
        r.vi32()?;
    }
    if t & 2048 != 0 {
        r.i32()?;
    }
    if t & 4096 != 0 {
        r.i32()?;
    }
    if t & 8192 != 0 {
        r.u8()?;
    }
    if t & 16384 != 0 {
        r.i32()?;
    }
    if t & 32768 != 0 {
        r.i32()?; // titre
    }
    Ok(State { target, hp, hp_max })
}

/// Un effet (buff, débuff) ; renvoie son lanceur.
fn abnormal(r: &mut Reader<'_>) -> R<EntityId> {
    let t = r.u8()?;
    r.vi32()?;
    r.i32()?;
    r.i64()?;
    r.i64()?;
    let caster = if t & 1 != 0 { r.vi32()? } else { 0 };
    r.u8()?;
    if t & 2 != 0 {
        r.i32()?;
    }
    r.bool()?;
    if t & 4 != 0 {
        r.vi32()?;
    }
    if t & 8 != 0 {
        r.i32()?;
    }
    if t & 16 != 0 {
        r.vec3()?;
    }
    Ok(caster)
}

fn deco(r: &mut Reader<'_>) -> R<()> {
    for _ in 0..4 {
        r.u8()?;
    }
    r.bool()?;
    r.i32()?;
    r.i32()?;
    r.u8()?;
    r.u8()?;
    r.u8()?;
    r.bool()?;
    for _ in 0..4 {
        r.u16()?;
    }
    r.u8()?;
    r.bool()?;
    r.u16()?;
    r.u16()?;
    Ok(())
}

fn equip(r: &mut Reader<'_>) -> R<()> {
    r.i32()?;
    r.u8()?;
    r.u8()?;
    r.i64()?;
    r.bool()?;
    r.i32()?;
    r.list(deco)?;
    r.list(|r| r.i32())?;
    r.bool()?;
    r.bool()?;
    r.bool()?;
    r.vi32()?;
    Ok(())
}

fn social(r: &mut Reader<'_>) -> R<()> {
    let t = r.u8()?;
    r.u8()?;
    r.i32()?;
    r.u8()?;
    r.u8()?;
    r.bool()?;
    r.i32()?;
    if t & 1 != 0 {
        r.i64()?;
    }
    if t & 2 != 0 {
        r.i32()?;
    }
    if t & 4 != 0 {
        r.i64()?;
    }
    if t & 8 != 0 {
        r.i32()?;
    }
    Ok(())
}

fn player_appear(r: &mut Reader<'_>) -> R<PlayerAppear> {
    let key = r.vi32()?;
    let f = r.u32()?;
    let d = desc(r)?;
    let st = state(r)?;
    let server = if f & 1 != 0 { r.u16()? } else { 0 };
    let g = r.opt(f & 2 != 0, guild)?;
    r.list(abnormal)?;
    r.list(equip)?;
    if f & 4 != 0 {
        r.u8()?;
    }
    if f & 8 != 0 {
        r.u8()?;
    }
    if f & 16 != 0 {
        r.bool()?;
    }
    if f & 32 != 0 {
        r.bool()?;
    }
    if f & 64 != 0 {
        r.u8()?;
    }
    if f & 128 != 0 {
        r.vec3()?;
    }
    if f & 256 != 0 {
        r.i64()?;
    }
    if f & 512 != 0 {
        r.u16()?;
    }
    if f & 1024 != 0 {
        r.u16()?;
    }
    if f & 2048 != 0 {
        r.i64()?;
    }
    if f & 4096 != 0 {
        r.bool()?;
    }
    if f & 8192 != 0 {
        r.u8()?;
    }
    if f & 16384 != 0 {
        r.bool()?;
    }
    let dbid = r.i64()?;
    r.list(|r| {
        r.u16()?;
        r.i32()
    })?;
    let level = r.i32()?;
    let conqueror_level = r.i32()?;
    let gs = r.i32()?;
    for bit in [1 << 15, 1 << 16, 1 << 17] {
        if f & bit != 0 {
            r.i32()?;
        }
    }
    r.list(|r| r.u8())?;
    if f & (1 << 18) != 0 {
        r.u8()?;
    }
    if f & (1 << 19) != 0 {
        r.i32()?;
    }
    if f & (1 << 20) != 0 {
        social(r)?;
    }
    for bit in [1 << 21, 1 << 22, 1 << 23] {
        if f & bit != 0 {
            r.i32()?;
        }
    }
    let abyss_rank = if f & (1 << 24) != 0 { r.i32()? } else { 0 };
    if f & (1 << 25) != 0 {
        r.bool()?;
    }
    if f & (1 << 26) != 0 {
        r.i32()?;
    }
    let cp = r.i64()?;
    r.u8()?;
    r.bool()?;
    Ok(PlayerAppear {
        key,
        name: d.name,
        pc_id: d.pc_id,
        race: d.race,
        server,
        guild: g,
        dbid,
        level,
        conqueror_level,
        gs,
        abyss_rank,
        cp,
        target: st.target,
        hp: st.hp,
        hp_max: st.hp_max,
    })
}

fn self_appear(r: &mut Reader<'_>) -> R<SelfAppear> {
    let key = r.vi32()?;
    let f = r.u32()?;
    let t = r.u8()?;
    let name = if t & 1 != 0 { r.str()? } else { String::new() };
    let server = r.u16()?;
    let pc_id = r.i32()?;
    if t & 2 != 0 {
        r.u8()?;
    }
    let level = r.i32()?;
    let gs = r.i32()?;
    // Contrôle de cohérence : sans lui, un paquet décalé donnerait des valeurs absurdes.
    if name.is_empty() || !(1..=200).contains(&level) || !(0..=100_000).contains(&gs) || class_name(pc_id).is_none() {
        return Err(Short);
    }
    // La suite donne le CP, tout à la fin. Si elle ne se lit pas jusqu'au
    // dernier octet, on garde le début (id, pseudo, GS) et le CP reste inconnu.
    let (cp, cp_max) = match self_appear_tail(r, t, f) {
        Ok(v) if r.is_done() && v.0 > 0 && v.0 <= v.1 => (v.0, v.1),
        _ => (0, 0),
    };
    Ok(SelfAppear { key, name, server, pc_id, level, gs, cp, cp_max })
}

/// Fin de `33 36`, après le GS : renvoie (CP, CP max).
fn self_appear_tail(r: &mut Reader<'_>, t: u8, f: u32) -> R<(i64, i64)> {
    // Fin de la fiche du perso.
    r.i32()?; // GS max
    r.i32()?; // niveau max
    r.i64()?; // expérience
    r.i32()?;
    r.i64()?;
    if t & 4 != 0 {
        r.u8()?;
    }
    r.list(|r| {
        r.u16()?;
        r.i64()?;
        r.u16()?;
        r.u16()
    })?;
    r.list(|r| {
        r.u8()?;
        r.u16()?;
        r.i64()?;
        r.u16()
    })?;
    r.i32()?;
    r.i32()?;
    r.i64()?;
    r.u8()?;
    if t & 8 != 0 {
        r.u8()?;
    }
    let point = |r: &mut Reader<'_>| -> R<()> {
        r.u8()?;
        r.u16()?;
        Ok(())
    };
    if t & 16 != 0 {
        r.list(point)?;
    }
    if t & 32 != 0 {
        r.list(point)?;
    }
    // État, légion, stats, effets, équipement.
    state(r)?;
    r.opt(f & 1 != 0, guild)?;
    r.list(|r| {
        r.u16()?;
        r.i32()
    })?;
    r.list(abnormal)?;
    r.list(equip)?;
    if f & 2 != 0 {
        r.u8()?;
    }
    r.i64()?; // carte
    r.i32()?;
    if f & 4 != 0 {
        r.i32()?;
    }
    if f & 8 != 0 {
        r.i32()?;
    }
    if f & 16 != 0 {
        r.vec3()?;
    }
    if f & 32 != 0 {
        r.u16()?;
    }
    r.i64()?; // dbid
    if f & 64 != 0 {
        r.f32()?;
    }
    if f & 128 != 0 {
        r.i32()?;
    }
    for bit in [256, 512, 1024] {
        if f & bit != 0 {
            r.u8()?;
        }
    }
    if f & 2048 != 0 {
        r.bool()?;
    }
    if f & 4096 != 0 {
        r.vi32()?;
    }
    r.list(|r| r.u8())?;
    r.bool()?;
    if f & 8192 != 0 {
        r.u8()?;
    }
    if f & 16384 != 0 {
        r.vi32()?;
    }
    if f & 32768 != 0 {
        r.i32()?;
    }
    if f & (1 << 16) != 0 {
        r.i64()?;
    }
    if f & (1 << 17) != 0 {
        r.list(|r| {
            let t2 = r.u8()?;
            r.u8()?;
            r.str()?;
            r.i32()?;
            r.vec3()?;
            r.i64()?;
            r.i64()?;
            if t2 & 1 != 0 {
                r.list(|r| r.i64())?;
            }
            Ok(())
        })?;
    }
    for bit in [1 << 18, 1 << 19, 1 << 20] {
        if f & bit != 0 {
            r.i32()?;
        }
    }
    if f & (1 << 21) != 0 {
        r.i64()?;
    }
    if f & (1 << 22) != 0 {
        // Dernière mort : date, serveur et pseudo du tueur, monstre, type.
        r.i64()?;
        r.u16()?;
        r.str()?;
        r.i32()?;
        r.u8()?;
    }
    if f & (1 << 23) != 0 {
        r.u16()?;
    }
    r.i32()?; // niveau de conquérant
    r.i32()?;
    r.i64()?;
    if f & (1 << 24) != 0 {
        r.bool()?;
    }
    if f & (1 << 25) != 0 {
        r.i64()?;
    }
    if f & (1 << 26) != 0 {
        r.i32()?;
    }
    if f & (1 << 27) != 0 {
        for _ in 0..4 {
            r.i32()?;
        }
    }
    let cp = r.i64()?;
    let cp_max = r.i64()?;
    r.list(point)?;
    r.list(point)?;
    if f & (1 << 28) != 0 {
        r.list(|r| {
            r.i32()?;
            r.i32()
        })?;
    }
    if f & (1 << 29) != 0 {
        r.list(|r| {
            r.i32()?;
            r.i32()
        })?;
    }
    Ok((cp, cp_max))
}

fn skill_effect(r: &mut Reader<'_>) -> R<SkillEffect> {
    let target = r.vi32()?;
    let f = r.u16()?;
    let caster = r.vi32()?;
    let skill = r.i32()?;
    r.u8()?;
    if f & 1 != 0 {
        r.i32()?;
    }
    r.u8()?;
    if f & 2 != 0 {
        for _ in 0..6 {
            r.bool()?;
        }
        r.vi64()?;
        r.bool()?;
        r.u8()?;
    }
    r.i32()?;
    r.i32()?;
    r.vi32()?;
    let amount = if f & 4 != 0 { r.vi64()? } else { 0 };
    if f & 8 != 0 {
        r.i32()?;
    }
    if f & 16 != 0 {
        r.bool()?;
    }
    if f & 32 != 0 {
        r.list(|r| r.vi64())?;
    }
    if f & 64 != 0 {
        r.list(|r| r.i32())?;
    }
    if f & 128 != 0 {
        r.u8()?;
    }
    if f & 256 != 0 {
        r.list(|r| r.i32())?;
    }
    r.u16()?;
    if f & 512 != 0 {
        r.vi64()?;
    }
    if f & 1024 != 0 {
        r.vi64()?;
    }
    if f & 2048 != 0 {
        r.i32()?;
    }
    if f & 4096 != 0 {
        r.bool()?;
    }
    if f & 8192 != 0 {
        r.i32()?;
    }
    Ok(SkillEffect { target, caster, skill, amount, damage: f & 2 != 0 })
}

fn abnormal_add(r: &mut Reader<'_>) -> R<AbnormalAdd> {
    let target = r.vi32()?;
    let casters = r.list(abnormal)?;
    Ok(AbnormalAdd { target, casters })
}

fn party_info(r: &mut Reader<'_>) -> R<Vec<PartyMember>> {
    let t = r.u8()?;
    r.i64()?; // clé du groupe
    r.i64()?; // chef
    r.i32()?; // taille max
    for _ in 0..4 {
        r.u8()?;
    }
    if t & 1 != 0 {
        let t2 = r.u8()?;
        r.i32()?;
        if t2 & 1 != 0 {
            r.i32()?;
        }
        r.i32()?;
    }
    if t & 2 != 0 {
        r.list(|r| {
            let t3 = r.u8()?;
            r.u8()?;
            r.i32()?;
            r.str()?;
            if t3 & 1 != 0 {
                r.i32()?;
            }
            Ok(())
        })?;
    }
    if t & 4 != 0 {
        r.list(|r| {
            r.u8()?;
            r.i32()?;
            r.vec3()?;
            r.bool()
        })?;
    }
    if t & 8 != 0 {
        r.list(|r| {
            r.u8()?;
            r.vi32()?;
            r.str()?;
            r.i32()?;
            r.str()?;
            r.i64()?;
            r.i32()?;
            r.i32()
        })?;
    }
    r.list(party_member)
    // La suite (repères, raison) ne sert pas : lecture tolérante.
}

fn party_member(r: &mut Reader<'_>) -> R<PartyMember> {
    let t = r.u8()?;
    r.u8()?; // numéro
    let key = r.i32()? as u32;
    let server = r.u16()?; // serveur d'origine
    r.u16()?; // serveur actuel
    r.str()?; // compte
    let dbid = r.i64()?;
    let name = r.str()?;
    let pc_id = r.i32()?;
    r.vi64()?;
    r.vi64()?;
    for _ in 0..6 {
        r.i32()?;
    }
    r.i64()?;
    r.i32()?;
    r.vec3()?;
    r.bool()?;
    let guild = if t & 1 != 0 { r.str()? } else { String::new() };
    if t & 2 != 0 {
        r.i32()?;
    }
    let level = r.i32()?;
    r.i32()?;
    let gs = r.i32()?;
    r.bool()?;
    r.bool()?;
    if t & 4 != 0 {
        r.list(|r| {
            let t2 = r.u8()?;
            r.i32()?;
            r.u8()?;
            if t2 & 1 != 0 {
                r.vi32()?;
            }
            Ok(())
        })?;
    }
    let cp = r.i64()?;
    r.u8()?;
    check_member(PartyMember { key, dbid, server, name, pc_id, guild, level, gs, cp })
}

fn force_member(r: &mut Reader<'_>) -> R<PartyMember> {
    let t = r.u8()?;
    r.u8()?;
    let key = r.i32()? as u32;
    let server = r.u16()?;
    r.u16()?;
    r.str()?;
    let dbid = r.i64()?;
    let name = r.str()?;
    let pc_id = r.i32()?;
    r.vi64()?;
    r.vi64()?;
    r.i64()?;
    r.i32()?;
    r.bool()?;
    let guild = if t & 1 != 0 { r.str()? } else { String::new() };
    if t & 2 != 0 {
        r.i32()?;
    }
    let level = r.i32()?;
    r.i32()?;
    r.bool()?;
    let gs = r.i32()?;
    r.vec3()?;
    let cp = r.i64()?;
    r.u8()?;
    check_member(PartyMember { key, dbid, server, name, pc_id, guild, level, gs, cp })
}

fn check_member(m: PartyMember) -> R<PartyMember> {
    if m.name.is_empty() || !(1..=200).contains(&m.level) || class_name(m.pc_id).is_none() {
        return Err(Short);
    }
    Ok(m)
}

fn party_member_update(r: &mut Reader<'_>) -> R<PartyMemberUpdate> {
    let dbid = r.i64()?;
    let pc_id = r.i32()?;
    r.i64()?;
    r.i32()?;
    r.vec3()?;
    let level = r.i32()?;
    r.i32()?;
    let gs = r.i32()?;
    let guild = r.str()?;
    r.i32()?;
    r.bool()?;
    let cp = r.i64()?;
    Ok(PartyMemberUpdate { dbid, pc_id, level, gs, guild, cp })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes() {
        assert_eq!(class_name(34), Some("Aède"));
        assert_eq!(class_name(5), Some("Gladiateur"));
        assert_eq!(class_name(1), None);
        assert_eq!(class_name(0), None);
        assert_eq!(class_name(37), None);
    }

    #[test]
    fn ciblage() {
        // 34 38 : indicateurs 0, résultat 0, cible 300 (varint AC 02).
        match decode(&[0x34, 0x38, 0, 0, 0, 0xAC, 0x02]) {
            Decoded::Message(Message::TargetSelf { target }) => assert_eq!(target, 300),
            d => panic!("{d:?}"),
        }
        // Octet en trop : refusé (message vérifié = lecture exacte).
        assert!(matches!(decode(&[0x34, 0x38, 0, 0, 0, 0xAC, 0x02, 9]), Decoded::Unreadable { .. }));
    }

    #[test]
    fn mort_avec_tueur() {
        let mut p = vec![0x04, 0x8D, 0x05];
        p.extend(7i32.to_le_bytes());
        p.push(0x09);
        p.extend(1303u16.to_le_bytes());
        p.extend([5, b'T', b'u', b'e', b'u', b'r']);
        p.extend([0]);
        p.extend(0u16.to_le_bytes());
        p.push(0);
        p.extend(0i32.to_le_bytes());
        p.extend([0, 0]);
        match decode(&p) {
            Decoded::Message(Message::Die(d)) => {
                assert_eq!((d.victim, d.killer, d.killer_server), (5, 9, 1303));
                assert_eq!(d.killer_name, "Tueur");
                assert_eq!(d.killer_guild, "");
            }
            d => panic!("{d:?}"),
        }
    }

    #[test]
    fn opcode_inconnu() {
        assert!(matches!(decode(&[0x00, 0x36, 1, 2, 3]), Decoded::Ignored));
    }
}
