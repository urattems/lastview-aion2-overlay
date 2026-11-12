//! Protocole du jeu : découpage du flux en paquets, opcodes, décodage.

pub mod framing;
pub mod messages;
pub mod opcodes;
pub mod reader;

pub use messages::{decode, class_name, Decoded, EntityId, Message};
