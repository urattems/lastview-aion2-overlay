# API NCSoft — appels utilisés par LastView

API non documentée du site officiel (celle que shugo.gg utilise). Elle peut changer : parsing défensif obligatoire. Région **EU uniquement**, `lang=fr-FR`.

Référence de départ : annexe A du [cahier des charges](CAHIER_DES_CHARGES.md). Ce fichier sera complété à l'étape 0 (section 2.2) avec les résultats des tests directs depuis une app desktop.

**Repo public : aucun identifiant de personnage réel ni réponse brute non anonymisée ici.**

## Endpoints

| Usage | Requête |
|---|---|
| Recherche | `GET https://api-search.plaync.com/aion2global/search/v2/character?keyword={pseudo}&page=1&size=100&localeInfo=en-US&region=eu&serverId={id ou vide}` |
| Profil | `GET https://aion2.plaync.com/api/character/info?lang=fr-FR&characterId=…&serverId=…&region=eu` |
| Équipement | `GET https://aion2.plaync.com/api/character/equipment?lang=fr-FR&characterId=…&serverId=…&region=eu` |
| Détail pièce | `GET https://aion2.plaync.com/api/character/equipment/item?id={itemId}&enchantLevel={…}&characterId=…&serverId=…&slotPos={n}&lang=fr-FR&region=eu` |
| Plateau Daevanion | `GET https://aion2.plaync.com/api/character/daevanion/detail?lang=fr-FR&characterId=…&serverId=…&region=eu&boardId={id}` |
| Serveurs | `GET https://aion2.plaync.com/en-us/api/gameinfo/servers?lang=en-US&region=eu` |
| Classes | `GET https://aion2.plaync.com/en-us/api/gameinfo/classes?lang=en-US&region=eu` |

## À vérifier à l'étape 0 (2.2)

- [ ] En-têtes nécessaires depuis une app desktop (User-Agent, Referer…).
- [ ] Sémantique de `enchantLevel` dans `equipment/item` : enchant seul ou enchant + dépassement.
- [ ] Chargement des icônes (`assets.playnccdn.com`) et du portrait (`profileimg.plaync.com`, 302) depuis la webview Tauri.
- [ ] Comportement sur pseudo inexistant et sur homonymes.
- [ ] Champ d'état des stigmas (équipé / verrouillé).

## Règles d'usage

- Aucun appel tant que l'utilisateur ne clique pas sur « + » ou ne lance pas une recherche.
- File d'attente unique, 1 requête à la fois, au moins 1 s entre deux requêtes.
- 403 ou 429 : pause de 10 min. 404 : pas de nouvel essai pendant 1 h.
- Cache des réponses : 10 min.

## Exemples de réponses (anonymisés)

_(à remplir à l'étape 0)_
