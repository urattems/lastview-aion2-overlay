# Architecture de LastView

Section 3 du cahier des charges. Ce document décrit ce qui existe dans le code, et pourquoi.

## Vue d'ensemble

```text
 Npcap ─► capture ─────► assembleur TCP ─► découpage ─► décodage ─► Engine ─────────────► Store (SQLite)
 (copie   (un fil par     (un seul fil,     (trames,     (champ par   (mémoire silencieuse,     │
 du       interface,      remise en ordre,  lots LZ4)    champ)       détection des             │ liste-modifiee
 trafic)  filtre « tcp ») trous abandonnés)                           rencontres)               ▼
                                                                                         Fenêtre BULLE
            table TCP du système ──► verrouillage du flux du jeu                                │ clic +
            (connexions d'AION2.exe)  (signature en repli)                                       ▼
                                                                       ApiClient ◄──────── Fenêtre FICHE
                                                                       (file unique, cache)
```

Deux crates Rust dans un même workspace :

| Crate | Rôle | Dépend de Tauri ? |
|---|---|---|
| `crates/lastview-core` | Tout le travail : capture, protocole, mémoire, rencontres, base, API, journal. Testable et utilisable en console (`lastview-dump`). | non |
| `src-tauri` (`lastview`) | L'application : fenêtres, zone de notification, commandes appelées par le front, moteur en tâche de fond. | oui |

Choix : séparer le cœur de Tauri. Il compile en quelques secondes, il se teste sans fenêtre, et il permet de relire un enregistrement (`lastview-dump replay`) pour vérifier un patch du jeu sans lancer l'app.

## Modules de `lastview-core`

| Module | Contenu |
|---|---|
| `capture/` | `npcap.rs` : `wpcap.dll` chargée à l'exécution (pas de SDK). `system.rs` : processus du jeu, table TCP, premier plan, curseur, en lecture seule. `stream.rs` : réassemblage TCP en continu. `mod.rs` : fils de capture, verrouillage du flux, état (`AttenteJeu`, `AttenteTrafic`, `Active`, `NpcapManquant`). |
| `protocol/` | `framing.rs` : découpage du flux (repris d'A2Tools). `reader.rs` : lecteur champ par champ (varints, booléens groupés). **`opcodes.rs` : tous les opcodes, à un seul endroit, avec leur date de vérification.** `messages.rs` : décodeurs. |
| `world.rs` | Mémoire silencieuse : joueurs vus (id de session → pseudo, serveur, classe, niveau, GS, CP, légion), PNJ, joueur local, membres du groupe. Purge à 30 min. |
| `encounters.rs` | `Engine` : décode, tient la mémoire à jour, sort les rencontres (Mort, Kill, Combat, Ciblé, Groupe, Buff). Rencontres en attente quand l'id n'est pas encore nommé. Sources activables. |
| `store.rs` | SQLite dans `%APPDATA%\LastView\lastview.db` : `players`, `encounters`, `api_cache`, `settings`. Anti-spam 5 s, compteur de rencontres (10 min), rétention 4 h / 150 joueurs, épingles. |
| `api.rs` | Client NCSoft : file unique, 1 requête/s, pause 10 min sur 403/429, 1 h sur 404, cache 10 min, parsing défensif, homonymes. |
| `diagnostics.rs` | Journal `%APPDATA%\LastView\logs\lastview.log` (5 Mo, un ancien gardé), niveau détaillé en mode diagnostic. |
| `replay.rs` | Relecture d'un enregistrement de `tools/recorder`. |

## Verrouillage du flux du jeu (validé par le chef de suivi)

1. **Table TCP du système**, toutes les 2 s : connexions établies des processus `AION2.exe`, port 443 exclu. Un segment est retenu s'il vient d'une de ces connexions. Un relais local (loopback) est couvert par la même règle, puisque le jeu s'y connecte.
2. **Signature**, en repli seulement si le jeu tourne mais qu'aucune connexion n'est trouvée depuis 10 s : un flux candidat est verrouillé après 30 paquets aux opcodes typiques du jeu.

Les interfaces réseau sont relues toutes les 30 s : une interface qui apparaît (VPN, passage Wi-Fi ↔ Ethernet) est ouverte à son tour. Le paramètre « Interface réseau » limite la capture à une seule interface.

## Fils d'exécution

| Fil | Rôle |
|---|---|
| `lv-jeu` | Processus et connexions du jeu, toutes les 2 s |
| `lv-interfaces` | Ouverture des interfaces Npcap, toutes les 30 s |
| `lv-capture` (un par interface) | Lecture Npcap, filtre BPF `tcp`, segments vers l'assembleur |
| `lv-assemblage` | Remise en ordre, découpage, paquets vers le moteur |
| `lv-moteur` (app) | `Engine` + `Store`, événements vers les fenêtres, purge chaque minute |

Le fil de l'interface ne fait aucun travail lourd. Les appels API se font hors du fil du moteur.

## Contrat front ↔ Rust

Commandes : `liste` (lignes de la bulle), `etat` (capture, joueurs en mémoire, joueur local). Événements : `liste-modifiee`, `etat`. Types côté TypeScript : `src/lib/api.ts`. D'autres commandes viendront avec la bulle (section 5), la fiche (6), la recherche (7) et les paramètres (8).

## Fenêtres

| Fenêtre | Configuration actuelle | À faire |
|---|---|---|
| `bulle` | sans bordure, transparente, toujours au-dessus, hors barre des tâches, cachée au démarrage, `focus: false` | états replié / déroulé, survol au curseur, clics traversants, visibilité selon le premier plan (section 5) |
| `fiche` | fenêtre normale, cachée ; la fermer la cache | contenu façon shugo.gg (section 6) |
| zone de notification | Afficher la bulle, Ouvrir la fiche, Mode diagnostic, Quitter | état de la capture, recherche, paramètres (section 9.4) |

## Sécurité

- CSP stricte : scripts et styles locaux, images seulement depuis les domaines d'images NCSoft.
- Droits Tauri : `core:default` seulement, pour les deux fenêtres.
- Aucun pseudo n'est injecté comme HTML (`textContent`, balises de la recherche retirées).
