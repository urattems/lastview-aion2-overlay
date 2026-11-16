# Recette V1 — session de jeu d'environ 1 h

Checklist de validation finale (cahier, section 11.2), mise à jour avec toutes les décisions prises depuis. Gio la déroule dans l'ordre, de préférence sur la page cliquable <https://claude.ai/artifact/AVcVw5yFdA7xUiPMZLho6n> (privée, réponses lues directement par Claude), sinon ici en cochant (`[x]`). Un point raté : noter ce qui s'est passé juste en dessous, sans s'arrêter.

Avant de commencer : version installée à jour (menu de l'icône, « À propos de LastView … »), jeu fermé.

## 1. Démarrage (jeu fermé)

- [ ] Après un redémarrage du PC, LastView est lancé tout seul : icône dans la zone de notification, aucune fenêtre ouverte.
- [ ] Infobulle de l'icône : « LastView 0.9.x — En attente du jeu ».
- [ ] Menu de l'icône : état, afficher / masquer la bulle, fiche du dernier joueur, rechercher un joueur, paramètres, mode diagnostic, mise à jour, à propos, quitter.
- [ ] Un second lancement de LastView n'ouvre pas de deuxième instance : il ramène les paramètres.
- [ ] La liste d'avant le redémarrage est toujours là (rencontres de moins de 4 h), et les épinglés aussi.

## 2. Entrée en jeu

- [ ] La bulle apparaît quand le jeu passe au premier plan ; l'état passe à « Capture active ».
- [ ] Si LastView a été lancé après le jeu : une seule notification « Les joueurs déjà autour de toi seront reconnus à leur prochaine apparition ».
- [ ] En ville, avec beaucoup de monde : la liste n'ajoute personne tant que Gio n'interagit avec personne.

## 3. Bulle

- [ ] Au repos : 30 % d'opacité, et les clics passent au jeu (cliquer « à travers » la bulle).
- [ ] Alt tenu, curseur posé un instant sur la bulle : 100 % et cliquable. Un simple passage du curseur en jouant ne la rend pas cliquable.
- [ ] Bulle repliée : le dernier joueur (pseudo, classe, CP en cyan). Elle se déplace en la glissant.
- [ ] Bulle déroulée : 5 lignes visibles (réglable), le reste en faisant défiler ; GS en blanc, CP en cyan, sans flèche ▲▼.
- [ ] Tags et compteur : « Combat ×3 » quand on recroise le même joueur.
- [ ] Étoile : épingler et désépingler un joueur ; le filtre « épinglés » ne montre qu'eux.
- [ ] Alt-Tab vers le bureau : la bulle disparaît. Retour au jeu : elle revient.
- [ ] La nouvelle icône (l'œil-cadran) s'affiche dans la bulle et dans la zone de notification.

## 4. Rencontres

- [ ] Cibler un joueur : il arrive en haut de la liste en moins de 2 s, avec pseudo, classe, GS et CP justes (comparés à Shugo).
- [ ] Combat avec un joueur : ligne créée ou remontée, tag « Combat ».
- [ ] Groupe ou buff d'un joueur : tag « Groupe » ou « Buff ».
- [ ] *(si l'occasion se présente, non bloquant)* Un kill en PvP : tag « Kill ». Une mort : tag « Mort ».
- [ ] *(si l'occasion se présente, non bloquant)* En Abysses normalisées : le CP affiché reste le dernier CP réel, avec le petit « N ».

## 5. Fiche

- [ ] « + » ouvre la fiche à la même place que la dernière fois (ou sur l'écran choisi dans les paramètres).
- [ ] En-tête : portrait, niveau, serveur, titre, iNiv, PC.
- [ ] Les 4 onglets sont complets et ressemblent à Shugo : Équipement, Compétences, Daevanion, Stats.
- [ ] Infobulle d'une pièce : stats, stats additionnelles, pierres de mana, pierre divine, source.
- [ ] Compétences : stigmas groupés et triés comme sur Shugo, passives à la fin.
- [ ] Daevanion : Ariel en orange, Azphel en violet, « Effets actifs » en trois blocs, sans ascenseur interne.
- [ ] « Copier en image » puis Ctrl+V dans Discord : la fiche entière s'affiche, sans les boutons.
- [ ] Clic droit sur « Copier en image », « Enregistrer en PNG » : le fichier arrive dans le dossier choisi dans les paramètres.
- [ ] « Shugo.GG » ouvre la bonne page.

## 6. Recherche

- [ ] Dans la bulle, taper « Ura » : suggestions pendant la frappe (liste, joueurs autour, puis NCSoft). Les joueurs de Vaizel passent en tête, les niveaux inférieurs à 10 en bas.
- [ ] Entrée ouvre la fiche ; le clavier revient aussitôt au jeu.
- [ ] Échap vide le champ et rend le clavier au jeu.
- [ ] La même recherche marche en haut de la fiche, et par « Rechercher un joueur… » dans le menu de l'icône.

## 7. Paramètres et raccourcis

- [ ] Opacité au repos changée : la bulle change tout de suite.
- [ ] Lignes visibles changées : la bulle change tout de suite.
- [ ] Raccourci « masquer / afficher la bulle » (par exemple Ctrl+Maj+F9) : marche en jeu.
- [ ] Raccourcis « fiche du dernier ciblé » et « déplier / replier » : marchent en jeu.
- [ ] Panneau « Diagnostic » : capture active, interface et port du jeu, paquets par seconde qui bougent, protocole « reconnu », API « disponible ».

## 8. Pannes (jeu fermé pour les deux premiers points)

- [ ] Fermer le jeu : la bulle disparaît, l'état passe à « En attente du jeu ». Le relancer : « Capture active » revient tout seul.
- [ ] Couper Internet, ouvrir une fiche déjà vue : « hors ligne, données du cache ». Rien ne gèle. Rétablir Internet, « Actualiser » : données fraîches.
- [ ] En jeu, passer du Wi-Fi à l'Ethernet ou couper / remettre le VPN : la capture reprend en moins de 15 s.
- [ ] Paramètres → Diagnostic → « Enregistrer 2 minutes de trafic » : le compte à rebours défile, puis le fichier apparaît dans « Dossier des enregistrements ».

## 9. Fin de session

- [ ] Aucune baisse de FPS perceptible sur l'heure de jeu.
- [ ] Mesure CPU / RAM de LastView sur l'heure : moins de 2 % de CPU en moyenne, moins de 150 Mo de mémoire (relevé fait par Claude, résultat dans `JOURNAL.md`).
- [ ] Le journal ne contient pas d'avalanche d'avertissements (panneau Diagnostic → « Journaux »).

## Après la recette

- [ ] *(quand Gio y a accès, non bloquant)* Installation sur le 2e PC depuis la Release GitHub.
- Tout est coché : nouvelle icône d'app, tag `v1.0.0`, Release, résumé final pour le chef de suivi.
