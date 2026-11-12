# LastView — Aion 2 Overlay
## Cahier des charges V1 (complet, à exécuter en autonomie)

> **Destinataire : Claude Code**, sur le PC Windows 11 de Gio (joueur Aion 2, perso **Urattems**, serveur **Vaizel**, Élyséens, Aède).
> **Rédigé par : le chef de suivi du projet** (une session Claude dédiée au projet, côté claude.ai). Toutes les décisions ci-dessous ont été prises et validées avec Gio, point par point.
> **Objectif : une V1 quasi définitive.** Pas de V1.1, V1.2… pour rajouter ce qui aurait pu être anticipé. Tout ce qui est listé ici fait partie de la V1.

---

## Sommaire

0. Lis-moi d'abord : rôle, autonomie, règles, communication
1. Vision : ce que fait l'app et ce qu'elle ne fait pas
2. Étape 0 : vérifications avant de construire
3. Architecture
4. Back-end
5. La bulle (overlay)
6. La fiche joueur
7. Recherche par pseudo
8. Paramètres
9. Livraison : repo, build, .exe, mise à jour
10. Robustesse et diagnostic
11. Tests et critères de validation V1

Annexes : A. API NCSoft (vérifiée) · B. Design tokens Shugo (relevés) · C. Design tokens overlay · D. Ce qu'A2Tools apporte · E. Glossaire

---

## 0. Lis-moi d'abord

### 0.1 Ton rôle
Tu es le **développeur** de LastView. Tu crées le repo, tu codes, tu testes, tu livres. Tu as une **grande liberté technique** : ce document fixe le *quoi* et les contraintes, pas chaque ligne de code. Si tu trouves mieux que ce qui est proposé ici, fais-le, et note-le dans `JOURNAL.md` avec la raison.

Tu peux **copier tel quel** ce qui marche dans A2Tools (même licence GPL-3.0), **t'en inspirer**, ou **réécrire** ce qui ne te convient pas. L'app doit appartenir à Gio : son nom, son architecture, son repo, pas un fork renommé.

### 0.2 Ton autonomie sur le PC
Gio te donne **l'accès complet à son PC** pour ce projet : Internet, terminal, navigateur, n'importe quel outil ou application installée ou installable pour coder au mieux (Rust, Node, Git, GitHub CLI, Visual Studio Build Tools, Wireshark, etc.).

