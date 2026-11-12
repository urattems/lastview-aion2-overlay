# lv-recorder — outil de l'étape 0

Enregistre le trafic **serveur → client** d'Aion 2 et l'horodatage des clics souris, puis cherche le paquet qui suit chaque clic de ciblage (cahier des charges, section 2.3).

100 % passif : copie Npcap du trafic des connexions TCP du processus `AION2.exe` (lues dans la table TCP du système), sondage de la souris en lecture seule (`GetAsyncKeyState`, `GetCursorInfo`). Aucune lecture mémoire, aucune injection, aucun paquet envoyé.

## Compiler

```
cargo build --release
```

Prérequis : Rust stable MSVC, Npcap installé (aucun SDK : `wpcap.dll` est chargée à l'exécution).

## Utiliser

```
lv-recorder devices                       # liste les interfaces vues par Npcap
lv-recorder record  <dossier>             # enregistre, guidé par le métronome, s'arrête seul
lv-recorder record  <dossier> --sans-bips # enregistre jusqu'à Ctrl+C, sans métronome
lv-recorder analyze <dossier>             # produit <dossier>/report.md
lv-recorder analyze <dossier> --window-ms 2000
lv-recorder dump    <dossier>             # packets.tsv + names.tsv pour une analyse ad hoc
```

**Métronome** (par défaut) : 30 s de préparation, 3 bips courts, 20 s de référence sans cibler, 12 bips toutes les 5 s (aigu = cibler A, grave = cibler B), 2 bips longs, arrêt automatique. L'analyse prend l'heure des bips comme référence (fenêtre de 3 s pour le temps de réaction) et cherche un opcode qui porte l'id de A après les bips aigus et celui de B après les bips graves. Sur des bips placés au hasard, le meilleur score plafonne vers 40 % ; un vrai paquet de ciblage doit dépasser 75 %.

**Pourquoi** : le client `AION2.exe` est protégé (son chemin et sa ligne de commande sont illisibles depuis un processus normal). Les clics et les touches faits pendant que le jeu a le focus ne sont pas vus par `GetAsyncKeyState` d'un programme non élevé. Le premier essai (10/11/2026) n'a capté aucun clic en jeu.

L'enregistreur note aussi, à ~30 Hz, si le curseur est visible quand le jeu a le focus (`GetCursorInfo`). C'est le mécanisme prévu pour la bulle (section 5.3) : le rapport dit s'il fonctionne avec ce jeu.

## Fichiers produits

| Fichier | Contenu |
|---|---|
| `capture.txt` | `ts_ms\|srcip:sport>dstip:dport\|seq\|hex`, un segment TCP serveur → client par ligne |
| `clicks.txt` | `ts_ms\|event\|…`, event = `down`, `up`, `marker` (clics, F8), `beep` (A/B), `phase`, `cursor`, `info` (privilèges) |
| `report.md` | Analyse : flux, entités nommées, fréquence des opcodes, opcodes classés par corrélation avec les clics, détail des candidats, verdict |

Ces fichiers contiennent des pseudos d'autres joueurs : ils restent sur le PC (`tools/captures/`, `tools/out/` et `*.txt` de capture sont ignorés par git).

## Ce qui est repris d'A2Tools (GPL-3.0)

- `src/framing.rs` : découpage du flux (varint, lots LZ4, enregistrements TLS ignorés), adapté de `src-tauri/src/capture/framing.rs` et `read_varint`.
- `src/pcap.rs` : chargement dynamique de `wpcap.dll`, approche de `pcap_capturer.rs`.
- `src/analyze.rs` : lecture des enregistrements d'identité `33 36` / `45 36` / `44 36`, logique de `masked_records`.
