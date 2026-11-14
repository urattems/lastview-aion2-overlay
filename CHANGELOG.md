# Changelog

Toutes les versions publiées de LastView.

## 0.9.1 — test de la mise à jour automatique

- Capture : une interface réseau qui disparaît (VPN coupé, adaptateur retiré) est fermée après 5 erreurs, au lieu d'être relue sans fin en remplissant le journal. Elle est rouverte si elle revient.
- Sert aussi à vérifier de bout en bout la mise à jour automatique depuis la 0.9.0.

## 0.9.0 — première Release de test

- Capture passive (Npcap) et reconnaissance des joueurs autour de vous.
- Rencontres : ciblage, combat, kill, mort, groupe, buff, avec compteur et épingles ; historique de 4 h conservé au redémarrage.
- Bulle au-dessus du jeu : repliée ou déroulée, traversante au repos, cliquable avec Alt, cachée hors du jeu.
- Fiche joueur calquée sur shugo.gg : équipement et infobulles, compétences et stigmas, Daevanion, stats et titres ; copier en image, enregistrer en PNG.
- Recherche par pseudo avec suggestions (liste, joueurs autour, API NCSoft région EU).
- Paramètres complets, appliqués à chaud ; raccourcis globaux.
- Zone de notification : état de la capture, menu complet ; une seule instance ; démarrer avec Windows.
- Mise à jour automatique depuis les Releases GitHub (installateur signé).