Une seule exception : **pendant que Gio joue**, tu ne prends pas la souris ni le clavier, et tu ne fais rien qui ralentisse sa machine (pas de build lourd pendant une session de jeu, sauf s'il te le demande).

### 0.3 Règles de conception non négociables
1. **100% passif vis-à-vis du jeu.** On lit une copie du trafic réseau (Npcap), point. Jamais : lire ou écrire la mémoire du processus du jeu, injecter une DLL, envoyer des paquets au serveur du jeu, simuler des touches ou des clics dans le jeu, modifier les fichiers du jeu.
2. **Tout reste en local.** Aucune donnée envoyée ailleurs que les appels de lecture à l'API publique NCSoft (et GitHub pour la mise à jour). Pas de télémétrie, pas de serveur à nous.
3. **Rien ne doit gêner le jeu** : pas de vol de focus, pas de clic intercepté hors survol volontaire, pas d'impact mesurable sur les FPS.
4. **Le repo est public** (décision de Gio, 10/11/2026) et ne contient jamais de données joueurs, de base locale, de jetons, de secrets ou de captures réseau brutes. Cette règle s'applique aussi aux docs, au journal et aux messages de commit. Tout fichier de test tiré d'une capture est anonymisé (pseudos remplacés, légions retirées).
5. **Ne supprime jamais de fichiers de Gio** en dehors du dossier du projet et du dossier de données de l'app.

### 0.4 Communication : qui décide, comment poser une question
- **Le chef de suivi**, c'est la session Claude de la conversation « chef app last view » sur claude.ai : <https://claude.ai/code/session_017xrHd2LoJi9Ycq9QGa9H8X>. C'est elle qui a mené l'analyse (code A2Tools, API, design Shugo) et qui tranche les questions de fond. Gio est l'intermédiaire.
- **Canal principal** : le fichier `QUESTIONS.md` à la racine du repo. Une question par entrée, avec le contexte, les options que tu vois et ta recommandation. Ensuite, dis à Gio : « J'ai une question pour le chef de suivi dans QUESTIONS.md ». Il la relaie et te rapporte la réponse, que tu notes sous la question.
- **Canal secondaire, autorisé par Gio** : si tu as la main sur l'ordinateur et que Gio n'est pas en jeu, tu peux ouvrir cette conversation dans l'app Claude ou le navigateur et y poser ta question toi-même.
- **Ne bloque pas pour rien.** Pour un détail d'implémentation, décide, note ton choix dans `JOURNAL.md` et avance. Ne pose une question que si le choix change ce que Gio verra ou utilisera, ou s'il est difficile à défaire.
- **Ce que Gio fait lui-même** : le test en jeu de l'étape 0, et les validations finales en jeu. Guide-le pas à pas, en français, phrases courtes.

### 0.5 Fichiers de suivi à tenir dans le repo
| Fichier | Contenu |
|---|---|
| `README.md` | En français : ce que fait l'app, installation, prérequis (Npcap), utilisation, paramètres |
| `JOURNAL.md` | Journal de bord daté : ce qui est fait, choix techniques et pourquoi, écarts par rapport à ce cahier |
| `QUESTIONS.md` | Questions ouvertes pour le chef de suivi, et leurs réponses |
| `CHANGELOG.md` | Versions publiées |
| `docs/protocol.md` | Tout ce qu'on sait des paquets utilisés (opcodes, champs, date de vérification) |
| `docs/api.md` | Les appels NCSoft utilisés, paramètres, champs lus, exemples de réponses anonymisées |
| `LICENSE` + `NOTICE` | GPL-3.0, et le crédit à A2Tools (voir 9.2) |
| `docs/CAHIER_DES_CHARGES.md` | Ce document, versionné |

### 0.6 Ordre de travail
Suis les sections dans l'ordre : **2 (étape 0) → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11**. Chaque section se termine par une **porte** : ce qui doit marcher avant de passer à la suite. Fais un commit propre par étape significative, avec des messages en français.

---

## 1. Vision

### 1.1 Le problème
En jeu, quand Gio cible un joueur (Alt pour libérer la souris, clic sur le joueur, sa bannière apparaît en haut), il veut voir **le stuff réel de ce joueur** : comment il est optimisé, ses lignes, ses pierres, ses compétences, son Daevanion. Aujourd'hui il doit retenir le pseudo et aller sur shugo.gg. Si le joueur part, c'est perdu.

### 1.2 Ce que fait LastView
1. Écoute passivement le trafic du jeu.
2. Mémorise en silence tous les joueurs qui apparaissent autour de Gio.
3. Dès qu'une **rencontre réelle** a lieu (Gio cible le joueur, combat, kill, mort, groupe), le joueur apparaît **en haut d'une petite bulle** par-dessus le jeu, déjà rempli : pseudo, classe, ancienneté, iNiv (« GS »), CP.
4. Un clic sur « + » ouvre une **fiche complète dans une fenêtre à part** (déplaçable sur le 2e écran), calquée sur shugo.gg : équipement, compétences et stigmas, Daevanion, stats.
5. La liste garde l'historique (4 h par défaut) pour retrouver quelqu'un qu'on a raté.

### 1.3 Ce que LastView ne fait PAS (hors V1, volontairement)
- Pas de DPS meter, pas d'analyse de combat (Gio a déjà d'autres outils pour ça).
- Pas de comparaison « lui contre moi » dans la fiche : **la fiche n'affiche que le joueur ciblé**. (La bulle garde une flèche ▲▼ sur le CP par rapport à Gio, ça oui.)
- Pas d'onglet Apparences.
- Pas de notes perso sur les joueurs.
- Pas de « radar à whales » (idée rangée pour plus tard).
- Pas de rayon de détection en mètres (la position de Gio n'est pas lisible en clair, et ce n'est plus utile : seuls les joueurs avec qui il y a interaction s'affichent).
- Pas de multi-région : **région EU uniquement**.

### 1.4 Contexte utile
- OS : Windows 11. Jeu en **plein écran fenêtré** (borderless), donc une fenêtre « toujours au-dessus » s'affiche bien par-dessus.
- Npcap est déjà installé (utilisé par Abyss Logs).
- Gio a un 2e écran et un 2e PC (d'où le .exe téléchargeable et la mise à jour automatique).
- Langue de l'app : **français**. Appels API en `lang=fr-FR`.

---

## 2. Étape 0 : vérifications avant de construire

But : lever les inconnues **avant** d'écrire l'app, pour ne rien construire sur une hypothèse fausse. Fais un petit binaire ou script de test séparé (`tools/`), pas l'app.

### 2.1 Préparer le terrain
- Installer la chaîne d'outils : Rust stable (MSVC), Node LTS, Tauri CLI v2, GitHub CLI (`gh auth login` avec Gio si besoin).
- Créer le repo privé `lastview-aion2-overlay` sur le compte GitHub de Gio (voir 9.1) dès maintenant, pour y committer le journal et les outils de test.
- Cloner A2Tools (<https://github.com/taengu/A2Tools-DPS-Meter>) à côté, en lecture, pour référence.

### 2.2 Test API NCSoft (sans le jeu)
Appeler **directement** (pas via un proxy tiers) les endpoints de l'annexe A pour le perso de Gio : recherche « Urattems » en région `eu`, puis `info`, `equipment`, un `equipment/item` (l'arme, slotPos 1), un `daevanion/detail` (boardId 81), `gameinfo/servers` et `gameinfo/classes`.
Vérifier :
- [ ] Les réponses arrivent en JSON depuis une app desktop (en-têtes nécessaires ? User-Agent ? Referer ?). Noter exactement ce qu'il faut envoyer.
- [ ] Le paramètre `enchantLevel` de `equipment/item` : enchant seul, ou enchant + dépassement ? (Shugo envoie 15 pour un +15 ◆2 ; un autre projet affirme enchant + dépassement.) Comparer les stats renvoyées avec l'infobulle Shugo de la même pièce.
- [ ] Les icônes (`assets.playnccdn.com`) et le portrait (`profileimg.plaync.com`, redirection 302) se chargent depuis la webview Tauri, ou faut-il les télécharger côté Rust et les servir en local.
- [ ] Le comportement en cas de pseudo inexistant, et ce que renvoie la recherche quand plusieurs persos ont le même nom.
Résultat dans `docs/api.md`.

### 2.3 Test de capture « ciblage » (avec Gio, en jeu)
Question : **le serveur envoie-t-il un paquet quand Gio cible un joueur ?** (Le sens client → serveur est chiffré, on ne voit pas le clic lui-même ; on cherche la réaction du serveur.)

Protocole à faire suivre à Gio :
1. Gio se place dans une ville **peu peuplée**, à côté de deux joueurs immobiles (A et B).
2. Tu lances l'enregistreur. Il capture le trafic serveur → client du jeu (port verrouillé comme le fait A2Tools) **et** l'horodatage des clics souris de Gio via un hook souris global **en lecture seule** (aucune injection).
3. **20 secondes sans rien cibler** (référence).
4. **1 minute** : Gio alterne A, B, A, B… environ un clic toutes les 5 secondes.
5. Analyse : chercher un type de paquet qui apparaît à chaque clic (fenêtre de ~500 ms) et qui contient l'identifiant d'entité de A, puis de B, puis de A, en alternance. Les identifiants d'entité de A et B sont connus via leur paquet de spawn.
6. Verdict dans `docs/protocol.md` : trouvé (opcode, structure, position de l'id) ou non trouvé.

**Si trouvé** : la détection du ciblage est passive, c'est la source principale.
**Si non trouvé → plan B, lecture de la bannière de cible** : capture d'une petite zone de l'écran (là où s'affiche le nom de la cible), OCR via l'API Windows intégrée (`Windows.Media.Ocr`), puis correspondance approximative avec les pseudos **déjà connus autour de Gio** (mémoire silencieuse). L'OCR n'a pas à lire un texte inconnu, juste à reconnaître lequel des noms présents est affiché. Déclenchement : au clic gauche quand le curseur est visible, avec un léger délai. Zone réglable dans les paramètres avec un mode « calibrer la zone ». Si tu trouves une autre méthode passive plus fiable, propose-la.

### 2.4 Autres vérifications
- [ ] Le nom du processus du jeu et la classe de sa fenêtre (pour détecter « le jeu est au premier plan »).
- [ ] Le spawn des joueurs fonctionne toujours avec le patch actuel (pseudo, classe, niveau, iNiv, CP, légion, serveur) : lancer le parser A2Tools ou ton extrait sur 5 minutes de capture en ville, et comparer 3 joueurs avec Shugo.
- [ ] L'identification du joueur local (Gio) dans les paquets.
- [ ] Les paquets utiles pour : combat (dégâts reçus et infligés entre joueurs), kill, mort, groupe. Et les **buffs reçus d'un autre joueur** : si un paquet fiable existe, ajoute la source « Buff » ; sinon, laisse-la de côté et note-le.
- [ ] Fonctionnement avec Npcap sans droits administrateur. Sinon, l'app demandera l'élévation au lancement (manifeste).

**Porte 0** : `docs/api.md` et `docs/protocol.md` remplis, verdict ciblage connu, plan choisi. Fais un résumé à Gio pour le chef de suivi avant d'attaquer la suite.

---

## 3. Architecture

### 3.1 Stack imposée
- **Tauri v2** (Rust côté système, webview pour l'interface).
- **Rust** pour : capture (Npcap via `pcap` comme A2Tools), parsing, mémoire, base locale, appels API, gestion des fenêtres, raccourcis globaux, tray.
- **Front** : HTML/CSS/TypeScript. Framework libre (vanilla TS + Vite recommandé, ou Svelte si tu préfères). Pas de lourdeur inutile : l'overlay doit être léger.
- **Base locale** : SQLite (`rusqlite`, fichier dans `%APPDATA%\LastView\`).

### 3.2 Les briques

```
 Npcap ─► Capture ─► Parser ─► Mémoire silencieuse ─► Détecteur de rencontres ─► Liste (SQLite)
 (copie trafic)  (port verrouillé)  (spawn, mort,     (tous les joueurs vus,     (ciblé, combat, kill,       │
                                    combat, groupe,    en RAM, avec TTL)          mort, groupe)               ▼
                                    ciblage)                                                          Fenêtre BULLE
                                                                                                           │ clic +
 Plan B éventuel : OCR bannière ─────────────────────────────►┘                                            ▼
                                                                         Client API NCSoft ◄──── Fenêtre FICHE
                                                                         (file d'attente, cache)   (+ recherche)
```

### 3.3 Les fenêtres
| Fenêtre | Type |
|---|---|
| **Bulle** | Sans bordure, transparente, toujours au-dessus, absente de la barre des tâches, clics traversants hors survol volontaire |
| **Fiche** | Fenêtre normale, redimensionnable, déplaçable sur n'importe quel écran, position et taille mémorisées |
| **Paramètres** | Fenêtre normale (peut être un onglet de la fiche ou une fenêtre à part, au choix) |
| **Tray** | Icône dans la zone de notification avec menu |

### 3.4 Arborescence suggérée (libre)
```
lastview-aion2-overlay/
  src-tauri/src/
    capture/      (Npcap, verrouillage du port, réassemblage TCP — repris d'A2Tools)
    protocol/     (opcodes centralisés, parsers de paquets)
    world/        (mémoire silencieuse, joueur local)
    encounters/   (détection des rencontres, compteur, épingles)
    store/        (SQLite, purge)
    api/          (client NCSoft, file d'attente, cache, modèles)
    overlay/      (fenêtres, survol, premier plan, raccourcis, tray)
    ocr/          (plan B si nécessaire)
    diagnostics/
  src/            (front : bulle, fiche, paramètres)
  tools/          (étape 0, outils de diagnostic)
  docs/
```

---

## 4. Back-end

### 4.1 Capture
- Reprendre la capture A2Tools : écoute Npcap, détection et verrouillage du port du jeu par signature, **uniquement le sens serveur → client**, réassemblage TCP.
- Gestion du changement d'interface réseau (Wi-Fi ↔ Ethernet, VPN) et du changement de port (changement de zone, relais).
- Interface réseau choisie automatiquement, réglable en paramètre.

### 4.2 Parser
- Reprendre uniquement ce qui sert : **spawn joueur**, **mort**, **dégâts entre entités** (pour combat et kill), **groupe**, **joueur local**, **changement de zone / carte**, et le **ciblage** si l'étape 0 l'a trouvé.
- **Tous les opcodes et offsets dans un seul module** (`protocol/opcodes.rs` ou équivalent), chacun commenté avec sa date de vérification. Le jour d'un patch, on corrige à un seul endroit.
- Rappel de ce qu'on sait (voir annexe D) : la famille spawn/mort a été décalée de +1 en juin 2026 (mort `0x3642`, anciennement `0x3641`) ; changement de zone `23 36` avec id d'entité 0 ; chargement de carte `21 36`.

### 4.3 Mémoire silencieuse
- Chaque joueur qui apparaît est mémorisé en RAM : id d'entité de session, pseudo, classe, niveau, iNiv, CP, légion, serveur (si connu), heure de dernière apparition.
- **Rien n'est affiché à ce stade.** C'est un annuaire qui permet de remplir instantanément une ligne de la liste lors d'une rencontre, et qui sert au plan B (OCR) et à la recherche.
- Purge des entrées non revues depuis 30 min (réglable en interne). Mise à jour si le joueur réapparaît avec des valeurs différentes.

### 4.4 Identité d'un joueur
- L'id d'entité ne vit que le temps d'une session : il ne sert pas d'identifiant durable.
- Clé durable : **pseudo + serveur** quand le serveur est connu. Validé à l'étape 0 : c'est quasi systématique (serveur présent dans 106 apparitions sur 106). Sinon, **pseudo + classe**, puis fusion de l'entrée dès que le serveur est appris (via la légion, un kill, ou l'API lors d'un clic sur « + »).

### 4.5 Joueur local (Gio)
- Détecté automatiquement dans les paquets (A2Tools sait le faire). Sert de référence pour la flèche ▲▼ du CP dans la bulle.
- Repli : champs « Mon perso » et « Mon serveur » dans les paramètres (pré-remplis Urattems / Vaizel).
- Le joueur local n'apparaît jamais dans la liste.

### 4.6 Détection des rencontres
Sources (chacune activable ou désactivable dans les paramètres, toutes actives par défaut) :

| Source | Déclencheur | Tag affiché |
|---|---|---|
| **Ciblé** | Gio cible le joueur (paquet `34 38` ; plan B OCR abandonné à l'étape 0) | Ciblé |
| **Combat** | Dégâts échangés entre Gio et ce joueur | Combat |
| **Kill** | Gio a tué ce joueur | Kill |
| **Mort** | Ce joueur a tué Gio | Mort |
| **Groupe** | Le joueur rejoint le groupe ou l'alliance de Gio | Groupe |
| **Buff** | Le joueur a buffé Gio (paquet `2A 38`, confirmé à l'étape 0, actif par défaut) | Buff |

Paquets de groupe et fiche du joueur local (`33 36`) : structures connues mais pas encore vues en capture. Décodage tolérant : un paquet illisible est ignoré sans planter et signalé dans les logs de diagnostic. À confirmer au premier usage réel.

Règles :
- **Une seule ligne par joueur.** Une nouvelle rencontre fait remonter sa ligne en haut, met à jour ses chiffres, l'heure, et le tag (le plus récent ; priorité d'affichage si simultané : Mort > Kill > Combat > Ciblé > Groupe > Buff).
- Les joueurs simplement croisés ne sont **jamais** ajoutés à la liste.
- **Anti-spam** : en plein combat, une même rencontre ne réécrit pas la base plus d'une fois toutes les 5 secondes par joueur.

### 4.7 Compteur de rencontres
- Pour chaque joueur, nombre de **rencontres distinctes** dans la fenêtre de rétention. Deux événements séparés de moins de 10 minutes comptent pour une seule rencontre.
- Affiché discrètement sur la ligne quand il vaut au moins 2 (ex. « ×3 »).

### 4.8 Épingles
- Une étoile sur la ligne épingle le joueur : il **échappe à la purge** et reste dans la liste.
- Les épinglés restent à leur place chronologique (pas de section à part), avec l'étoile pleine. Un filtre « épinglés seulement » est disponible dans la bulle.

### 4.9 Stockage (SQLite)
- Tables suggérées : `players` (identité, dernières valeurs connues), `encounters` (joueur, type, horodatage), `pins`, `api_cache` (réponses JSON + date), `settings`.
- **Rétention** : 4 h ou 150 joueurs (le premier atteint), réglables. Purge au démarrage puis toutes les minutes. Les épinglés ne sont jamais purgés.
- La liste **survit à un redémarrage** de l'app ou du PC.
- Le fichier SQLite est hors du repo (`%APPDATA%\LastView\`), et `.gitignore` bloque `*.db`, `*.pcap`, `*.a2es`.

### 4.10 Client API NCSoft
- **Aucun appel tant que Gio ne clique pas sur « + »** (ou ne lance pas une recherche). La liste est 100% alimentée par les paquets.
- Au clic sur « + » :
  1. Si le `characterId` est déjà connu (cache) : appels `info` + `equipment` en parallèle.
  2. Sinon : recherche par pseudo en région `eu`, filtrée par serveur si connu. Si le serveur est inconnu et que plusieurs persos portent ce nom : départager par classe, niveau et CP les plus proches ; s'il reste plusieurs candidats, la fiche affiche un petit sélecteur.
  3. Détail d'une pièce (`equipment/item`) : à l'ouverture de son infobulle uniquement.
  4. Détail d'un plateau Daevanion : à l'ouverture de l'onglet, plateau affiché d'abord ; les autres à la demande.
- **Cache** : réponses valides 10 minutes (comme Shugo), bouton « Actualiser » pour forcer (autorisé si la donnée a plus de 30 secondes).
- **Respect de l'API** : file d'attente unique, 1 requête à la fois, au moins 1 seconde entre deux requêtes. Sur 403 ou 429 : pause de 10 minutes, état visible dans la fiche (« NCSoft limite les requêtes, reprise dans 9 min »). Sur 404 : « Perso introuvable », pas de nouvel essai pendant 1 h.
- **Parsing défensif** : tous les champs optionnels, valeurs par défaut, jamais de plantage si NCSoft ajoute ou retire un champ. Les champs inconnus vont dans le log de diagnostic.
- **Images** : icônes d'objets, de compétences, du Daevanion et portrait mis en cache disque (`%APPDATA%\LastView\img\`), limite 200 Mo avec nettoyage des plus anciennes.

**Porte 4** : en jeu, la mémoire silencieuse se remplit, une rencontre crée ou remonte une ligne en base avec les bons chiffres, et un appel API complet fonctionne sur Urattems. Montrer à Gio un dump lisible.

---

## 5. La bulle (overlay)

Référence visuelle : maquette « Last View — overlay Aion 2 », artboards **Overlay déroulé** et **Overlay replié** (<https://claude.ai/artifact/9bDg6Lk38cE4WEDZY4YTyX>, ouvrable depuis le navigateur de Gio connecté à claude.ai). Tokens en annexe C. **Gio a validé ce style** : coins légèrement arrondis, filiforme, police Barlow / Barlow Condensed.

### 5.1 État replié (par défaut)
Une petite pastille compacte (≈ 300 × 44 px) :
- icône **œil** (ambre) ;
- **pseudo** du dernier joueur rencontré (gras) ;
- sa **classe** (gris) ;
- son **CP**, en ambre avec ▲ s'il est supérieur au CP de Gio, en bleu avec ▼ s'il est inférieur ;
- chevron ▾ pour dérouler.
Pas de titre « LAST VIEW », pas de compteur de joueurs. S'il n'y a encore personne : œil + « Aucune rencontre ».

### 5.2 État déroulé
- En-tête : œil, « LAST VIEW », chevron ▴ pour replier, champ de recherche (section 7), filtre épinglés.
- En-tête de colonnes : JOUEUR · GS · CP.
- **12 lignes visibles**, puis **défilement** pour remonter jusqu'au bout de la rétention (150 joueurs, 4 h).
- Une ligne (44 px de haut) :
  - pseudo (gras, tronqué avec … si trop long) + tag de rencontre (pastille colorée, désactivable) + compteur « ×n » si ≥ 2 ;
  - sous-ligne : classe · « il y a 3 min » (mise à jour en continu : « à l'instant », « il y a 20 s », « il y a 1 min », « il y a 1 h 12 ») ;
  - **GS** (= iNiv) ;
  - **CP** avec ▲ ambre / ▼ bleu par rapport à Gio ;
  - étoile d'épingle (au survol de la ligne, pleine si épinglé) ;
  - bouton **« + »** (zone cliquable 44 × 44) : ouvre la fiche.
- Une nouvelle rencontre arrive en haut avec une courte mise en évidence (fond qui s'éclaircit puis revient, 1 s), sans faire sauter la vue si Gio est en train de défiler plus bas.

### 5.3 Survol, clics traversants et opacité (point critique)
Fonctionnement d'Aion 2 : en temps normal la souris tourne la caméra et le curseur est caché. Quand Gio appuie sur **Alt**, le curseur apparaît et la souris devient libre.

Comportement voulu :
- **Au repos** : la bulle est **traversante** (les clics passent au jeu, `set_ignore_cursor_events(true)`) et à **30% d'opacité**.
- **Survol volontaire** : quand le **curseur système est visible** (Alt maintenu) **et** qu'il est au-dessus de la bulle, elle passe à **100% d'opacité** et devient cliquable. Détection par sondage de la position et de l'état du curseur (≈ 30 fois par seconde, `GetCursorPos` + `GetCursorInfo`), puisque la fenêtre traversante ne reçoit pas d'événements de survol.
- Curseur qui quitte la bulle ou disparaît : retour à 30% et traversante après 300 ms.
- Opacités au repos et au survol réglables dans les paramètres.
- La bulle **ne prend jamais le focus clavier** au jeu, sauf quand Gio clique dans le champ de recherche (et le rend dès qu'il sort du champ ou appuie sur Échap).

### 5.4 Visibilité
- **Cachée quand le jeu n'est pas au premier plan** (Gio sur son bureau, navigateur, Discord…). Elle réapparaît dès que le jeu revient au premier plan. Exception : elle reste visible si c'est la fiche ou les paramètres de LastView qui ont le focus.
- **Raccourci « masquer / afficher la bulle »** : configurable, **aucune touche assignée par défaut**.
- Position de la bulle : déplaçable en mode survol (poignée sur l'en-tête), mémorisée, sur l'écran du jeu. Si l'écran mémorisé n'existe plus, retour en haut à droite de l'écran principal.
- Option « verrouiller la position ».

**Porte 5** : Gio valide en jeu le replié, le déroulé, le défilement, le survol avec Alt, les clics traversants et le masquage hors jeu.

---

## 6. La fiche joueur

### 6.1 Principes
- **Fenêtre à part**, ouverte par « + », par la recherche ou par le raccourci « fiche du dernier ciblé ». Déplaçable sur le 2e écran, redimensionnable, **position, taille et écran mémorisés**.
- Une seule fiche à la fois : ouvrir un autre joueur remplace le contenu (avec boutons ◀ ▶ d'historique de navigation dans la fiche, c'est un petit plus apprécié).
- **Uniquement les données du joueur ciblé.** Pas de colonne « toi ».
- **Calque visuel de shugo.gg, page personnage.** Gio l'a demandé explicitement : la référence est la page Shugo, à reproduire au plus près (structure, couleurs, dégradés, bordures, tailles, police Noto Sans). Les valeurs relevées sur leur site sont en annexe B. Tu peux ouvrir <https://shugo.gg/fr/character>, chercher « Urattems » et inspecter chaque onglet dans le navigateur de Gio pour affiner. On reproduit leur rendu avec notre propre code : ne copie pas leurs fichiers CSS ou JS compilés.
- Maquette de référence : artboard **Fiche joueur** de la maquette (annexe C pour le lien).
- Pendant le chargement : squelettes (blocs gris animés) à la place exacte du contenu, jamais une page vide.

### 6.2 En-tête (commun à tous les onglets)
- Portrait 96 × 96 (coins 12, bordure bleutée), pastille de niveau dessous.
- **Pseudo** (blanc, gras), badge **serveur**, badge **« Ciblé il y a 20 s »** (type de rencontre + ancienneté, ou « Recherche » si ouvert depuis la recherche).
- « Données API il y a 2 min » (date de la donnée en cache).
- Titre principal (couleur de son grade).
- Ligne : classe • race • pastille **iNiv**.
- Pastille **PC** (cyan).
- Les **3 titres équipés** (attaque, défense, autre) en pastilles.
- Boutons : **Actualiser**, **Shugo.GG ↗** (ouvre la page du perso dans le navigateur par défaut), **Copier en image** (6.8), fermer.

### 6.3 Onglet Équipement
Sections en cartes, dans cet ordre, grille de 3 colonnes :
1. **Armes** : main principale, main secondaire.
2. **Armures** : torse, bas, casque, épaulettes, gants, chaussures, cape.
3. **Accessoires** : ceinture, collier, 2 boucles d'oreilles, 2 anneaux, 2 bracelets, amulette.
4. **Runes** : 2 emplacements.
5. **Arcanes** : 5 emplacements.
6. **Familier et ailes** : familier (nom + niveau), ailes.

Chaque pièce : icône 44 px, badge **◆n** turquoise si dépassement > 0, **« +15 Nom »** dans la couleur du grade, emplacement en dessous. Emplacement vide : carte grisée « Vide ».

**Infobulle de pièce** (au survol ou au clic, appel `equipment/item` à la demande) — même structure que Shugo :
- icône, « +15 Nom » (couleur du grade), grade et catégorie ;
- dépassement en losanges ◆◆◇◇◇ (rempli / max) ;
- « Niv. 86 (Niv. requis 45) » et classe ;
- **STATS DE BASE** : chaque stat, avec le bonus de renforcement entre parenthèses en vert ; les lignes de dépassement en orange ;
- **Taux de lien d'âme** avec barre ;
- **STATS ADDITIONNELLES (n/max)** en vert ;
- **PIERRES DE MANA (n/max)** avec icône et couleur de grade ;
- **PIERRE DIVINE (n/max)** : nom et description ;
- arcanes : leurs compétences et bonus de set si l'API les donne ;
- **SOURCE** en pastilles ;
- « Non échangeable » en rouge si c'est le cas.
L'infobulle reste dans l'écran (repositionnement automatique), se ferme avec Échap, un clic ailleurs, ou la croix.

### 6.4 Onglet Compétences
- **Compétences actives (n)** : grille de tuiles (icône + niveau en badge + nom tronqué).
- **Compétences passives (n)** si l'API les distingue.
- **Compétences Stigma (n, x équipées)** : bloc « Équipé (x) » avec badge « Équipé », puis « Non acquise (n) » grisées avec le niveau requis.
- Infobulle au survol : nom, niveau, catégorie.
- Mention discrète : « La spécialisation des compétences n'est pas fournie par l'API NCSoft. » (Même limite chez Shugo.)

### 6.5 Onglet Daevanion
- Sélecteur des plateaux en onglets (Nezekan, Zikel, Vaizel, Triniel, Ariel, Azphel…) avec « ouverts / total » et une barre de progression.
- **Grille du plateau** (lignes × colonnes renvoyées par `daevanion/detail`) : nœuds ouverts lumineux, fermés en gris, icône par nœud, infobulle avec ses effets.
- Panneau latéral **« Effets actifs (n) »** : améliorations de compétences puis effets de stats, depuis `openSkillEffectList` et `openStatEffectList`.
- Bouton plein écran de la grille.

### 6.6 Onglet Stats
- **Stats divines** : la roue (Force, Dextérité, Précision, Santé, Intelligence, Volonté… au centre, valeurs autour), en reproduction graphique propre (SVG), avec une bascule **Image / Texte** comme Shugo.
- **Stats** : la liste complète de `stat.statList`, groupée par catégorie si possible, chaque stat avec ses effets secondaires (`statSecondList`) en infobulle.
- Bloc **Titres** : les titres équipés avec leurs bonus (`title.titleList`), et « titres possédés n / total ».

### 6.7 États particuliers
- Serveur inconnu et homonymes : sélecteur de candidats (pseudo, classe, niveau, serveur, CP).
- Perso introuvable, API en pause, pas d'Internet : message clair dans la fiche, avec ce qui était en cache s'il y en a.
- Données anciennes (> 10 min) : petite mention + Actualiser.

### 6.8 Copier en image
- Bouton « Copier en image » : capture de l'onglet affiché (en-tête compris) en PNG **dans le presse-papiers**, prêt pour un Ctrl+V dans Discord. Petit message « Copié ✓ ».
- Clic droit sur le bouton : « Enregistrer en PNG… ».

**Porte 6** : les 4 onglets s'affichent sur Urattems et sur 3 autres joueurs. Gio compare côte à côte avec Shugo et valide le rendu.

---

## 7. Recherche par pseudo

- Champ en haut de la bulle déroulée (et en haut de la fiche).
- **Suggestions pendant la frappe**, à partir de 2 caractères, après 300 ms sans frappe :
  1. d'abord les joueurs de la **mémoire silencieuse** et de la liste (instantané, sans réseau) ;
  2. puis la recherche API, **région EU uniquement**.
- Chaque suggestion : pseudo, classe, niveau, serveur. Navigation au clavier (↑ ↓ Entrée, Échap).
- Entrée ou clic : ouvre la fiche. Une recherche n'ajoute pas le joueur à la liste des rencontres.
- Les balises HTML renvoyées par l'API dans le nom (`<strong>`) sont retirées proprement, jamais injectées telles quelles dans la page.

---

## 8. Paramètres

| Groupe | Paramètre | Défaut |
|---|---|---|
| **Liste** | Durée de rétention | 4 h |
| | Nombre maximum de joueurs | 150 |
| | Sources : Ciblé / Combat / Kill / Mort / Groupe / Buff | toutes actives |
| | Afficher les tags de rencontre | oui |
| | Afficher le compteur de rencontres | oui |
| | Vider la liste (bouton, avec confirmation) | — |
| **Bulle** | Opacité au repos | 30% |
| | Opacité au survol | 100% |
| | Masquer quand le jeu n'est pas au premier plan | oui |
| | Verrouiller la position | non |
| | Réinitialiser la position (bouton) | — |
| **Raccourcis** | Masquer / afficher la bulle | aucun |
| | Ouvrir la fiche du dernier ciblé | aucun |
| | Déplier / replier la bulle | aucun |
| **Moi** | Mon perso / Mon serveur (repli si la détection auto échoue) | Urattems / Vaizel |
| **Capture** | Interface réseau | automatique |
| **Fiche** | Ouvrir sur l'écran | dernier utilisé |
| | Durée du cache API | 10 min |
| **Système** | Démarrer avec Windows | **oui** (pour démarrer avant le jeu) |
| | Mise à jour automatique | oui |
| | Mode diagnostic | non |
| | Ouvrir le dossier de données / des logs | — |

- Le client du jeu tourne en administrateur. Les raccourcis passent par `RegisterHotKey` : **vérifié en jeu le 12/11/2026** (12 appuis reçus sur 12, en jeu comme hors jeu). L'option « Lancer LastView en administrateur » est donc inutile.
- Les raccourcis se définissent en appuyant sur la combinaison voulue. Conflit avec un raccourci déjà pris : message clair. Bouton « effacer ».
- Tous les paramètres sont appliqués à chaud, sans redémarrer.

---

## 9. Livraison

### 9.1 Repo GitHub
- Nom : **`lastview-aion2-overlay`**, **public** (décision de Gio, 10/11/2026), sur le compte GitHub de Gio (le connecter avec `gh auth login` si nécessaire, en guidant Gio).
- Nom affiché de l'app : **LastView**. Titre complet : « LastView — Aion 2 Overlay ».
- Branche `main` protégée de toi-même : travaille en branches par étape, fusionne quand la porte de l'étape est passée.
- `.gitignore` : `target/`, `node_modules/`, `*.db`, `*.pcap`, `*.a2es`, dossiers de captures, fichiers d'environnement.

### 9.2 Licence
- **GPL-3.0** (le code repris d'A2Tools l'impose, et ça ne gêne en rien un usage perso).
- `NOTICE` : crédit à A2Tools (<https://github.com/taengu/A2Tools-DPS-Meter>) pour la capture et la connaissance du protocole, en listant les fichiers repris ou adaptés. Garder leurs en-têtes de copyright dans les fichiers copiés.
- Mention « Fan-made, non affilié à NCSOFT » dans le README et dans « À propos ».

### 9.3 Build et .exe
- **GitHub Actions** sur `windows-latest` : à chaque tag `v*`, build Tauri et publication d'une Release avec :
  - l'installateur `.exe` (NSIS) ;
  - le `.exe` portable (lancement direct, WebView2 déjà présent sur Windows 11).
- Version affichée dans « À propos » et dans le menu du tray.
- Pas de signature de code (projet perso) : documenter dans le README l'avertissement SmartScreen (« Informations complémentaires » → « Exécuter quand même »).

### 9.4 Lancement et tray
- Au lancement : pas de fenêtre principale. L'icône LastView apparaît dans la zone de notification, la bulle s'affiche quand le jeu est au premier plan.
- Menu du tray : état (capture active / en attente du jeu / erreur), Afficher la bulle, Ouvrir la fiche du dernier joueur, Rechercher un joueur, Paramètres, Mode diagnostic, À propos, Quitter.
- Une seule instance à la fois (second lancement = ramène l'instance existante).
- Option « Démarrer avec Windows » (plugin autostart), **activée par défaut**.
- Si le jeu tourne déjà au lancement de LastView : notification unique « Les joueurs déjà autour de toi seront reconnus à leur prochaine apparition (changer de zone accélère). »
- Si **Npcap est absent** : message clair au lancement avec le lien de téléchargement officiel (<https://npcap.com>) et la case à cocher conseillée à l'installation.

### 9.5 Mise à jour automatique
- Plugin updater de Tauri, alimenté par les Releases GitHub.
- **Repo public** (décision du 12/11/2026) : l'updater lit directement les Releases publiques, sans jeton GitHub. Aucun paramètre « jeton ». Les Releases restent signées par la clé de l'updater Tauri.
- Vérification au démarrage puis toutes les 6 h. Mise à jour proposée (jamais forcée pendant que le jeu tourne), installée à la fermeture ou sur clic.

**Porte 9** : Gio installe la Release sur son 2e PC à partir de GitHub uniquement, et ça marche.

---

## 10. Robustesse et diagnostic

### 10.1 Patchs du jeu
- Opcodes centralisés (4.2). Si les paquets ne sont plus reconnus (aucun spawn décodé alors que le jeu tourne depuis 2 minutes) : état « Protocole à mettre à jour » dans le tray et dans la bulle (petit point rouge sur l'œil), sans planter.
- Garder un œil sur les commits d'A2Tools : ils corrigent souvent vite après un patch. Documenter dans `docs/protocol.md` comment reporter une correction.

### 10.2 Mode diagnostic
- Active des logs détaillés (fichier tournant, taille plafonnée, dans `%APPDATA%\LastView\logs\`).
- Option « enregistrer 2 minutes de trafic » (format brut horodaté, comme les outils A2Tools) pour analyser un patch. Ces fichiers ne quittent jamais le PC et ne vont jamais dans le repo.
- Panneau d'état : interface réseau, port verrouillé, paquets par seconde, joueurs en mémoire, dernière rencontre, état de l'API, version du protocole.

### 10.3 Pannes à gérer proprement
| Situation | Comportement |
|---|---|
| Jeu fermé | Capture en attente, bulle cachée, tray « en attente du jeu » |
| Changement de zone / téléportation | Mémoire conservée, pas de rencontre fantôme |
| Coupure Internet | Liste et bulle fonctionnent, la fiche affiche le cache ou « hors ligne » |
| API NCSoft modifiée | Champs manquants affichés « — », jamais de plantage |
| Écran débranché | Fenêtres ramenées sur un écran existant |
| Base SQLite corrompue | Renommée en `.bak`, nouvelle base créée, message dans les logs |
| Panique Rust dans un module | Le module redémarre, l'app reste en vie, erreur loggée |

### 10.4 Performance
- CPU < 2% en moyenne en jeu, RAM < 150 Mo.
- La bulle ne se redessine que quand quelque chose change (pas d'animation en boucle).
- Le sondage du curseur est léger (≈ 30 Hz) et s'arrête quand le jeu n'est pas au premier plan.

---

## 11. Tests et critères de validation V1

### 11.1 Tests automatiques (à toi de les écrire et de les faire passer)
- **Parser** : tests unitaires sur des paquets enregistrés (anonymisés si besoin, fichiers de test minimes) pour spawn, mort, dégâts, groupe, ciblage.
- **Rencontres** : règles de la section 4.6 (une ligne par joueur, remontée, priorité des tags, anti-spam), compteur (fusion sous 10 min), purge, épingles.
- **API** : test de contrat sur Urattems (EU, Vaizel) : recherche, info, equipment, item, daevanion. Le test vérifie les champs utilisés, pas les valeurs.
- **File d'attente API** : espacement, pause sur 429 simulé.
- **Front** : rendu de la bulle et de la fiche avec des données factices, captures d'écran comparées à la maquette.
- CI : les tests tournent sur chaque push.

### 11.2 Recette en jeu avec Gio (validation finale)
- [ ] L'app démarre en arrière-plan, icône dans le tray, aucune fenêtre gênante.
- [ ] En ville avec 200 joueurs, la liste reste vide tant que Gio n'interagit avec personne.
- [ ] Gio cible un joueur : il apparaît en haut de la liste en moins de 2 secondes, avec pseudo, classe, GS et CP corrects (vérifiés contre Shugo).
- [ ] Un combat, un kill, une mort créent ou remontent la bonne ligne avec le bon tag.
- [ ] La bulle repliée montre le dernier joueur. Dépliée : 12 lignes, défilement, compteur, épingle.
- [ ] Sans Alt : la bulle est à 30% et les clics passent au jeu. Avec Alt et survol : 100% et cliquable.
- [ ] Alt-Tab vers le bureau : la bulle disparaît. Retour au jeu : elle revient.
- [ ] « + » ouvre la fiche sur le 2e écran, à la même place que la dernière fois.
- [ ] Les 4 onglets sont complets et ressemblent à Shugo à s'y méprendre.
- [ ] Infobulle d'une pièce : stats, lignes, pierres de mana, pierre divine, source.
- [ ] Recherche « Ura » : suggestions, Entrée ouvre la fiche.
- [ ] Copier en image puis Ctrl+V dans Discord fonctionne.
- [ ] Redémarrage du PC : la liste est toujours là (si < 4 h), les épinglés aussi.
- [ ] Aucune baisse de FPS perceptible sur une session de 1 h.
- [ ] Installation sur le 2e PC depuis la Release GitHub, et mise à jour automatique testée avec une version `v1.0.1` de test.

Quand tout est coché : tag `v1.0.0`, Release, résumé final à Gio pour le chef de suivi.

---

## Annexe A — API NCSoft (vérifiée le 10/11/2026 sur Urattems)

API non documentée du site officiel (celle que Shugo utilise). Elle peut changer : parsing défensif obligatoire.

**Identifiants de Gio** : région `eu`, serverId `1303` (Vaizel), characterId `ZOFzGjNorPtjdJSy8Gneu4w20aWFrfvNtYf4kzkJv9A=` (à encoder une seule fois dans l'URL).

| Usage | Requête | Champs principaux |
|---|---|---|
| Recherche | `GET https://api-search.plaync.com/aion2global/search/v2/character?keyword={pseudo}&page=1&size=100&localeInfo=en-US&region=eu&serverId={id ou vide}` | `list[]` : `characterId`, `name` (contient `<strong>`), `race`, `pcId`, `level`, `serverId`, `serverName`, `profileImageUrl`, `region` ; `pagination` |
| Profil | `GET https://aion2.plaync.com/api/character/info?lang=fr-FR&characterId=…&serverId=…&region=eu` | `profile` (`characterName`, `characterLevel`, `className`, `pcId`, `raceName`, `serverName`, `combatPower`, `titleName`, `titleGrade`, `profileImage`) ; `stat.statList[]` (`name`, `type`, `value`, `statSecondList`) ; `title` (`titleList[]` : `name`, `grade`, `equipCategory`, `statList`, `equipStatList` ; `ownedCount`, `totalCount`) ; `daevanion.boardList[]` (`id`, `name`, `icon`, `openNodeCount`, `totalNodeCount`, `openPercent`) ; `ranking` |
| Équipement | `GET https://aion2.plaync.com/api/character/equipment?lang=fr-FR&characterId=…&serverId=…&region=eu` | `equipment.equipmentList[]` (`id`, `name`, `grade`, `enchantLevel`, `exceedLevel`, `icon`, `slotPos`, `slotPosName`) ; `equipment.skinList[]` (ignoré) ; `petwing` (`pet`, `wing`, `wingSkin`) ; `skill.skillList[]` (`id`, `name`, `icon`, `category`, `skillLevel`, `acquired`, `equip`, `needLevel`) |
| Détail pièce | `GET https://aion2.plaync.com/api/character/equipment/item?id={itemId}&enchantLevel={…}&characterId=…&serverId=…&slotPos={n}&lang=fr-FR&region=eu` | `mainStats[]` (`name`, `value`, `extra`, `minValue`, `exceed`), `subStats[]`, `subStatCount`, `magicStoneStat[]` (`name`, `value`, `grade`, `icon`), `magicStoneSlotCount`, `godStoneStat[]` (`name`, `desc`, `grade`, `icon`), `godStoneSlotCount`, `soulBindRate`, `level`, `equipLevel`, `categoryName`, `classNames`, `maxEnchantLevel`, `maxExceedEnchantLevel`, `sources[]`, `tradable` |
| Plateau Daevanion | `GET https://aion2.plaync.com/api/character/daevanion/detail?lang=fr-FR&characterId=…&serverId=…&region=eu&boardId={id}` | `nodeList[]` (`row`, `col`, `nodeId`, `name`, `icon`, `grade`, `type`, `open`, `effectList`) ; `openSkillEffectList[]`, `openStatEffectList[]` (`desc`) |
| Serveurs | `GET https://aion2.plaync.com/en-us/api/gameinfo/servers?lang=en-US&region=eu` | liste des serveurs |
| Classes | `GET https://aion2.plaync.com/en-us/api/gameinfo/classes?lang=en-US&region=eu` | correspondance `pcId` → classe |

Notes :
- Codes `slotPosName` vus : MainHand 1, SubHand 2, Helmet 3, Shoulder 4, Torso 5, Pants 6, Gloves 7, Boots 8, Necklace 10, Earring1 11, Earring2 12, Ring1 13, Ring2 14, Bracelet1 15, Bracelet2 16, Belt 17, Cape 19, Amulet 22, Rune1 23, Rune2 24, Arcana1 à 5 = 41 à 45.
- Grades vus : `Unique`, `Legend`, `Special` (runes), `Rare` (pierres).
- **Icônes** : `https://assets.playnccdn.com/static-aion2-gamedata/resources/{fichier}.png` (256 ou 512 px). Icône PC : `https://assets.playnccdn.com/static-aion2/characters/img/info/profile_power_icon_pc.png`.
- **Portrait** : `https://profileimg.plaync.com/game_profile_images/aion2global/images?gameServerKey={serverId}&charKey=…` (redirection 302 vers un JPEG 512 px).
- Stigmas : selon un autre projet, l'état équipé ou verrouillé se lit dans un champ dédié (vu sous le nom `Dp`) ; à confirmer à l'étape 0.
- Shugo met ses réponses en cache 10 min.
- Un projet tiers qui interroge cette API depuis un serveur s'impose : 1 recherche par seconde au plus, une fiche toutes les 5 s, pause de 10 min sur 403/429. Notre usage (un clic de temps en temps) est bien en dessous, mais la file d'attente reste obligatoire.

## Annexe B — Design tokens Shugo (relevés sur leur page le 10/11/2026)

| Élément | Valeurs |
|---|---|
| Police | Noto Sans ; texte de base `#C9D1D9` |
| Carte de section | dégradé 145° `rgba(18,26,42,.9)` → `rgba(10,16,28,.92)`, rayon 14, padding 14 × 16, ombre `0 14px 32px rgba(0,0,0,.35)`, pas de bordure |
| Titre de section | 14,4 px, 700, `#B8DCFF`, interlettrage 0,4 px |
| Grille d'objets | 3 colonnes de 296 px, espacement 8 |
| Carte d'objet | dégradé 145° `rgba(18,26,42,.9)` → `rgba(12,18,30,.9)`, bordure 1 px couleur du grade à 40%, rayon 10, padding 10 × 12, espacement 12, hauteur 66, ombre `0 10px 24px rgba(0,0,0,.25)` + `inset 0 0 0 1px rgba(255,255,255,.03)` |
| Icône d'objet | 44 × 44, rayon 6 |
| Nom d'objet | 13,6 px, 700, couleur du grade ; emplacement 12 px `#B7C4D4` |
| Grades | Unique `#FFD700` (bordure `rgba(255,215,0,.4)`) · Legend `#6699FF` (`rgba(51,102,255,.4)`) · Special `#00BFFF` (`rgba(0,191,255,.4)`) · familier : nom blanc, bordure `rgba(99,189,255,.35)` |
| Badge dépassement ◆n | 11,2 px, 700, `#5ECFCF`, bordure 1 px `#5ECFCF`, fond dégradé 135° `rgba(20,40,45,.95)` → `rgba(30,60,65,.9)`, lueur `0 0 8px rgba(94,207,207,.5)`, rayon 4, padding 2 × 5 |
| Onglets | hauteur 44, rayon 10, bordure `rgba(120,160,255,.12)`, fond dégradé `rgba(28,34,50,.85)` → `rgba(20,26,42,.88)`, texte 14,7 px 700 majuscules, interlettrage 0,55 px, `rgba(180,200,230,.82)`, ombre `0 2px 8px rgba(0,0,0,.22)` ; actif : bordure et lueur bleues plus marquées, trait lumineux en bas |
| Portrait | 96 × 96, rayon 12, bordure 2 px `rgba(100,140,200,.5)`, ombre `0 4px 12px rgba(0,0,0,.3)` |
| Pastille niveau | fond `#4A5A7A`, bordure 2 px `rgba(100,140,200,.4)`, rayon 9, 12,8 px 700 blanc |
| « Mis à jour » | 11,2 px `rgba(200,215,235,.55)` |
| Titre du perso | 12,5 px `#FFD700` |
| Ligne classe | 13,1 px `#B0C4DE` |
| Pastille iNiv | fond `rgba(255,255,255,.06)`, bordure `rgba(255,255,255,.12)`, rayon 5, padding 4 × 10 ; libellé 11,2 px 500 `rgba(180,195,220,.6)` ; valeur 11,8 px 700 `rgba(230,240,255,.95)` |
| Pastille PC | fond dégradé 135° `rgba(20,35,45,.95)` → `rgba(15,30,40,.95)`, bordure `rgba(120,242,251,.35)`, rayon 8, padding 5,6 × 11,2, texte `#78F2FB` ; libellé « PC » 10,9 px 700 majuscules |
| Pastille de titre | fond `rgba(18,25,38,.85)`, bordure `rgba(255,255,255,.08)`, rayon 8, hauteur 30, texte 13,1 px 600 `#FFD23F` |
| Infobulle d'objet | fond sombre opaque, bordure couleur du grade ; libellés de section en petites majuscules grises ; bonus de renforcement en vert ; lignes de dépassement en orange ; stats additionnelles en vert ; « Non échangeable » en rouge |
| Compétences | tuiles icône + badge de niveau bleu en bas à droite ; stigmas équipés avec badge vert « Équipé » ; non acquises grisées avec « Niv. 22 » |
| Daevanion | onglets de plateaux avec « 73/88 » et barre de progression ; grille de nœuds carrés ; panneau « Effets actifs (n) » à droite |

## Annexe C — Overlay (validé par Gio) et maquette

Maquette : <https://claude.ai/artifact/9bDg6Lk38cE4WEDZY4YTyX> (artboards Overlay déroulé, Overlay replié, Fiche joueur).

| Token | Valeur |
|---|---|
| Polices | Barlow (texte), Barlow Condensed (chiffres, libellés) |
| Fond bulle | `#161B22`, en-tête `#1C222B` |
| Bordure | `#2A313C`, séparateurs de lignes `#222933` |
| Rayon | 10 px |
| Texte / secondaire | `#E6EAF0` / `#9AA4B2` |
| CP supérieur à Gio | ambre `#F2A541` + ▲ |
| CP inférieur à Gio | bleu `#6CB6FF` + ▼ |
| Tags | Mort : fond `#F2A541`, texte `#14181E` · Kill et Combat : fond `#3A2A12`, texte `#F2B866` · Ciblé et Groupe : fond `#15304A`, texte `#8CC8FF` · Buff : fond `#1D3330`, texte `#7FD6C2` |
| Ligne | hauteur 44, bouton « + » 44 × 44 avec séparateur à gauche |
| Icônes | traits fins (SVG), jamais d'emoji dans l'interface |

## Annexe D — Ce qu'A2Tools apporte

Repo : <https://github.com/taengu/A2Tools-DPS-Meter> (Rust + Tauri, GPL-3.0).
- `src-tauri/src/capture/` : capture Npcap (`pcap_capturer.rs`), réassemblage et parsing (`stream_processor.rs`).
- `src-tauri/src/combat/capture_dispatcher.rs` : verrouillage du port par signature, lecture du seul sens serveur → client.
- Spawn joueur : pseudo, classe, niveau, iNiv, CP, légion et serveur (ce dernier seulement si le joueur a une légion, environ 45% des joueurs, ou quand il tue Gio). Validé par leurs devs fin octobre 2026 sur 512 enregistrements de 354 joueurs.
- Mort `0x3642` (anciennement `0x3641`, décalage de juin 2026), changement de zone `23 36` (id 0 = joueur local), chargement de carte `21 36`.
- Le sens client → serveur est chiffré (voir `combat/ping_tracker.rs`).
- Leur système de plugins interdit volontairement les données sur les autres joueurs : on ne peut pas faire un plugin, d'où une app à nous.
- Outils de diagnostic et format de capture brute (`a2t-inspect`, `docs/third-party-meters.md`) : utiles pour l'étape 0 et le mode diagnostic.

## Annexe E — Glossaire

| Terme | Sens |
|---|---|
| **GS / iNiv** | Niveau d'objet moyen (« GS » dans la bulle = « iNiv » chez Shugo et NCSoft) |
| **CP / PC** | Puissance de combat (« CP » dans la bulle = « PC » dans l'API FR) |
| **Dépassement / exceed ◆** | Renforcement au-delà de +15, affiché en losanges |
| **Mémoire silencieuse** | Annuaire en RAM de tous les joueurs vus, jamais affiché tel quel |
| **Rencontre** | Interaction réelle (ciblé, combat, kill, mort, groupe, buff) qui fait entrer un joueur dans la liste |
| **Chef de suivi** | La session Claude de la conversation « chef app last view », qui tranche les questions de fond |
