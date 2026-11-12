//! **Tous les opcodes utilisés par LastView, à un seul endroit.**
//!
//! Le jour d'un patch, on corrige ici. Chaque opcode porte sa date de
//! vérification sur nos captures, ou « non vérifié en capture ». Un opcode
//! s'écrit comme sur le réseau : les deux premiers octets du paquet.
//! Détails des champs : `docs/protocol.md`.

pub type Opcode = [u8; 2];

/// Ta cible (réponse à ton ciblage). Vérifié le 11/11/2026 (35/35).
pub const TARGET_SELF: Opcode = [0x34, 0x38];
/// « L'entité X cible Y ». Vérifié le 11/11/2026 (36/36).
pub const TARGET_OTHER: Opcode = [0x35, 0x38];
/// Mort d'une entité, avec le pseudo, le serveur et la légion du tueur. Vérifié le 11/11/2026 (45/45).
pub const DIE: Opcode = [0x04, 0x8D];
/// Apparition d'un joueur. Vérifié le 11/11/2026 (106/106).
pub const PLAYER_APPEAR: Opcode = [0x45, 0x36];
/// Disparition d'un joueur (sortie de zone). Structure courte, lue en tête seulement.
pub const PLAYER_DISAPPEAR: Opcode = [0x47, 0x36];
/// Apparition d'un PNJ ou d'un monstre : seul l'id en tête est lu.
pub const NPC_APPEAR: Opcode = [0x41, 0x36];
/// Effet de compétence (dégâts, soins). Vérifié le 11/11/2026 (2 702/2 702).
pub const SKILL_EFFECT: Opcode = [0x04, 0x38];
/// Ajout d'effet (buff, débuff), avec le lanceur. Vérifié le 11/11/2026 (213/213).
pub const ABNORMAL_ADD: Opcode = [0x2A, 0x38];
/// Ta fiche, au chargement de zone. Non vérifié en capture.
pub const SELF_APPEAR: Opcode = [0x33, 0x36];
/// Changement de stats ; l'entité est presque toujours toi. Repli non confirmé pour ton id.
pub const SELF_STATS: Opcode = [0x4A, 0x36];
/// Changement de niveau d'une entité.
pub const LEVEL_UPDATE: Opcode = [0x91, 0x8D];
/// Changement de légion d'une entité.
pub const GUILD_UPDATE: Opcode = [0x33, 0x8A];

/// Ton groupe complet. Non vérifié en capture.
pub const PARTY_INFO: Opcode = [0x00, 0x92];
/// Un joueur rejoint le groupe. Non vérifié en capture.
pub const PARTY_ADD_MEMBER: Opcode = [0x0D, 0x92];
/// Un membre se reconnecte. Non vérifié en capture.
pub const PARTY_RECONNECT_MEMBER: Opcode = [0x22, 0x92];
/// Mise à jour d'un membre (clé : dbid). Non vérifié en capture.
pub const PARTY_UPDATE_MEMBER: Opcode = [0x1C, 0x92];
/// Tu quittes le groupe (réponse). Non vérifié en capture.
pub const PARTY_LEAVE_SELF: Opcode = [0x13, 0x92];
/// Tu es exclu du groupe. Non vérifié en capture.
pub const PARTY_KICKOUT: Opcode = [0x19, 0x92];
/// Groupe dissous. Non vérifié en capture.
pub const PARTY_DISBAND: Opcode = [0x1F, 0x92];
/// Départ du groupe. Non vérifié en capture.
pub const PARTY_LEAVE: Opcode = [0x1F, 0x93];
/// Alliance : un joueur la rejoint. Non vérifié en capture.
pub const FORCE_ADD_MEMBER: Opcode = [0x0A, 0x96];
/// Alliance : un membre se reconnecte. Non vérifié en capture.
pub const FORCE_RECONNECT_MEMBER: Opcode = [0x1B, 0x96];

/// Les opcodes qui identifient le flux du jeu quand la table TCP ne suffit
/// pas (relais, VPN) : les plus fréquents dans nos captures.
pub const SIGNATURE: &[Opcode] = &[[0x00, 0x36], [0x02, 0x38], SKILL_EFFECT, [0x2B, 0x38], [0x1D, 0x37], [0x1C, 0x37], [0x33, 0x8A], SELF_STATS];
