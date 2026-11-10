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
