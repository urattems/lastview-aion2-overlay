# API NCSoft — appels utilisés par LastView

API non documentée du site officiel (celle que shugo.gg utilise). Elle peut changer : parsing défensif obligatoire. Région **EU uniquement**, `lang=fr-FR`.

**Vérifié le 10/11/2026** sur Urattems (Vaizel, 1303), directement depuis le PC avec `curl` (étape 0, section 2.2).

**Repo public : aucune réponse brute non anonymisée ici.** Les identifiants ci-dessous sont ceux de Gio, qui a accepté qu'ils figurent dans le cahier des charges.

## Résultats de l'étape 0

| Question | Réponse |
|---|---|
| En-têtes nécessaires depuis une app desktop ? | **Aucun.** Un `curl` nu (User-Agent `curl/x`, pas de Referer, pas de cookie) reçoit du JSON `200` sur tous les endpoints. |
| `enchantLevel` de `equipment/item` | **Enchant + dépassement.** Avec `15` pour une arme +15 ◆2, l'API renvoie les stats de base avec le bonus d'enchant (`extra`) mais **aucune ligne de dépassement**. Avec `17`, elle ajoute les lignes `"exceed": true` (ATQ +24, Boost de DGT +2 %). → envoyer `enchantLevel + exceedLevel`. |
| Icônes (`assets.playnccdn.com`) | `200 image/png`, `Access-Control-Allow-Origin: *`, `Cache-Control: max-age=356653`. Chargeables directement par `<img>` dans la webview. Cache disque côté Rust quand même (hors ligne). |
| Portrait (`profileimg.plaync.com`) | `302` vers `fizz-download.playnccdn.com/...` puis `200 image/jpeg` (~15 Ko). Une balise `<img>` suit la redirection. |
| Pseudo inexistant | Recherche : `200` avec `list: []`, `pagination.total: 0`. `info` avec un characterId invalide : `404 application/problem+json` (`code: resource.not_found`). |
| Homonymes | **La recherche est floue** (préfixe / sous-chaîne, insensible à la casse) : « Kira » → 2 961 résultats dont 45 « Kira » exacts sur des serveurs différents ; « Ura » → 10 000. **Filtrer côté client sur le nom exact** (sans les balises `<strong>`), puis par `serverId` (ou le passer dans l'URL : « Kira » + 1303 → 58 résultats, 1 exact). Un pseudo est unique par serveur. |
| Stigmas | Dans `equipment.skill.skillList`, `category` vaut `Active`, `Passive` ou **`Dp`** (= stigma). Champs : `acquired` (0/1), `equip` (0/1), `skillLevel`, `needLevel`. Un stigma non acquis a `acquired: 0`. Les actives ont `equip` 0/1 ; les passives `equip: 0`. |
| `characterId` | Base64 avec `=` final. La recherche le renvoie **déjà encodé** (`%3D`), `info` le renvoie brut (`=`). Normaliser (décoder puis encoder une fois). |
| Pagination de la recherche | `page`, `size` (100 max observé), `total`, `endPage`. |

## Endpoints

| Usage | Requête |
|---|---|
| Recherche | `GET https://api-search.plaync.com/aion2global/search/v2/character?keyword={pseudo}&page=1&size=100&localeInfo=en-US&region=eu&serverId={id ou vide}` |
| Profil | `GET https://aion2.plaync.com/api/character/info?lang=fr-FR&characterId=…&serverId=…&region=eu` |
| Équipement + compétences | `GET https://aion2.plaync.com/api/character/equipment?lang=fr-FR&characterId=…&serverId=…&region=eu` |
| Détail pièce | `GET https://aion2.plaync.com/api/character/equipment/item?id={itemId}&enchantLevel={enchant+exceed}&characterId=…&serverId=…&slotPos={n}&lang=fr-FR&region=eu` |
| Plateau Daevanion | `GET https://aion2.plaync.com/api/character/daevanion/detail?lang=fr-FR&characterId=…&serverId=…&region=eu&boardId={id}` |
| Serveurs | `GET https://aion2.plaync.com/en-us/api/gameinfo/servers?lang=en-US&region=eu` |
| Classes | `GET https://aion2.plaync.com/en-us/api/gameinfo/classes?lang=en-US&region=eu` |

## Structures observées (champs utilisés)

### Recherche
`list[]` : `characterId` (encodé), `name` (avec `<strong>…</strong>` autour de la partie trouvée), `race` (1 Élyséen, 2 Asmodien), `pcId`, `level`, `serverId`, `serverName`, `profileImageUrl` (chemin relatif), `region`. `pagination` : `page`, `size`, `total`, `endPage`.

### `info`
- `profile` : `characterId`, `characterName`, `characterLevel`, `className`, `pcId`, `raceId`, `raceName`, `regionName`, `serverId`, `serverName`, `gender`, `genderName`, `combatPower`, `titleId`, `titleName`, `titleGrade`, `profileImage` (URL absolue).
- `stat.statList[]` (17 entrées) : `name`, `type` (`STR`, `DEX`, …), `value`, `statSecondList[]` (chaînes, ex. « Augmentation de l'ATQ +0.8% »).
- `title` : `ownedCount`, `totalCount`, `titleList[]` : `id`, `name`, `grade`, `equipCategory` (`Etc`, …), `statList[]` (`desc`), `equipStatList[]` (`desc`), `ownedCount`, `ownedPercent`, `totalCount`.
- `daevanion.boardList[]` : `id` (81 Nezekan, 82 Zikel, 83 Vaizel, 84 Triniel, …), `name`, `icon`, `open`, `openNodeCount`, `totalNodeCount` (88 ou 116), `openPercent`.
- `ranking.rankingList` : `null` sur Urattems.

### `equipment`
- `equipment.equipmentList[]` : `id`, `name`, `grade` (`Unique`, `Legend`, `Special`), `enchantLevel`, `exceedLevel`, `icon`, `slotPos`, `slotPosName`.
- `equipment.skinList[]` : ignoré.
- `petwing.pet` : `id`, `name`, `level`, `icon`. `petwing.wing` et `petwing.wingSkin` : `id`, `name`, `grade`, `enchantLevel`, `icon`.
- `skill.skillList[]` (35 entrées) : `id`, `name`, `icon`, `category` (`Active` / `Passive` / `Dp`), `skillLevel`, `acquired`, `equip`, `needLevel`.

Emplacements vus : MainHand 1, SubHand 2, Helmet 3, Shoulder 4, Torso 5, Pants 6, Gloves 7, Boots 8, Necklace 10, Earring1 11, Earring2 12, Ring1 13, Ring2 14, Bracelet1 15, Bracelet2 16, Belt 17, Cape 19, Amulet 22, Rune1 23, Rune2 24, Arcana1 à 5 = 41 à 45.

### `equipment/item`
`id`, `name`, `grade`, `gradeName`, `icon`, `level`, `levelValue`, `equipLevel`, `enchantLevel`, `maxEnchantLevel` (15), `maxExceedEnchantLevel` (5), `categoryName`, `classNames[]`, `raceName`, `type` (`Equip`, `Accessory`), `soulBindRate` (chaîne, « 100 »), `tradable`, `tradablePersonal`, `storable`, `enchantable`, `decomposable`, `favorite`, `costumes[]`, `sources[]` (chaînes),
`mainStats[]` : `id`, `name`, `value`, `extra` (bonus d'enchant), `minValue` (arme), `exceed` (bool),
`subStats[]` : `id`, `name`, `value` ; `subStatCount`, `subSkillCountMax`, `subStatRandom`,
`magicStoneStat[]` : `id`, `name`, `value`, `grade`, `icon`, `slotPos` ; `magicStoneSlotCount`,
`godStoneStat[]` : `name`, `desc`, `grade`, `icon`, `slotPos` ; `godStoneSlotCount`.

### `daevanion/detail`
`nodeList[]` (225 = 15 × 15) : `boardId`, `row`, `col`, `nodeId`, `name`, `icon`, `grade` (`None`, `Common`, …), `type` (`None`, `Stat`, …), `open` (0/1), `effectList[]` (`desc`). Les cases vides ont `type: "None"` et `name: ""`.
`openSkillEffectList[]`, `openStatEffectList[]` : `desc`.

### `gameinfo/servers`
`serverList[]` : `raceId`, `serverId`, `serverName`, `serverShortName`. EU : 1301 Siel, 1302 Nezekan, 1303 Vaizel, 1304 Kaisinel, 1305 Yustiel, 1306 Ariel, …

### `gameinfo/classes`
`classList[]` : `id` (pcId), `name`, `text`. Vus : 2 Gladiator, 3 Templar, 4 Ranger, 5 Assassin, 6 Spiritmaster, 7 Sorcerer, 8 Cleric, 9 Chanter, … (34 = Aède / Bard d'après le profil d'Urattems).

## Règles d'usage

- Aucun appel tant que l'utilisateur ne clique pas sur « + » ou ne lance pas une recherche.
- File d'attente unique, 1 requête à la fois, au moins 1 s entre deux requêtes.
- 403 ou 429 : pause de 10 min. 404 : pas de nouvel essai pendant 1 h.
- Cache des réponses : 10 min.
- Nom de recherche : retirer les balises HTML avant affichage, jamais d'injection dans la page.
