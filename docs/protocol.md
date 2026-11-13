# Protocole réseau Aion 2 — ce qu'on sait

Tout ce qui est connu des paquets utilisés par LastView : opcodes, structures, position des champs, et **date de vérification**. Seul le sens serveur → client est lisible (le sens client → serveur est chiffré).

Source principale : [A2Tools DPS Meter](https://github.com/taengu/A2Tools-DPS-Meter) v2.0.96 (10/11/2026), `src-tauri/src/capture/stream_processor.rs`, `framing.rs`, `abnormal.rs`, `combat/death_recap.rs`. Les dates entre parenthèses sont celles des commentaires A2Tools.

## Transport

- TCP, serveur du jeu sur le port **13328** observé (193.202.112.96 pour Vaizel le 10/11/2026). Le client (`AION2.exe`, fenêtre titrée `AION2`) a aussi des connexions HTTPS (443) et une paire loopback 127.0.0.1 : à ignorer.
- Deux processus `AION2.exe` tournent : un parent sans fenêtre et le client. Les connexions sont sur le client.
- **Cadrage du flux** (`framing.rs`) : `00` = bourrage ; `17 03 03 <len u16>` = enregistrement TLS d'une autre connexion, à sauter ; `<varint len> <payload>` = un paquet, la trame occupe `len − 4 + (octets du varint)` ; `<varint len> FF FF <u32 size LE> <lz4 block>` = un lot compressé de paquets (profondeur ≤ 4), à parcourir avec le même cadrage.
- Varint : LEB128 non signé, 7 bits par octet, ≤ 5 octets.
- Un **opcode** = les 2 premiers octets du payload. 99 % des paquets ont un second octet `36`, `37`, `38` ou `8D`.
- Vérifié le 10/11/2026 avec `lv-recorder` : 290 segments, 1 550 paquets décodés, 0 octet non consommé.

## Structures exactes (11–12/11/2026)

Les paquets ne sont plus lus par motifs approximatifs, comme A2Tools le fait, mais champ par champ. La colonne « Statut » dit ce qui a été vérifié sur nos captures. Conventions de lecture :

- `vi32` = varint (LEB128) ; `u8`, `u16`, `i32`, `i64` = petit-boutiste ; `str` = longueur en varint puis UTF-8, un `00` final éventuel retiré.
- `bool` : les booléens sont **regroupés bit par bit** dans un même octet. Le premier `bool` lu réserve un octet, les 7 suivants en lisent les autres bits.
- Beaucoup de messages ont un octet ou un `u32` d'**indicateurs** : un champ marqué `[bit n]` n'est présent que si ce bit vaut 1.

| Opcode | Rôle | Champs, dans l'ordre | Statut |
|---|---|---|---|
| `34 38` | **Ta cible** (réponse à ton ciblage) | `indicateurs u8`, `résultat u16` (0 = OK), `cible vi32` (0 = plus de cible), `[bit 0] partie i32` | vérifié sur nos captures |
| `35 38` | **Cible d'une entité** : « X cible Y » | `X vi32`, `indicateurs u8`, `Y vi32`, `[bit 0] partie i32`. Sert à la cible de ta cible. | vérifié sur nos captures |
| `04 8D` | **Mort** d'une entité | `victime vi32`, `compétence i32`, `tueur vi32`, `serveur du tueur u16`, `pseudo du tueur str`, `légion du tueur str`, `emblème u16`, `pnj tueur vi32`, `action i32`, `raison u8`, `type u8` | vérifié sur nos captures |
| `45 36` | **Apparition d'un joueur** | `id vi32`, `indicateurs u32`, `desc` (pseudo, classe…), `état`, `[bit 0] serveur u16`, `[bit 1] légion`, `effets list`, `équipement list`, … `dbid i64`, `stats list`, `niveau i32`, `niveau de conquérant i32`, `GS i32`, … `[bit 24] rang abyssal i32`, … `CP i64`, `type de mort u8`, `bool` | vérifié sur nos captures |
| `04 38` | **Effet de compétence** (dégâts, soins) | `cible vi32`, `indicateurs u16`, `lanceur vi32`, `compétence i32`, … **Indicateur bit 1 = bloc « détail des dégâts »** : présent sur les coups (`6`), absent sur les soins et buffs (`4`) | vérifié sur nos captures (90 269 lus) |
| `2A 38` | **Ajout d'effet** (buff, débuff) | `entité vi32`, `effets list`, chaque effet portant `[bit 0] lanceur vi32` | vérifié sur nos captures |
| `33 36` | **Ta fiche**, au chargement de zone | `id vi32`, `indicateurs u32`, puis : `indicateurs u8`, `[bit 0] pseudo str`, `serveur u16`, `classe i32`, `[bit 1] race u8`, `niveau i32`, `GS i32`, `GS max i32`, … Plus loin : `dbid i64`, `[bit 22] dernière mort` (`date i64`, `serveur du tueur u16`, `pseudo du tueur str`, `monstre i32`, `type u8`), `CP i64`, `CP max i64`. LastView n'a besoin que du début : il connaît ainsi ton id pour la session. | **vérifié le 12/11/2026** : 9/9 lus en entier, CP lu = CP de l'API (159 717) |
| `47 36` | Disparition d'un joueur (sortie de zone) | `id vi32`, `raison u8`, `effet vi32` | — |
| `41 36` / `42 36` | Apparition / disparition de PNJ | monstres et PNJ | — |
| `91 8D` | Changement de niveau | `id vi32`, `niveau i32` | — |
| `33 8A` | Changement de légion | `id vi32`, `légion` | — |
| `1C 56` | Changement d'équipement | avec le nouveau GS | — |
| `56 36` | Ton CP | `CP actuel i64`, `CP max i64` (16 octets). Vu 3 fois le 11/11 : actuel 118 046 → 159 222 dans une même session, max fixe à 211 025 ; l'actuel est proche de l'API (159 786). Sert de référence aux flèches ▲▼. | vu en capture, sens à confirmer |
| `00 92` | **Ton groupe complet** | `indicateurs u8`, `clé du groupe i64`, `chef dbid i64`, `taille max i32`, `u8 ×4` (partage du butin, type), `[bit 0] cible du groupe`, `[bit 1] marqueurs list`, `[bit 2] marqueurs de carte list`, `[bit 3] list`, **`membres list`** (voir « membre » ci-dessous), `[bit 4] repères list`, puis `raison u8` | **vérifié le 12/11/2026** (1/1) |
| `0D 92` | **Un joueur rejoint le groupe** | un « membre » | non vérifié en capture, à confirmer au premier usage réel |
| `22 92` | Un membre se reconnecte | un « membre » | non vérifié en capture, à confirmer au premier usage réel |
| `1C 92` | Mise à jour d'un membre | `dbid i64`, `classe i32`, `carte i64`, `sous-zone i32`, `position 3×f32`, `niveau i32`, `niveau de conquérant i32`, `GS i32`, `légion str`, `emblème i32`, `bool`, `CP i64`, `u8` | **vérifié le 12/11/2026** (1 628/1 628). Le `dbid` est aussi dans l'apparition `45 36` : un groupe formé avant le lancement de LastView est reconnu ainsi (4/4 membres) |
| `1B 92` | PV / PM d'un membre | inutile pour LastView | — |
| `11 92` | Nouveau chef de groupe | `chef dbid i64` | non vérifié en capture |
| `19 92` | Tu es exclu du groupe | `bool` (par vote) | non vérifié en capture |
| `13 92` | Réponse quand tu quittes le groupe | `résultat u16`. C'est le `13 92 00 00` d'A2Tools | non vérifié en capture |
| `1F 92` | Groupe dissous | `raison u8` | non vérifié en capture |
| `1F 93` | Départ du groupe | `clé du groupe i64`, `raison u8` | **vérifié le 12/11/2026** (1/1) |
| `02 96` / `0A 96` | Alliance : infos / nouveau membre | même idée, avec plusieurs groupes. Le membre d'alliance a les mêmes champs utiles dans un autre ordre. | non vérifié en capture |
| `02 97` | **Recherche de groupe**, pas ton groupe | A2Tools l'appelait « roster » | — |

**Membre de groupe** (dans `00 92`, `0D 92`, `22 92`), non vérifié en capture : `indicateurs u8`, `numéro u8`, **`id i32`** (même id d'entité que les autres paquets, en entier fixe ici), **`serveur d'origine u16`**, `serveur actuel u16`, `compte str`, `dbid i64`, **`pseudo str`**, **`classe i32`**, `PV vi64`, `PV max vi64`, `PM, PA, PD` (6 × i32), `carte i64`, `sous-zone i32`, `position 3×f32`, `bool en vie`, **`[bit 0] légion str`**, `[bit 1] emblème i32`, **`niveau i32`**, `niveau de conquérant i32`, **`GS i32`**, `bool`, `bool`, `[bit 2] compétences list`, **`CP i64`**, `u8`.

Un membre de groupe arrive donc **avec son pseudo, son serveur et son id**, même s'il était déjà à l'écran avant le lancement de LastView.

Conséquences pour LastView :

- **Mort** et **Kill** : `04 8D` donne directement le pseudo, le serveur et la légion du tueur. Pas besoin de l'avoir vu apparaître.
- **Serveur** : `45 36` le porte dans toutes les apparitions vues (106/106), pas seulement pour les joueurs en légion comme le supposait A2Tools.
- **Joueur déjà à l'écran au lancement** : sans `45 36` reçu, on a son id mais pas son pseudo. C'est une limite de toute capture passive. LastView démarre donc avec Windows, avant le jeu.
- **Paquets non vérifiés en capture** (`33 36`, groupe) : décodage tolérant. Un paquet qui ne se lit pas jusqu'au dernier octet est ignoré sans planter et signalé dans les logs de diagnostic.

### Vérifications sur nos captures (11/11/2026)

Décodeur de test écrit pour LastView (Python, hors repo), lecture stricte champ par champ. Captures `ciblage1`, `ciblage3`, `ciblage4`.

| Paquet | Lus jusqu'au dernier octet | Remarque |
|---|---|---|
| `45 36` apparition de joueur | 106 / 106 | **le serveur est présent dans les 106** ; légion dans 71 |
| `04 8D` mort | 45 / 45 | pseudo, serveur et légion du tueur |
| `34 38` ta cible | 35 / 35 | |
| `35 38` cible d'une entité | 36 / 36 | |
| `04 38` effet de compétence | 2 702 / 2 702 | aucun effet d'un joueur sur un autre joueur : les 1 066 effets entre joueurs nommés sont lancés sur soi-même (corrigé le 12/11/2026) |
| `2A 38` ajout d'effet | 213 / 213 | 18 posés par un autre joueur : la source « Buff » est faisable |

Comparaison avec l'API NCSoft (classe, niveau, GS = stat `ItemLevel`, CP, serveur), sur 3 joueurs capturés le soir même (anonymisés) :

| Joueur | Paquet `45 36` | API, 30 min plus tard | Résultat |
|---|---|---|---|
| Joueur A | Templier, 45, Vaizel | identique | identique |
| Joueur B | Rôdeur, 45, Vaizel | identique | identique |
| Joueur C | Rôdeur, 45, Vaizel | CP différent de 7 points | CP à 7 près, le reste identique |

Sur 3 joueurs capturés la veille, classe, niveau et serveur sont identiques. GS et CP ont monté ensemble entre-temps, comme après une amélioration d'équipement (un des trois identique en tout).

**Premier PvP réel (séance de la porte 5, 12/11/2026)** : 520 643 paquets relus, 0 illisible. Résultat avant correction : un membre du groupe (un Clerc) compté en « Combat » à cause de ses soins. **Faux positif corrigé** : Combat exige le bloc de dégâts (bit 1). Après correction : 9 joueurs adverses (un autre serveur) en « Combat », tous avec des coups portant le bloc de dégâts, aucun membre du groupe, et une « Mort » tuée par un joueur adverse. Manque possible : des dégâts sur la durée sans bloc de détail ne déclenchent pas « Combat » ; un échange de coups en contient toujours. Test : `non_regression::porte5_groupe_fiche_pvp`.

**Règle Combat (chef de suivi, 12/11/2026)** : une rencontre « Combat » n'est créée que pour des dégâts entre ton entité et un joueur identifié par son apparition `45 36`. Exclus : effets sur soi, familiers et invocations (ce sont des PNJ, `41 36`), monstres, entités inconnues (jamais mises en attente), membres de groupe jamais vus par `45 36`. Tests : `encounters::tests::exclusions_de_combat`. **Au premier PvP réel : vérifier qu'il n'y a ni faux positif ni manque, et noter le résultat ici.**

**Ta fiche `33 36` en entier** : LastView lit maintenant tout le paquet pour obtenir ton CP (`CP i64`, `CP max i64`, après les infos de conquérant). Si la fin ne se lit pas jusqu'au dernier octet, on garde le début (id, pseudo, GS) et le CP reste inconnu. Non vérifié en capture.

**Référence CP / GS des flèches ▲▼** : paquets (`33 36`, `56 36`), sinon API sur « Mon perso / Mon serveur » au démarrage puis toutes les 30 min, sinon pas de flèche. La source est donnée dans l'état de l'app (`reference.source`).

**PvP réel** : `ciblage1` contient 38 morts dont le tueur est un joueur, avec son pseudo, son serveur et sa légion. Les sources « Kill » et « Mort » sont vérifiées sur du vrai PvP. **« Combat » ne l'est pas** : aucun dégât d'un joueur sur un autre dans nos captures. La structure de `04 38` est la même pour toutes les entités (2 702 lus), mais le premier usage réel en PvP reste à observer.

**Ton id (joueur local)** : `33 36` le donne, mais il n'est pas encore vu en capture. Repli étudié : l'entité de `4A 36` (changement de stats). Elle est unique dans `ciblage1` et `ciblage4`, mais deux entités apparaissent dans `ciblage3`. Le repli est donc marqué « non confirmé » et sert seulement tant que `33 36` n'est pas reçu.

`33 36` et les paquets de groupe (`00 92`, `1C 92`, `1F 93`) : **vus et vérifiés le 12/11/2026** (séance de la porte 5). Restent non vus : `0D 92`, `22 92`, `11 92`, `13 92`, `19 92`, `1F 92`, alliance (`02 96`, `0A 96`).

## Paquets utiles à LastView

| Usage | Opcode | Structure | Vérifié | Statut |
|---|---|---|---|---|
| **Joueur local** (self record) | `33 36` | `<id varint> <mask1 u32> <mask2 u8> [mask2 & 1 : <len u8> <utf8 nom>]` puis `<classe u32> … <niveau u32> … <GS u32> ×2 … <CP u64>`. Envoyé à chaque chargement de zone puis toutes les quelques minutes, souvent dans un lot imbriqué. Un seul entité a ce record : c'est Gio. | A2Tools 10/2026 ; structure exacte plus haut (12/11/2026) | non vérifié en capture, à confirmer au premier usage réel |
| **Spawn joueur** | `45 36` (anciennement `44 36`, décalage +1 de juin 2026) | même en-tête masqué (id, nom) ; après le nom : classe `u32` (valeurs du roster), `48 f4 ff ff <niveau u32> 00 00 00 00 <GS u32>`, légion `<id u16> 00 00 00 00 <serveur u16> <len u8> <nom>` (~45 % des joueurs), CP `u64` près de la fin après `f4 cb 1f <u32> <n u32>`. Validé par A2Tools sur 512 records / 354 joueurs (28/10/2026). Se trouve aussi **au milieu** d'autres paquets (scan `embedded_spawns`). | A2Tools 10/2026 | à confirmer en jeu |
| **Mort** | `42 36` (anciennement `41 36`) | `<id varint> <varint 0> <flag varint>` : flag 1 = chargé mort, **3 = mort en combat**, 7 = disparu du monde (despawn). | A2Tools 06/2026 | à confirmer |
| **Kill / tué par** | `04 8D` | `<id victime varint> <4 octets> <id tueur varint> <serveur u16 LE> <len u8> <nom tueur> [<len u8> <légion>]`. Si la victime est Gio : « tué par » (nom, serveur, légion du tueur). Sinon : un autre joueur tué par `owner` (kill ou assist). Trouvé n'importe où dans un paquet. | A2Tools 10/2026 | à confirmer |
| **Dégâts** | `04 38` | `<cible varint> <switch varint> <flag varint> <acteur varint> <skill u32> <uid u8> <type varint> …` ; entrées chaînées `01 00`. Skill à 7 chiffres = PNJ. Sert à « Combat » (Gio ↔ joueur). | A2Tools 10/2026 | à confirmer |
| **Roster de groupe** | `02 97` | `party_key u32, nom str, taille u8, donjon u32, u8 u8, leader u64, u8×3, count varint, membres : mask u8, slot u8, dbid u64, pseudo str, u32, niveau u32, GS u32, serveur u16 u16, u8, CP u64, u16 u8`. Envoyé à chaque changement de groupe. | A2Tools 10/2026 | **corrigé 12/11/2026** : c'est la recherche de groupe. La source « Groupe » repose sur `00 92`, `0D 92`, `22 92` et `1C 92` (voir plus haut). |
| **Groupe quitté** | `13 92 00 00` (monde ouvert), `1D 97 00 00` (donjon/raid) | — | A2Tools 10/2026 | — |
| **Buff reçu** | `2A 38` | `<entité> 01 <flags u8> <instance> <abnormal u32> <length i64> <end i64> <caster> <level u8> [flags & 2 : <skill u32>] <u8> <x f32> <y f32> <z f32>`. `caster` = qui a lancé : si `entité` = Gio et `caster` = un autre joueur → source « Buff ». 336 702 records lus par A2Tools sans erreur. | A2Tools 10/2026 | **faisable** |
| Changement de zone | `23 36` | `<id varint>` ; id 0 = Gio téléporté. | A2Tools | — |
| Chargement de carte | `21 36` | `<u32 count> <u32 map id> …`, 52 octets. | A2Tools 10/2026 | — |
| Changement de stats (soi) | `4A 36` | `<entité> <n u8> (<stat u16> <value i32>)*` | A2Tools | inutile pour la V1 |
| **Ciblage** | **`34 38`** et **`35 38`** | `34 38 00 00 00 <id cible varint>` ; `35 38 <id cible varint> 00 00`. Envoyés ensemble, 0,4 à 1,3 s après le clic. `34 38 00 00 00 00` (id 0) = plus de cible. Voir le verdict ci-dessous. | **11/11/2026** (LastView, 2 essais) | **trouvé** |

Notes :
- L'id d'entité est de session : il change à chaque connexion et les ids sont réutilisés.
- Les noms font 1 à 12 lettres ou chiffres (A2Tools `sanitize_nickname`), champ de 1 à 36 octets.
- Les serveurs connus (ids 1000–2999) sont dans `capture/servers.rs` d'A2Tools.

## Verdict ciblage (étape 0, section 2.3)

**Trouvé, le 11/11/2026.** Le ciblage est détectable passivement : c'est la source principale de la section 4.6, pas besoin du plan B (OCR).

Méthode : `tools/recorder` avec métronome (12 bips toutes les 5 s, aigu = cibler A, grave = cibler B, après 20 s de référence sans cibler). Le jeu tourne en administrateur, donc les clics ne sont pas lisibles : l'heure des bips sert de référence.

| Essai | `35 38` après les bips | `34 38` après les bips | Id qui change à chaque bip | Hors des fenêtres | Pendant la référence |
|---|---|---|---|---|---|
| `ciblage3` | 11/12 | 11/12 | 100 % | 0,02 /s | 0 |
| `ciblage4` | 12/12 | 11/12 | 100 % | 0,00 /s | 0 |
| Contrôle : bips au hasard sur `ciblage1` | — | — | aucun opcode rare au-dessus du bruit | — | — |

Dans `ciblage4`, l'id alterne entre deux entités (12106 et 6350), au rythme des bips. Une fois, `34 38` porte l'id 0 : la cible a été perdue.

Structure :

```text
<len> 34 38 00 00 00 <id cible varint>     nouvelle cible (id 0 = aucune cible)
<len> 35 38 <id cible varint> 00 00        idem, envoyé dans la même trame ou juste après
```

À savoir pour l'implémentation :
- Les octets nuls de `34 38` sont peut-être un id de source (0 = soi) suivi d'indicateurs : à surveiller si d'autres valeurs apparaissent.
- A2Tools voit passer `35 38` à la mort d'un monstre combattu : quand la cible meurt, le jeu la retire. Les monstres sont aussi des cibles. **LastView ne retient que les ids présents dans la mémoire silencieuse comme joueurs** (spawn `45 36`).
- Un joueur déjà à l'écran au lancement de LastView n'a pas encore de spawn dans la capture : on a son id, pas son nom. Il sera connu au prochain chargement de zone ou à sa réapparition. En attendant, la rencontre est mise en attente, puis complétée dès que l'id est nommé.
- Le délai serveur est court (< 1,5 s), compatible avec l'objectif « en haut de la liste en moins de 2 s ».

Autre résultat du même test : **le curseur reste lisible quand le jeu a le focus.** `GetCursorInfo` voit les bascules visible/caché quand Gio tient Alt (29 et 17 bascules relevées). Le mécanisme de survol de la bulle (5.3) fonctionne sans droits admin. Par contre, `GetAsyncKeyState` ne voit ni les clics ni les touches tant que LastView n'est pas lui aussi lancé en administrateur. Les raccourcis globaux (section 8) doivent passer par `RegisterHotKey`, à vérifier en jeu à la section 5.

## Reporter une correction après un patch

Tous les opcodes et offsets de LastView seront centralisés dans un seul module (`protocol/opcodes.rs`), chacun avec sa date de vérification. Procédure :
1. Vérifier les commits récents d'A2Tools (`git -C ../A2Tools-DPS-Meter pull` puis `git log --oneline -20`) : ils corrigent généralement sous 48 h.
2. Enregistrer 2 minutes de trafic avec `lv-recorder record` et regarder `report.md` (fréquence des opcodes) : un décalage de +1 de la famille `36` se voit tout de suite.
3. Corriger `opcodes.rs`, relancer les tests unitaires du parser, mettre à jour ce fichier.

## CP normalisé (Abysses normalisées) — règle du 13/11/2026

Dans les Abysses normalisées, tous les joueurs reçoivent le CP maximum du jeu. L'API NCSoft a ainsi gardé en cache le CP d'Urattems à 393 117, contre environ 162 000 en réalité, avec un iNiv inchangé (2 915).

Règle appliquée (`crates/lastview-core/src/cp.rs`, `Store::observer_cp`) :
- **normalisé** si le CP augmente de plus de 40 % alors que l'iNiv varie de moins de 2 %, par rapport au dernier CP réel connu du même joueur (paquets ou API) ;
- **ou** s'il est égal au plafond, appris dès que 3 joueurs différents remontent exactement le même CP (réglage `cp_plafond` en base, il peut changer avec un patch).

Un CP normalisé n'écrase jamais un CP réel :
- la bulle montre le dernier CP réel avec un petit « N », ou « — » s'il n'y en a aucun ;
- la fiche montre le dernier CP réel, avec la mention « CP de l'API normalisé (Abysses) : … ».

**À vérifier à la prochaine capture en Abysses normalisées** (pas encore fait) :
- `45 36` (apparition) et `33 36` (ta fiche) donnent-ils le CP normalisé ?
- `56 36` « CP max » (sens non confirmé) est-il justement ce plafond ? Le 11/11, il valait 211 025 hors Abysses normalisées.

Noter ici le résultat.
