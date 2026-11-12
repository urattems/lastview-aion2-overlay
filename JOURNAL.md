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
- **PvP déjà vérifié** : `ciblage1` contient 38 morts tuées par un joueur et plus de 1 000 effets entre joueurs nommés. Combat, kill et mort sont validés sur du vrai PvP.
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
