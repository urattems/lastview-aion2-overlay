# Journal de bord — LastView

Journal daté : ce qui est fait, les choix techniques et pourquoi, les écarts par rapport au cahier des charges.

## 2026-11-10 — Création du repo

- Repo `lastview-aion2-overlay` créé sur le compte GitHub de Gio (urattems).
- **Écart par rapport au cahier (9.1)** : le repo est **public**, à la demande explicite de Gio. Conséquences :
  - La règle 0.3.4 (jamais de données joueurs, de base locale, de jetons ni de captures brutes dans le repo) devient encore plus importante : `.gitignore` bloque `*.db`, `*.pcap`, `*.a2es`, les dossiers de captures et les fichiers d'environnement. Les exemples de réponses API dans `docs/api.md` seront anonymisés.
  - La mise à jour automatique (9.5) se simplifie : les Releases d'un repo public sont téléchargeables sans jeton. Le jeton GitHub en lecture seule prévu n'est plus nécessaire. À confirmer avec le chef de suivi (voir `QUESTIONS.md`).
- Fichiers de suivi initialisés : `README.md`, `JOURNAL.md`, `QUESTIONS.md`, `CHANGELOG.md`, `LICENSE` (GPL-3.0), `NOTICE`, `docs/CAHIER_DES_CHARGES.md`, `docs/protocol.md`, `docs/api.md`.
- Emplacement local : `C:\Users\Giovanni\aion2-work\lastview-aion2-overlay`.
- Prochaine étape : section 2 du cahier (étape 0) : chaîne d'outils, clone d'A2Tools en lecture, test API NCSoft, test de capture « ciblage » avec Gio en jeu.

## 2026-11-10 — Étape 0 (section 2), partie autonome

**Outillage (2.1)**
- Installés via winget : Rust stable MSVC (rustup 1.29.3, cargo 1.99.1), Visual Studio 2022 Build Tools avec le workload C++ (MSVC 14.44). Node 24 et npm 11 étaient déjà là. Tauri CLI : sera installé en paquet npm (`@tauri-apps/cli`) au moment du squelette de l'app, plus rapide que `cargo install`.
- A2Tools cloné en lecture dans `C:\Users\Giovanni\aion2-work\A2Tools-DPS-Meter` (v2.0.96 du 10/11/2026).
- Npcap présent, **`AdminOnly = 0`** dans le registre : la capture marche sans élévation. Pas de manifeste admin nécessaire.

