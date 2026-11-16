# Changelog

Toutes les versions publiées de LastView.

## 0.9.2 — robustesse et diagnostic

- Un module qui plante (moteur, capture, bulle) redémarre tout seul ; l'app reste en vie.
- Base de données corrompue : mise de côté en `.bak`, nouvelle base créée.
- Sans Internet, la fiche affiche les données du cache avec la mention « hors ligne », sans attente.
- Fenêtre restée sur un écran débranché : ramenée sur l'écran principal.
- Après un patch du jeu qui change le protocole : point rouge sur l'œil de la bulle et état « Protocole à mettre à jour » dans la zone de notification.
- Panneau de diagnostic dans les paramètres, avec l'enregistrement de 2 minutes de trafic.
- Changement d'interface réseau (Wi-Fi, Ethernet, VPN) repris en 10 s au lieu de 30.
- Bulle cachée : la boucle de la bulle tourne 8 fois moins vite.

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
