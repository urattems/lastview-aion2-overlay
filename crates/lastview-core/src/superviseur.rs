//! Modules supervisés (section 10.3) : une panique dans un fil (moteur,
//! capture, bulle…) est journalisée, puis le module redémarre après une courte
//! pause qui s'allonge si les paniques se répètent. L'app reste en vie.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

/// Texte d'une panique.
pub fn message(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_else(|| "panique sans message".into())
}

/// Exécute `f` et la relance après chaque panique. S'arrête quand `f` se
/// termine normalement. `pause_min` : première pause avant redémarrage.
pub fn superviser(nom: &str, pause_min: Duration, mut f: impl FnMut()) {
    let mut pause = pause_min;
    loop {
        let debut = Instant::now();
        match catch_unwind(AssertUnwindSafe(&mut f)) {
            Ok(()) => return,
            Err(e) => {
                log::error!("module {nom} : panique « {} », redémarrage dans {} s", message(e.as_ref()), pause.as_secs_f32());
                std::thread::sleep(pause);
                // Après une minute sans panique, la pause revient au minimum.
                pause = if debut.elapsed() > Duration::from_secs(60) { pause_min } else { (pause * 2).min(Duration::from_secs(30)) };
            }
        }
    }
}

/// Lance `f` dans un fil nommé, supervisé.
pub fn fil(nom: &'static str, f: impl FnMut() + Send + 'static) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name(format!("lv-{nom}")).spawn(move || superviser(nom, Duration::from_secs(2), f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_module_redemarre_apres_une_panique() {
        let mut essais = 0;
        superviser("essai", Duration::from_millis(1), || {
            essais += 1;
            if essais < 3 {
                panic!("panique simulée {essais}");
            }
        });
        assert_eq!(essais, 3, "deux paniques, puis une exécution normale");
    }

    #[test]
    fn fil_supervise_reste_en_vie() {
        let h = fil("essai-fil", {
            let mut n = 0;
            move || {
                n += 1;
                if n == 1 {
                    panic!("panique simulée");
                }
            }
        })
        .unwrap();
        // Le fil se termine normalement au second passage : la panique ne remonte pas.
        assert!(h.join().is_ok());
    }
}
