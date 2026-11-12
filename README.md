# LastView — Aion 2 Overlay

Overlay passif pour Aion 2 (Windows 11) : affiche, par-dessus le jeu, le stuff réel des joueurs que vous rencontrez (ciblage, combat, kill, mort, groupe), avec une fiche complète calquée sur shugo.gg.

> **Fan-made, non affilié à NCSOFT.** Projet personnel sous licence GPL-3.0.

## État du projet

En construction. Voir [JOURNAL.md](JOURNAL.md) pour l'avancement et [docs/CAHIER_DES_CHARGES.md](docs/CAHIER_DES_CHARGES.md) pour la spécification complète de la V1.

## Ce que fait LastView

1. Écoute passivement une copie du trafic réseau du jeu (Npcap). Jamais de lecture mémoire, d'injection ni d'envoi de paquets.
2. Mémorise en silence les joueurs qui apparaissent autour de vous.
3. Dès qu'une rencontre réelle a lieu (vous ciblez le joueur, combat, kill, mort, groupe), il apparaît en haut d'une petite bulle au-dessus du jeu : pseudo, classe, GS, CP.
4. Un clic sur « + » ouvre une fiche complète dans une fenêtre à part : équipement, compétences et stigmas, Daevanion, stats.
5. L'historique est conservé 4 h (réglable) et survit à un redémarrage.

Tout reste en local. Les seuls appels réseau sortants sont les lectures de l'API publique NCSoft (données de personnage) et GitHub (mise à jour).

## Prérequis

- Windows 11, jeu en plein écran fenêtré (borderless).
- [Npcap](https://npcap.com) installé (déjà présent si vous utilisez Abyss Logs ou A2Tools).
- WebView2 (inclus dans Windows 11).

## Installation

À venir : installateur `.exe` et version portable dans les Releases GitHub.

Le binaire n'est pas signé. Au premier lancement, SmartScreen peut afficher un avertissement : cliquez sur « Informations complémentaires » puis « Exécuter quand même ».

## Utilisation

À venir.

## Paramètres

À venir.

## Développement

Stack : Tauri v2, Rust (capture, parsing, SQLite, API), front HTML/CSS/TypeScript (Vite, sans framework). Détails : [docs/architecture.md](docs/architecture.md).

```
lastview-aion2-overlay/
  crates/lastview-core/  cœur sans interface : capture, protocole, mémoire, rencontres, base, API
  src-tauri/             application Tauri : fenêtres, zone de notification, commandes
  src/                   front (bulle, fiche, paramètres)
  tools/                 outils de l'étape 0
  docs/                  cahier des charges, architecture, protocole, API
```

Prérequis : Rust stable (MSVC), Node 22 ou plus, Npcap.

```
npm install
cargo test -p lastview-core          # tests du cœur
npm run tauri dev                    # l'app en mode développement
cargo run -p lastview-core --release --bin lastview-dump -- live   # le cœur seul, en console
```

## Crédits

La capture réseau et la connaissance du protocole s'appuient sur [A2Tools DPS Meter](https://github.com/taengu/A2Tools-DPS-Meter) (GPL-3.0). Voir [NOTICE](NOTICE).
