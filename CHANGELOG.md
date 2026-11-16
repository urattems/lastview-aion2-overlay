# Changelog

Toutes les versions publiées de LastView.

## 1.0.0 — première version stable

LastView 1.0 reprend tout ce que les versions 0.9 ont construit et testé, plus les corrections de la recette en jeu.

**Ce que fait LastView**
- Écoute passive d'une copie du trafic réseau du jeu (Npcap) : jamais de lecture mémoire, d'injection ni d'envoi de paquets.
- Mémoire silencieuse des joueurs autour de vous ; un joueur n'entre dans la liste qu'à une vraie rencontre : ciblage, combat, kill, mort, groupe, buff.
- Bulle au-dessus du jeu : repliée (dernier joueur) ou déroulée (5 lignes, réglable), GS en blanc, CP en cyan, tags, compteur ×n, épingles. Traversante au repos (30 %), cliquable avec Alt, cachée hors du jeu.
- CP normalisé des Abysses : la bulle et la fiche gardent le dernier CP réel, avec un petit « N ».
- Fiche joueur calquée sur shugo.gg : équipement et infobulles détaillées, compétences et stigmas, Daevanion (plateaux Ariel et Azphel en couleur, effets actifs en trois blocs), stats et titres. Copier en image pour Discord, enregistrer en PNG, lien Shugo.
- Recherche par pseudo avec suggestions pendant la frappe (liste, joueurs autour, puis NCSoft région EU, joueurs de votre serveur et de votre faction en tête).
- Paramètres complets appliqués à chaud ; raccourcis globaux qui marchent en jeu.
- Zone de notification : état de la capture, menu complet ; une seule instance ; démarrage avec Windows ; mise à jour automatique signée.
- Robustesse : modules qui redémarrent seuls, base corrompue mise de côté, fiche hors ligne depuis le cache, fenêtres ramenées si un écran disparaît, alerte « Protocole à mettre à jour » après un patch du jeu, panneau de diagnostic et enregistrement de 2 minutes de trafic.
- Nouvelle icône : l'œil-cadran, « le dernier regard », avec une version dessinée pour 16 px.

**Corrections depuis la 0.9.2 (recette en jeu)**
- Plus de fausse alerte « Protocole à mettre à jour » sur l'écran de sélection du perso : l'alerte attend un vrai trafic de jeu.
- Journal plus propre : « capture active » n'est écrit qu'au changement de serveur ; le mode diagnostic ne détaille plus les bibliothèques réseau.
- Paramètres : la section Raccourcis précise qu'aucun raccourci n'est défini au départ.

**Limites connues / à confirmer en usage réel**
- **Kill en PvP** (tag « Kill ») : déclaré OK en recette, mais aucun kill n'apparaît dans le journal de la session. La mort tuée par un joueur (tag « Mort »), elle, est confirmée.
- **Abysses normalisées** (CP réel gardé, petit « N ») : déclaré OK en recette, non confirmé dans le journal.
- **Changement de réseau en jeu** (Wi-Fi ↔ Ethernet, VPN) : déclaré OK en recette, mais aucune bascule n'est visible dans le journal.
- **Compteur ×n** (même joueur recroisé) : couvert par les tests automatiques, pas encore observé en jeu.

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
