//! LastView — cœur de l'application, sans interface.
//!
//! Chaîne : capture Npcap → réassemblage TCP → découpage en paquets →
//! décodage → mémoire silencieuse → détection des rencontres → base SQLite.
//! Le client API NCSoft est appelé seulement à la demande de l'utilisateur.
//!
//! 100 % passif vis-à-vis du jeu : on lit une copie du trafic réseau et la
//! table TCP du système. Jamais de lecture de la mémoire du jeu, d'injection,
//! d'envoi de paquets au serveur ni de simulation d'entrée.

pub mod api;
pub mod capture;
pub mod cp;
pub mod diagnostics;
pub mod encounters;
pub mod fixtures;
pub mod hotkey;
pub mod protocol;
pub mod replay;
pub mod store;
pub mod world;

pub use encounters::{Encounter, Engine, Kind};
pub use store::Store;