**API NCSoft (2.2)** : résultats complets dans `docs/api.md`. Points qui changent le code :
- Aucun en-tête requis.
- `enchantLevel` = enchant **+ dépassement** (vérifié : à 17 l'API ajoute les lignes `exceed: true`, à 15 non).
- Recherche floue : filtrer sur le nom exact côté client ; un pseudo est unique par serveur.
- Stigmas = `category: "Dp"` avec `acquired` / `equip`.
- `characterId` renvoyé encodé (`%3D`) par la recherche, brut (`=`) par `info` : normaliser.
- Icônes avec CORS `*`, portrait en 302 : `<img>` direct possible, cache disque côté Rust pour le hors-ligne.

**Protocole (2.4)** : résumé dans `docs/protocol.md`, repris du code A2Tools.
- Processus `AION2.exe` (deux instances : un parent sans fenêtre, le client avec la fenêtre titrée `AION2`). Connexion jeu observée sur le port 13328.
- Joueur local : record `33 36`, un seul entité l'a. Spawn joueur `45 36` avec classe, niveau, GS, CP, légion + serveur. Mort `42 36` flag 3. Kill `04 8D` (victime, tueur, serveur, nom, légion). Dégâts `04 38`. Roster `02 97`. **Buff `2A 38` porte le lanceur** → la source « Buff » est faisable.
- Choix : pour l'outil de l'étape 0, le flux du jeu est identifié par **la table TCP du système** (connexions établies des PID `AION2.exe`, port 443 exclu) plutôt que par signature de port comme A2Tools. Plus simple, tout aussi passif (on ne touche pas le processus). Pour l'app, on reprendra le verrouillage par signature d'A2Tools **en plus**, pour les cas de relais loopback. À trancher à la section 4.

**Outil `tools/recorder` (lv-recorder)**
- Rust, sans Tauri. `wpcap.dll` chargée dynamiquement (pas de SDK), FFI directe vers user32/kernel32/iphlpapi (pas de crate `windows`, build en quelques secondes).
- `record` : trafic serveur → client + clics (sondage 4 ms en lecture seule) + marqueur F8. `analyze` : remise en ordre TCP, cadrage repris d'A2Tools (varint, lots LZ4), annuaire des entités nommées, corrélation opcode ↔ clics, rapport Markdown.
- Validé à vide avec le jeu ouvert : 290 segments, 1 550 paquets décodés, 0 octet non consommé, fréquences d'opcodes cohérentes avec A2Tools (`04 38`, `42 36`, `2A/2B 38`, `04 8D`).
- Protocole du test en jeu pour Gio : `tools/recorder/PROTOCOLE_TEST.md`.

**Reste pour la porte 0** : le test de ciblage en jeu (2.3) avec Gio, puis le verdict dans `docs/protocol.md`, et la confirmation du spawn joueur sur 3 joueurs contre Shugo (2.4) à partir de la même capture.

## 2026-11-11 — Premier test de ciblage : non concluant, outil corrigé

**Essai 1 (`tools/captures/ciblage1`, 10/11 17:10)**
- Trafic parfait : 2 515 segments, 13 793 paquets décodés, 0 trou, 0 octet perdu. 9 joueurs nommés par leur spawn `45 36`.
- **Clics : aucun capté en jeu.** Seuls 3 clics ont été vus, tous quand une autre fenêtre avait le focus. Cause : le client `AION2.exe` (PID avec la fenêtre) est protégé, son chemin et sa ligne de commande sont illisibles depuis un processus normal. `GetAsyncKeyState` d'un programme non élevé ne voit pas les entrées destinées à cette fenêtre.
- Analyse sans les clics (recherche d'un opcode où l'id alterne entre deux joueurs toutes les ~5 s) : aucune série de plus de 3, ce qui est le niveau du bruit. **Verdict : non concluant** (ni « trouvé », ni « absent »).

**Conséquences**
- Outil : ajout d'un **métronome** (bips aigus = A, graves = B, toutes les 5 s, arrêt automatique). L'heure des bips remplace celle des clics. Analyse A/B : un opcode doit porter l'id de A après les bips A et celui de B après les bips B. Contrôle sur des bips simulés posés au hasard sur la capture 1 : meilleur score 42 %, verdict « rien de net », comme attendu. Seuil retenu : 75 %.
- Outil : relevé à ~30 Hz de l'état du curseur (`GetCursorInfo`) quand le jeu a le focus, et du niveau de privilège du jeu et de l'outil au démarrage.
- **Impact possible sur la bulle (5.3)** : si `GetCursorInfo` ne voit pas non plus les bascules Alt en jeu, la détection du survol volontaire devra passer par autre chose (lancer LastView en administrateur, ou un autre signal). Le prochain test le dira. Si un changement de conception est nécessaire, question au chef de suivi.
- Le plan B du cahier (OCR déclenché « au clic gauche ») a le même problème : on ne voit pas le clic. Il faudrait le déclencher autrement (OCR périodique de la zone de bannière, léger, seulement quand le curseur est visible). À trancher après le test 2.
- Observation annexe : `01 91` (342 octets) arrive très régulièrement toutes les ~3 s. À regarder plus tard, peut-être un état de groupe ou de cible.

## 2026-11-11 — Ciblage trouvé : `34 38` / `35 38`

- Gio a refait le test trois fois avec le métronome (`ciblage2` interrompu, `ciblage3` et `ciblage4` complets).
- Privilèges relevés par l'outil : le client `AION2.exe` tourne **en administrateur** (intégrité haute), le processus parent en intégrité moyenne. Ça explique les clics invisibles.
- Le rapport automatique concluait d'abord « pas de paquet » : l'analyse A/B ne regardait que les ids **nommés**, et les deux joueurs ciblés étaient à l'écran avant le début de l'enregistrement (pas de spawn capté, donc pas de pseudo). En regardant les paquets rares qui suivent les bips : `34 38` et `35 38` arrivent après 11 ou 12 bips sur 12, jamais pendant la référence, et l'id qu'ils portent alterne à chaque bip.
- Analyse corrigée : l'id en tête compte même sans pseudo, et nouveau critère « l'id change à chaque bip », robuste quand Gio inverse A et B en cours de test (c'est arrivé dans les deux essais). Verdict automatique : « Ciblage détecté » sur `ciblage3` et `ciblage4`, rien sur le contrôle à bips aléatoires.
- **Décision** : la source « Ciblé » repose sur `34 38` / `35 38`. Le plan B (OCR) est abandonné pour la V1.
- **Bulle** : `GetCursorInfo` voit bien le curseur apparaître et disparaître avec Alt quand le jeu a le focus. Le survol de la section 5.3 marche sans élévation. `GetAsyncKeyState` ne voit rien en jeu : les raccourcis passeront par `RegisterHotKey`, à vérifier en jeu.
- Point de conception noté dans `docs/protocol.md` : un joueur ciblé dont on n'a pas encore le spawn (déjà à l'écran au lancement de LastView) est mis en attente, puis complété dès que son id est nommé.
- Reste pour la porte 0 : vérifier le décodage du spawn (classe, niveau, GS, CP) sur 3 joueurs contre Shugo. Les captures 3 et 4 contiennent 38 et 43 joueurs nommés : je peux le faire sans Gio.

## 2026-11-11 — Structures exactes des paquets

- Recoupement avec un outil tiers installé sur le PC de Gio, avec son accord. Aucun code repris.
- Structures exactes relevées : `34 38` (ta cible), `35 38` (cible d'une entité), `04 8D` (mort avec le pseudo, le serveur et la légion du tueur), `45 36` (apparition : pseudo, serveur, classe, niveau, GS, CP, rang abyssal, légion).
- Un joueur dont on n'a pas reçu l'apparition ne peut pas être nommé : c'est une limite de toute capture passive. Notre échec du premier test venait seulement de l'heure de lancement de l'enregistreur.
- Vérifié sur nos captures : 116 paquets de mort et de ciblage se lisent jusqu'au dernier octet avec ces structures. Détails dans `docs/protocol.md`.
- **Choix** : LastView lira les paquets champ par champ, avec un lecteur à indicateurs et booléens groupés, plutôt qu'avec les motifs approximatifs d'A2Tools. C'est plus solide face aux patchs. Le code est écrit par nous.
- Question posée au chef de suivi (Q2) : comment présenter l'origine de ces structures dans un repo public.

## 2026-11-11 — Vérification du décodage complet

- Décodeur de test en Python (dossier temporaire, hors repo) pour `45 36`, `04 8D`, `34 38`, `35 38`, `04 38`, `2A 38`. Tous les paquets de ces types dans nos 3 captures se lisent jusqu'au dernier octet : 3 207 paquets, aucun échec.
- **Le serveur est dans toutes les apparitions de joueur** (106/106). La règle 4.4 du cahier se simplifie : la clé « pseudo + serveur » est presque toujours disponible dès l'apparition. Le repli « pseudo + classe » reste pour les cas rares.
- Spawn contre API, 3 joueurs capturés le soir même : classe, niveau, GS et serveur identiques. CP identique pour 2, à 7 points près pour le 3e. Le GS de l'API est la stat `ItemLevel` de `info`.
- Buffs : 18 des 213 ajouts viennent d'un autre joueur. Source « Buff » retenue pour la V1.
- Pas encore vus : `33 36` (soi), groupe, dégâts joueur → joueur. Test court pour Gio : `tools/recorder/PROTOCOLE_TEST2.md`.

## 2026-11-12 — Groupe et fiche perso, test 2 optionnel

- Gio ne peut pas facilement grouper pour le test 2. Les structures ont été relevées par le même recoupement avec un outil tiers, avec son accord.
- Relevé : `33 36` (ta fiche : pseudo, serveur, classe, niveau, GS, CP, et même ta dernière mort avec le pseudo du tueur), et tous les paquets de groupe : `00 92` groupe complet, `0D 92` arrivée, `22 92` reconnexion, `1C 92` mise à jour, `13 92` / `1F 92` / `1F 93` / `19 92` départ, dissolution, exclusion. Détails dans `docs/protocol.md`.
- **Correction** : `02 97`, appelé « roster de groupe » par A2Tools, est la recherche de groupe. La source « Groupe » repose sur `00 92` et `0D 92`.
- **Bonne nouvelle pour la bulle** : un membre de groupe arrive avec son id, son pseudo, son serveur, sa classe, son niveau, son GS et son CP. Pas besoin d'avoir vu son apparition.
- **PvP déjà vérifié** : `ciblage1` contient 38 morts tuées par un joueur. Kill et mort sont validés sur du vrai PvP. *(Corrigé le 12/11 : les « plus de 1 000 effets entre joueurs nommés » annoncés ici étaient des effets lancés sur soi-même. Combat n'est donc pas encore vu en PvP.)*
- **Choix** : le test 2 devient optionnel. `33 36` et le groupe seront vérifiés au premier usage réel. Un paquet qui ne se lit pas jusqu'au bout est ignoré et journalisé, sans planter.
- Porte 0 : tous les points de la section 2 sont traités. Prêt pour la section 3 (architecture) dès l'accord du chef de suivi.

## 2026-11-12 — Porte 0 validée, ajustements du cahier

**Décisions du chef de suivi**
- **Porte 0 validée.**
- **Q1** : le paramètre « jeton GitHub » est supprimé. L'updater lit les Releases publiques. Repo public = aucun secret, jamais.
- **Q2** (décision de Gio) : l'outil tiers n'est pas nommé dans le repo. Les structures vérifiées sur nos captures sont présentées comme telles. Les autres sont marquées « non vérifié en capture, à confirmer au premier usage réel ».
- **Données joueurs** : les pseudos réels sont retirés de la doc (Joueur A, B, C). **Règle** : tout fichier de test tiré d'une capture est anonymisé (pseudos remplacés, légions retirées).
- **Historique** : `etape-0` est fusionnée dans `main` en un seul commit écrasé, puis supprimée sur GitHub.

**Ajustements du cahier (appliqués dans `docs/CAHIER_DES_CHARGES.md`)**
- 4.4 : la clé « pseudo + serveur » est quasi systématique (validé, 106/106 apparitions).
- 4.6 : la source Buff est confirmée (`2A 38`) et active par défaut.
- Groupe et `33 36` non vus en capture : décodage tolérant. Un paquet illisible est ignoré sans planter et signalé dans les logs de diagnostic. À confirmer au premier usage réel.
- « Démarrer avec Windows » passe à **OUI par défaut**. Si le jeu tourne déjà au lancement de LastView, une notification unique s'affiche : « Les joueurs déjà autour de toi seront reconnus à leur prochaine apparition (changer de zone accélère). »
- Le client du jeu tourne en administrateur. `RegisterHotKey` est à vérifier en jeu dès que possible. S'il échoue : option « Lancer LastView en administrateur », via une tâche planifiée à l'ouverture de session, pour éviter la fenêtre UAC à chaque démarrage. Aucun raccourci n'est assigné par défaut.
- 0.3, règle 4 : maintenant que le repo est public, elle s'applique aussi aux docs, au journal et aux messages de commit.
- Verrouillage du flux validé : table TCP du système + signature A2Tools.

**Suite** : section 3 (architecture). Prochain point au chef de suivi à la fin de la porte 4.

## 2026-11-12 — Sections 3 et 4 : architecture et cœur

**Correction de la doc protocole**
- Les « 1 066 effets entre joueurs nommés » annoncés le matin étaient des effets lancés sur soi-même. Aucun dégât d'un joueur sur un autre dans nos captures : la source « Combat » n'est pas encore vue en PvP réel. Kill et mort le sont (38 morts tuées par un joueur).
- Ton id sans `33 36` : repli par l'entité de `4A 36` (changement de stats), marqué « non confirmé ». Il est unique dans 2 captures sur 3, d'où le statut provisoire.

**Architecture (détails : `docs/architecture.md`)**
- **Deux crates** : `lastview-core` (tout le travail, sans Tauri) et `src-tauri` (l'app). Raison : le cœur se teste et se relit sur enregistrement sans fenêtre, et se compile en quelques secondes.
- Capture : `wpcap.dll` chargée à l'exécution, comme l'outil de l'étape 0, plutôt que la crate `pcap` (pas de SDK Npcap à installer pour compiler). Un fil par interface, interfaces relues toutes les 30 s, un seul fil d'assemblage.
- Verrouillage : table TCP du système, et signature en repli après 10 s sans connexion trouvée.
- Réassemblage TCP en continu : doublons rognés, segments en avance mis en attente, trou abandonné après 1,5 s (le découpage se resynchronise seul).
- Décodage champ par champ, opcodes dans `protocol/opcodes.rs` avec leur date de vérification. Messages vérifiés : lecture exacte au dernier octet. Groupe et `33 36` : lecture tolérante avec contrôles de cohérence, paquet illisible compté et journalisé.
- API : `ureq` (bloquant) dans un fil à part, plutôt qu'un client asynchrone. La file unique « 1 requête à la fois » est alors un simple verrou.
- Front : Vite + TypeScript sans framework, une page par fenêtre.
- Icône provisoire (œil ambre sur fond sombre, couleurs de l'annexe C), à remplacer si Gio en veut une autre.

**Résultats**
- 23 tests unitaires : lecteur, découpage, réassemblage (désordre, doublons, trou), décodeurs, détecteur (Mort, Kill, Combat, Buff, Groupe, attente du pseudo, jamais soi dans la liste), base (anti-spam, compteur, fusion de serveur, purge, épingles, plafond), API (identifiants, homonymes, parsing défensif).
- Relecture des 3 enregistrements par toute la chaîne : 29 386 paquets, **0 illisible, 0 trou**. Les comptes par type sont ceux de l'étape 0 (106 apparitions, 2 702 effets, 213 ajouts d'effet). Lignes « Ciblé » créées en base.
- API sur Urattems : recherche, profil (CP, GS, titre), équipement (25 pièces, 35 compétences) en 6,7 s, puis 0 s depuis le cache.
- App Tauri lancée avec le jeu ouvert : jeu trouvé, connexion de Vaizel verrouillée, capture active en moins d'une seconde. Un plantage au démarrage a été corrigé : lecture non alignée d'un pointeur dans la lecture des privilèges du jeu (aussi corrigé dans `tools/recorder`).
- Écoute en direct de 2 min : 2 411 paquets, 0 illisible, mais aucun joueur autour (Gio AFK). **La partie « en jeu » de la porte 4 reste à faire avec Gio** : `docs/PROTOCOLE_PORTE4.md`.

**Pas encore fait dans la section 4, reporté**
- Cache disque des images (200 Mo) : avec la fiche, section 6, où les images servent.
- Champs inconnus de l'API dans le journal de diagnostic : avec la fiche.
- Compilations faites en priorité basse sur 4 cœurs, le jeu étant ouvert.

## 2026-11-12 — Porte 4 validée en jeu

- Test de Gio avec `lastview-dump live` (`docs/PROTOCOLE_PORTE4.md`), à Vaizel, dans une zone peuplée.
- **37 ciblages en 3 min, 37 fois le bon joueur** d'après Gio : pseudo, classe, GS et CP identiques à la bannière en jeu. 35 joueurs distincts en base, tous avec serveur, et légion quand ils en ont une. Les deux joueurs ciblés deux fois ont une seule ligne.
- Pendant le test, Gio a aussi bougé la caméra, attaqué des monstres et récolté : **aucun faux « Ciblé »**. `34 38` ne part qu'au vrai ciblage, et les monstres ciblés sont écartés.
- API complète sur Urattems : validée plus tôt dans la journée.
- **Point ouvert** : ton id n'a été que deviné (`4A 36`), jamais confirmé par `33 36`. Soit il n'y a pas eu de changement de zone, soit le paquet était illisible. Le journal ne le disait qu'en mode diagnostic. Corrigé : le premier paquet illisible de chaque type est maintenant toujours signalé, et la confirmation du joueur local aussi. À revoir au prochain test.

## 2026-11-12 — Décisions de la porte 4 appliquées, tests renforcés

- `etape-3` fusionnée dans `main` sans écrasement (`22a2964`), après vérification que `1e2d433` était bien poussé. Branche `etape-5` ouverte.

**Décisions du chef de suivi**
1. **Référence CP / GS de Gio** (flèches ▲▼) : paquets d'abord (`33 36` lu en entier pour le CP, `56 36` pour le CP en cours de session), sinon l'API sur « Mon perso / Mon serveur » au démarrage puis toutes les 30 min, sinon aucune flèche. Source exposée dans l'état (`reference.source` : `paquet`, `api`, `aucune`).
2. **File API** : une opération = une ouverture de fiche, une infobulle, un plateau, une recherche. Profil et équipement partent ensemble (2 requêtes au plus), 1 s d'espacement entre deux opérations. **Interprétation à valider** : le clic sur « + » est une seule opération, recherche comprise quand le `characterId` n'est pas connu (recherche, puis profil et équipement ensemble). Sans cela, recherche + 1 s + fiche dépasse 2 s.
   - Mesures réelles sur Urattems, sans cache : **0,64 s, 0,74 s, 1,76 s, 1,77 s**. NCSoft répond en 0,55 à 1,8 s par requête (curl : 0,58 s pour `info`, 0,55 à 0,75 s pour `equipment`). Les 6,7 s de la veille contenaient la recherche, 2 × 1 s d'espacement, et trois connexions TLS à froid.
3. **Combat** : seulement entre Gio et un joueur vu par `45 36`. Familiers et invocations (PNJ), monstres, entités inconnues et effets sur soi exclus, sans mise en attente.

**Tests (chef de suivi, complément)**
- Extraits anonymisés de nos 3 enregistrements dans `crates/lastview-core/tests/fixtures/` (1 295 paquets) : `lastview-dump fixture`. Seuls les paquets décodés sont gardés, et seulement les octets lus. Chaque chaîne est remplacée (`AnonNNN`), les longueurs sont recalculées, et le paquet anonymisé doit se décoder à nouveau. Contrôle : aucun pseudo connu, aucun mot lisible restant.
- `tests/non_regression.rs` : chaque extrait rejoué dans toute la chaîne (lots LZ4, segments dans le désordre et en double, séquence TCP qui repasse par zéro) : 0 illisible, 0 trou, comptes de ciblages, morts, buffs et apparitions, rencontres et lignes en base.
- `tests/robustesse.rs` : 200 000 paquets aléatoires, tous les vrais paquets tronqués à chaque longueur et altérés, 20 000 tampons de découpage, 50 000 segments TCP aléatoires. Aucun plantage, en debug (contrôles de débordement) comme en release.
- File API avec un faux serveur : espacement, 2 requêtes simultanées, clic sur « + » en une opération, pause sur 429, 404. Test de contrat réel sur Urattems (`--ignored`) : passe.
- GitHub Actions (`.github/workflows/tests.yml`, `windows-latest`) : front, tests du cœur, compilation de l'app à chaque push.

## 2026-11-12 — Section 5 : la bulle (partie code)

- **Raccourci global** : `lastview-dump raccourci` enregistre Ctrl+Maj+F9 avec `RegisterHotKey` et note chaque appui, en précisant si le jeu est au premier plan. À tester en jeu (le jeu tourne en administrateur).
- **Survol (5.3)**, dans `src-tauri/src/overlay.rs` :
  - au repos, la fenêtre est traversante et à 30 % ;
  - sondage à 30 Hz de `GetCursorInfo` : 100 % et cliquable seulement si le curseur est visible **et** au-dessus de la bulle ;
  - retour au repos 300 ms après la sortie.
  - La fenêtre a le style `WS_EX_NOACTIVATE` et s'affiche sans activation (`SW_SHOWNOACTIVATE`) : un clic sur la bulle ne prend jamais le focus au jeu.
  - Le champ de recherche retire ce style le temps de la saisie, puis rend le premier plan à la fenêtre d'avant (Échap ou sortie du champ).
- **Visibilité (5.4)** : la bulle est cachée si ni le jeu ni une fenêtre de LastView n'est au premier plan. Le menu de la zone de notification la masque ou l'affiche. Position déplaçable par l'en-tête, mémorisée (500 ms après la fin du déplacement). Elle revient en haut à droite si l'écran mémorisé n'existe plus. Option de verrouillage lue dans les paramètres.
- **Front** :
  - replié et déroulé d'après la maquette validée, 12 lignes puis défilement ;
  - tags, compteur ×n, « il y a … » mis à jour chaque seconde ;
  - flèches CP par rapport à la référence de Gio, sans flèche si la référence est inconnue ;
  - étoile d'épingle, bouton « + », filtre épinglés, filtre par pseudo dans la liste ;
  - mise en évidence d'1 s d'une nouvelle rencontre ;
  - ancre de défilement : la vue ne saute pas quand une ligne arrive en haut.
- La fenêtre suit la taille du contenu : la zone de survol est exactement la bulle visible.
- Polices Barlow et Barlow Condensed **embarquées** (`@fontsource`, licence OFL), sans appel à Google Fonts : tout reste local.
- Chaque clic reçu par la bulle est journalisé (`bulle : clic sur …`) : c'est la preuve, après 30 min de jeu, qu'elle n'en intercepte aucun hors survol.
- **Mode démo** (`bulle.html?demo`, hors Tauri seulement) : la bulle avec les joueurs fictifs de la maquette, pour les captures et la vérification du rendu sans le jeu. Captures faites : le rendu correspond à la maquette.
- `lastview.exe --diagnostic` : mode diagnostic dès le lancement.
- Séance de jeu préparée pour Gio : `docs/PROTOCOLE_PORTE5.md`. Elle couvre le raccourci, la bulle, 30 min de jeu, `33 36`, le groupe (donjon en recherche automatique) et le PvP, avec l'enregistreur en parallèle pour les prochains extraits de test.

## 2026-11-12 — Correctifs avant la séance de la porte 5

Signalé par Gio au début de la séance : la bulle et la fiche affichaient une page d'erreur d'Edge (« localhost a refusé de se connecter »), et la bulle cachait la mini-carte.
- **Cause 1** : l'app était compilée par `cargo` directement, sans l'option `custom-protocol` que l'outil Tauri ajoute d'habitude. Elle cherchait donc le serveur de développement (`localhost:1420`). Correction : option activée d'office dans `src-tauri/Cargo.toml`. Vérifié par une capture de la fenêtre fiche : elle affiche bien sa page.
- **Cause 2** : la bulle interrogeait l'app avant que son état soit enregistré (`state not managed`). Son script s'arrêtait, et la bulle restait vide. Correction : l'état est enregistré avant la création des fenêtres. Les erreurs du front vont maintenant dans le journal (`front bulle : …`).
- **Position** : par défaut, la bulle est à droite au tiers de la hauteur, sous la mini-carte. La mise en place au démarrage était enregistrée comme un déplacement de Gio. Ces mouvements sont maintenant ignorés pendant 3 s, et la position enregistrée par erreur a été effacée.
- **Leçon** : mon essai de lancement ne vérifiait que le journal, pas le contenu des fenêtres. Il vérifie maintenant la taille et la position réelles de la bulle, et l'absence d'erreur du front.

## 2026-11-12 — Séance de jeu de la porte 5 : ce que les paquets montrent

Enregistrement de 44 min (13:08 à 13:52 UTC) et journal de l'app. 520 643 paquets relus par toute la chaîne : **0 illisible, 0 trou**.
- **`33 36` vérifié** : 9/9 lus en entier, joueur local confirmé. L'id deviné par `4A 36` était le bon. **CP lu = CP de l'API (159 717)** : la référence ▲▼ vient maintenant du jeu.
- **Groupe vérifié** : `00 92` (1), `1C 92` (1 628), `1F 93` (1) lus jusqu'au bout. Le groupe de donjon existait avant l'enregistrement, donc aucun « groupe complet » n'a été reçu au départ. **Ajout** : un `dbid` de `1C 92` inconnu du groupe mais vu dans une apparition `45 36` est un membre. Les 4 membres sont reconnus.
- **PvP vu, et un faux positif trouvé** : un Clerc du groupe était compté en « Combat » à cause de ses soins. Analyse de `04 38` : le bit 1 des indicateurs (bloc « détail des dégâts ») est présent sur tous les coups (`6`), et absent sur tout ce que lance le groupe (`4`). **Correction** : Combat exige ce bloc. Après correction : 9 joueurs adverses en « Combat », aucun membre du groupe, et une « Mort » tuée par un joueur adverse.
- **Extrait anonymisé `porte5.txt`** (0-160 s et 990-1 040 s, 512 Ko), avec le test `porte5_groupe_fiche_pvp` : fiche confirmée avec CP, 4 membres de groupe, du Combat sans aucun membre du groupe, une Mort. L'outil d'extraction garde maintenant sans plafond les effets qui touchent un joueur, et il rejoue l'extrait anonymisé pour contrôle.
- **Bulle** : la séance a commencé avec la version cassée (page d'erreur). Le moteur du survol a bien réagi (entrées et sorties de survol dans le journal), mais le test de 30 min de la bulle reste à refaire avec la version corrigée. Le résultat du test du raccourci est dans la fenêtre PowerShell de Gio : à récupérer.

## 2026-11-12 — `RegisterHotKey` validé en jeu

- `lastview-dump raccourci` (Ctrl+Maj+F9, 1 min) : **12 appuis reçus sur 12**. 8 avec le jeu au premier plan (4 curseur caché en mode caméra, 4 curseur visible avec Alt), 4 hors du jeu.
- Le jeu tourne en administrateur, mais un raccourci global enregistré auprès de Windows arrive quand même à LastView, qui n'est pas élevé. **Décision** : les raccourcis (section 8) passent par le plugin Tauri `global-shortcut`, qui repose sur `RegisterHotKey`. L'option « Lancer LastView en administrateur » prévue en repli n'est pas nécessaire : elle n'est pas faite.

## 2026-11-12 — 30 min de jeu avec la bulle (porte 5, 5.3)

- Gio a laissé la main pendant qu'il jouait en donjon. LastView a été arrêté, recompilé en priorité « inactive » et relancé en mode diagnostic, avec l'enregistreur à côté. Ni la souris ni le clavier n'ont été touchés.
- **28 min de jeu, bulle déroulée au-dessus du jeu : 0 clic reçu par la bulle**. Elle n'a intercepté aucun clic.
- Ta fiche a été confirmée par `33 36` à chaque changement de zone (6 fois, un nouvel id à chaque zone). 11 membres de groupe, 80 buffs, 4 ciblages. **Aucun faux « Combat »** depuis la correction des soins. Les « Combat » avec 3 coéquipiers relevés entre 15:10 et 15:13 venaient de l'ancienne version, encore lancée à ce moment-là.
- **6 survols, tous involontaires** : le curseur visible a traversé la bulle en 0,06 à 0,19 s. Pendant ce temps, un clic aurait été pris par la bulle. **Ajout** : le curseur doit rester 250 ms sur la bulle avant qu'elle devienne cliquable (`ENTER_DWELL`). Un survol voulu dure bien plus longtemps.
- Reste à faire avec Gio : la partie interactive (dérouler, défiler, étoile, « + », déplacement, masquage avec Alt+Tab) et des captures pour le chef de suivi.

## 2026-11-12 — Retours de Gio sur la bulle en jeu (écarts au cahier 5.1, 5.2 et annexe C)

Gio a utilisé la bulle en vrai, en jeu (affichage avec le jeu, repli et déroulé au clic : OK). Ses demandes, appliquées :
- **5 lignes visibles** au lieu de 12, le reste en défilant. Raison : un groupe fait 5 joueurs, et 12 lignes font une liste trop longue par-dessus le jeu. Réglable (`lignes_visibles`, de 3 à 20), le paramètre sera à la section 8.
- **Bulle repliée déplaçable** : un glisser de plus de 4 px la déplace, un clic simple la déroule toujours.
- **Couleurs** : GS en blanc, **CP d'une seule couleur** (cyan `#78F2FB`, celui des pastilles CP de Shugo), au lieu de ambre ▲ / bleu ▼. La petite flèche ▲▼ reste, discrète, de la même couleur. GS et CP sont un peu plus gros (18 px dans la liste, 19 px repliée). Le message de Gio s'est coupé sur le choix de la couleur : à confirmer avec lui.
- Écart au cahier (5.2 : 12 lignes ; annexe C : CP ambre / bleu), voulu par Gio. Signalé au chef de suivi au prochain point.
- **Suite (Gio)** : le cyan lui va. **Flèches ▲▼ retirées** : « pas hyper intéressant ». La référence CP / GS de Gio reste calculée (paquets, sinon API), mais elle n'est plus affichée dans la bulle. Écart au cahier 4.5 et 5.1 (« flèche ▲▼ du CP par rapport à Gio »), voulu par Gio.
- **PvP** : Gio confirme s'être battu en Abysses. Aucun kill pendant la séance, donc **la source « Kill » n'est toujours pas vue en PvP réel**. Elle reste couverte par les tests unitaires, et la mort tuée par un joueur, elle, est vue.

## 2026-11-12 — Porte 5 validée, réponses du chef appliquées

- **Porte 5 validée** par le chef de suivi. `etape-5` est fusionnée dans `main` sans écrasement.
- `characterId` en cache : sur un 404, il est oublié et **une seule** recherche est refaite, dans la même opération. L'appelant reçoit `id_en_cache_invalide` pour l'effacer de la liste. Testé avec un faux serveur.
- **Appel API périodique sur le perso de Gio supprimé** : sans les flèches, il ne servait plus. Le CP et le GS de Gio restent lus dans les paquets, sans coût. Aucun test sur la flèche n'existait côté Rust (le calcul était dans le front, retiré avec les flèches).
- Cahier mis à jour (4.5, 5.1, 5.2, 5.4, 8, 11.2, annexe C) avec les choix de Gio validés : 5 lignes réglables, CP cyan, GS blanc, plus de flèches, position sous la mini-carte, 250 ms d'arrêt du curseur avant le survol.
- **Build** : la CI passe par `tauri build --no-bundle`, plus par `cargo` seul. Nouvelle vérification : `lastview.exe --verifier-pages` contrôle que `bulle.html` et `fiche.html` sont dans le binaire et que l'option `custom-protocol` est active (code 0, sinon 3, rapport dans `verifier-pages.txt`). La CI échoue si le contrôle échoue. **Preuve** : compilé exprès sans l'option, le contrôle renvoie 3 (« pages ABSENTES ») ; avec `tauri build`, il renvoie 0. Les Releases (section 9) passeront par `tauri build`.
- GitHub Actions : les deux derniers runs sur `etape-5` sont verts (f07064c en 4 min 47 s, 8f9e878 en 3 min 00 s). Le run sur `main` partira avec le push de la fusion.
- Captures de Gio : elles contiennent de vrais pseudos, donc jamais dans le repo. Pour illustrer le README, mode démo (`bulle.html?demo`).

## 2026-11-12 — Section 6, étape 1 : en-tête et onglet Équipement

- Feu vert du chef pour la section 6, en deux temps : d'abord l'onglet Équipement et ses infobulles, comparé à Shugo, puis les autres onglets.
- **Côté Rust** (`src-tauri/src/fiche.rs`) :
  - `fiche_ouvrir` : un clic sur « + » est une seule opération (recherche si besoin, puis profil et équipement ensemble). Le `characterId` est retenu sur la ligne de la liste, et effacé sur un 404.
  - `fiche_piece` : détail d'une pièce à la demande.
  - `ouvrir_shugo` : la page du perso dans le navigateur par défaut, adresses shugo.gg seulement.
  - Les appels réseau tournent hors du fil de l'interface.
- **Cache disque des images** (report de la section 4, `images.rs`) :
  - protocole `lvimg` (`http://lvimg.localhost/<url>`), fichiers dans `%APPDATA%\LastView\img` ;
  - 200 Mo au plus : les plus anciens partent d'abord, et un fichier relu est « rajeuni » ;
  - seuls les domaines d'images de NCSoft sont acceptés, et la CSP n'autorise plus que ce protocole pour les images.
- **Champs inconnus de l'API** (report) : chaque clé inconnue d'une réponse (profil, équipement, pièce…) est signalée une fois dans le journal. La liste des clés connues a été relevée sur les vraies réponses du 12/11.
- **Fenêtre** : position et taille mémorisées (500 ms après la fin du déplacement), restaurées si l'écran existe encore.
- **Mode test** : `lastview.exe --fiche Pseudo:serveur [--infobulle <emplacement>]` ouvre seulement la fiche, sans capture ni bulle, et sans prendre le focus. Il sert aux captures (`PrintWindow`, qui ne capture que la fenêtre de LastView).
- **Front** (`src/fiche.ts`, `styles/fiche.css`) :
  - valeurs de l'annexe B et maquette validée, Noto Sans embarquée (OFL) ;
  - en-tête : portrait, niveau, serveur, rencontre ou « Recherche », « Données API il y a … », titre, classe • race • iNiv, PC, 3 titres équipés ;
  - boutons ◀ ▶ (historique), Actualiser (désactivé si la donnée a moins de 30 s), Shugo.GG ↗, fermer ;
  - 4 onglets (pas d'Apparences) ;
  - Équipement : 6 sections, cartes grisées « Vide », badge ◆n de dépassement, couleurs de grade ;
  - squelettes animés pendant le chargement ;
  - erreurs claires (introuvable, pause API), sélecteur d'homonymes.
- **Infobulle de pièce** :
  - en-tête tout de suite, détail à l'arrivée ;
  - grade · catégorie, losanges de dépassement, niveau et niveau requis, classe ;
  - stats de base (bonus d'enchantement en vert, lignes de dépassement en orange, fourchette min - max des armes) ;
  - lien d'âme avec barre, stats additionnelles, compétences additionnelles (accessoires, arcanes), pierres de mana par grade, pierre divine avec description ;
  - set d'arcanes avec ses bonus (actifs en vert, inactifs en gris), sources, « Non échangeable » ;
  - toujours dans l'écran ; se ferme avec Échap, un clic ailleurs ou la croix.
- **Testé sur 3 persos** : Urattems (Aède), un Spiritualiste aux 5 arcanes vides, un Templier. 0 erreur du front, 0 champ inconnu.
- **Shugo** : sa page s'ouvre sur un bandeau de consentement aux cookies. Je ne l'accepte pas dans un navigateur automatique. Les captures Shugo de la comparaison sont demandées à Gio, dans son navigateur.
