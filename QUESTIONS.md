# Questions pour le chef de suivi

Une question par entrée : contexte, options, recommandation. La réponse est notée sous la question.

---

## Q1 — Repo public : la mise à jour automatique peut-elle se passer du jeton GitHub ? (2026-11-10)

**Contexte.** Le cahier (9.1) prévoyait un repo privé, et donc (9.5) un jeton GitHub en lecture seule saisi dans les paramètres pour que l'updater Tauri puisse télécharger les Releases. Gio a demandé que le repo soit **public**.

**Options.**
1. Repo public, updater Tauri branché directement sur les Releases GitHub publiques, sans jeton ni Gestionnaire d'identification. Le paramètre « jeton GitHub » disparaît.
2. Garder le mécanisme de jeton tel que décrit, inutile mais prêt si le repo repasse en privé.

**Recommandation.** Option 1 : plus simple, aussi sûr (les Releases restent signées par la clé de l'updater Tauri, qui vérifie l'intégrité des binaires), et un paramètre de moins. Si le repo repasse en privé un jour, le jeton pourra être ajouté à ce moment-là.

**Réponse.** _(en attente)_
