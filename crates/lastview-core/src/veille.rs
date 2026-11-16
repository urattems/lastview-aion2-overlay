//! Veille du protocole (section 10.1). Après un patch du jeu, les opcodes ou
//! les structures peuvent changer : les paquets arrivent, mais plus rien n'est
//! reconnu. Signe sûr : **aucune apparition** (joueur ou PNJ) décodée alors que
//! le trafic du jeu passe depuis 2 minutes. Les PNJ apparaissent partout, même
//! dans une zone vide de joueurs. L'état « Protocole à mettre à jour » n'arrête
//! rien : l'app continue de tourner.

/// Délai sans aucune apparition décodée, trafic présent.
pub const DELAI_MS: i64 = 2 * 60 * 1000;

#[derive(Debug, Default)]
pub struct VeilleProtocole {
    /// Heure du premier paquet du jeu depuis la dernière remise à zéro.
    debut: Option<i64>,
    /// Compteurs au moment de la remise à zéro.
    base_paquets: u64,
    base_spawns: u64,
    alerte: bool,
}

impl VeilleProtocole {
    pub fn new() -> Self {
        Self::default()
    }

    /// Jeu fermé ou relancé : on repart de zéro, sur les compteurs actuels.
    pub fn remettre_a_zero(&mut self, paquets: u64, spawns: u64) {
        *self = Self { base_paquets: paquets, base_spawns: spawns, ..Self::default() };
    }

    /// Observe les compteurs du moteur à l'heure `ts` ; rend `true` si le protocole est à mettre à jour.
    pub fn observer(&mut self, ts: i64, paquets: u64, spawns: u64) -> bool {
        let p = paquets.saturating_sub(self.base_paquets);
        let s = spawns.saturating_sub(self.base_spawns);
        if s > 0 {
            if self.alerte {
                log::info!("protocole : apparitions de nouveau décodées, alerte levée");
            }
            self.alerte = false;
            self.debut = None;
            return false;
        }
        if p == 0 {
            return self.alerte;
        }
        let debut = *self.debut.get_or_insert(ts);
        if !self.alerte && ts - debut >= DELAI_MS {
            self.alerte = true;
            log::error!("protocole à mettre à jour : {p} paquets du jeu en {} s, aucune apparition décodée", (ts - debut) / 1000);
        }
        self.alerte
    }

    pub fn alerte(&self) -> bool {
        self.alerte
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alerte_apres_deux_minutes_sans_apparition() {
        let mut v = VeilleProtocole::new();
        assert!(!v.observer(0, 0, 0), "pas de trafic : pas d'alerte");
        assert!(!v.observer(1_000, 50, 0));
        assert!(!v.observer(1_000 + DELAI_MS - 1, 500, 0));
        assert!(v.observer(1_000 + DELAI_MS, 600, 0));
        // Une apparition décodée lève l'alerte.
        assert!(!v.observer(1_000 + DELAI_MS + 10, 610, 1));
    }

    #[test]
    fn remise_a_zero_au_redemarrage_du_jeu() {
        let mut v = VeilleProtocole::new();
        v.observer(0, 10, 3);
        v.remettre_a_zero(10, 3);
        assert!(!v.observer(1_000, 20, 3));
        assert!(v.observer(1_000 + DELAI_MS, 900, 3), "aucune nouvelle apparition depuis la remise à zéro");
    }
}
