//! CP « normalisé » (Abysses normalisées, décision du chef de suivi du 13/11/2026).
//!
//! Dans les Abysses normalisées, tous les joueurs reçoivent le même CP, le
//! maximum du jeu. L'API et les paquets peuvent alors remonter ce CP au lieu du
//! vrai. On ne rejette pas les gros sauts (un vrai joueur qui se stuffe doit
//! rester visible) : le signal, c'est un CP qui explose alors que l'iNiv ne
//! bouge pas. Un vrai upgrade fait monter les deux.
//!
//! Un CP est normalisé si :
//! - il augmente de plus de 40 % alors que l'iNiv varie de moins de 2 %, par
//!   rapport à la dernière valeur connue du même joueur ;
//! - ou il est égal au plafond de normalisation, appris quand au moins
//!   [`PLAFOND_JOUEURS`] joueurs différents remontent exactement le même CP.

/// Hausse de CP au-delà de laquelle, iNiv stable, le CP est jugé normalisé.
pub const SAUT_CP: f64 = 0.40;
/// Variation d'iNiv en dessous de laquelle l'iNiv est jugé stable.
pub const INIV_STABLE: f64 = 0.02;
/// Joueurs différents au même CP exact pour apprendre le plafond.
pub const PLAFOND_JOUEURS: i64 = 3;

/// `precedent` : dernier CP réel connu du joueur et son iNiv (0 si inconnu).
pub fn est_normalise(precedent: Option<(i64, i64)>, cp: i64, iniv: i64, plafond: Option<i64>) -> bool {
    if cp <= 0 {
        return false;
    }
    if plafond == Some(cp) {
        return true;
    }
    let Some((cp0, iniv0)) = precedent else { return false };
    if cp0 <= 0 || iniv0 <= 0 || iniv <= 0 {
        return false;
    }
    let saut = (cp - cp0) as f64 / cp0 as f64;
    let variation = ((iniv - iniv0) as f64 / iniv0 as f64).abs();
    saut > SAUT_CP && variation < INIV_STABLE
}

/// CP à afficher dans la fiche (CP de l'API) : le dernier CP réel connu s'il
/// est normalisé, sinon le CP de l'API. `mention` explique l'écart.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CpAffiche {
    pub affiche: i64,
    pub api: i64,
    pub normalise: bool,
    pub mention: Option<String>,
}

pub fn cp_fiche(api: i64, normalise: bool, reel: Option<i64>) -> CpAffiche {
    if !normalise {
        return CpAffiche { affiche: api, api, normalise, mention: None };
    }
    let fr = |n: i64| {
        let s = n.to_string();
        let mut out = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push('\u{202f}');
            }
            out.push(c);
        }
        out
    };
    match reel.filter(|r| *r > 0) {
        Some(r) => CpAffiche { affiche: r, api, normalise, mention: Some(format!("CP de l'API normalisé (Abysses) : {}, valeur réelle affichée", fr(api))) },
        None => CpAffiche { affiche: api, api, normalise, mention: Some(format!("CP de l'API normalisé (Abysses) : {}, aucune valeur réelle connue", fr(api))) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saut_de_cp_iniv_stable_normalise() {
        // Le cas d'Urattems : 161 698 → 393 117, iNiv 2 915 → 2 915.
        assert!(est_normalise(Some((161_698, 2915)), 393_117, 2915, None));
    }

    #[test]
    fn saut_de_cp_iniv_en_hausse_vrai_upgrade() {
        // +45 % de CP, iNiv +5 % : un vrai upgrade, affiché tel quel.
        assert!(!est_normalise(Some((100_000, 2000)), 145_000, 2100, None));
        // Petite hausse de CP, iNiv stable : rien d'anormal.
        assert!(!est_normalise(Some((100_000, 2000)), 120_000, 2000, None));
    }

    #[test]
    fn plafond_connu() {
        assert!(est_normalise(None, 393_117, 1500, Some(393_117)));
        assert!(!est_normalise(None, 393_116, 1500, Some(393_117)));
    }

    #[test]
    fn fiche_sans_valeur_reelle() {
        let a = cp_fiche(393_117, true, None);
        assert_eq!(a.affiche, 393_117);
        assert!(a.mention.unwrap().contains("normalisé"));
        let b = cp_fiche(393_117, true, Some(159_022));
        assert_eq!(b.affiche, 159_022);
        assert!(b.mention.unwrap().contains("393\u{202f}117"));
        assert_eq!(cp_fiche(159_022, false, None).mention, None);
    }
}
