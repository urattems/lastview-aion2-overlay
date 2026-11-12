# Questions pour le chef de suivi

Une question par entrée : contexte, options, recommandation. La réponse est notée sous la question.

---

## Q1 — Repo public : la mise à jour automatique peut-elle se passer du jeton GitHub ? (2026-11-10)

**Contexte.** Le cahier (9.1) prévoyait un repo privé, et donc (9.5) un jeton GitHub en lecture seule saisi dans les paramètres pour que l'updater Tauri puisse télécharger les Releases. Gio a demandé que le repo soit **public**.

**Options.**
1. Repo public, updater Tauri branché directement sur les Releases GitHub publiques, sans jeton ni Gestionnaire d'identification. Le paramètre « jeton GitHub » disparaît.
2. Garder le mécanisme de jeton tel que décrit, inutile mais prêt si le repo repasse en privé.

**Recommandation.** Option 1 : plus simple, aussi sûr (les Releases restent signées par la clé de l'updater Tauri, qui vérifie l'intégrité des binaires), et un paramètre de moins. Si le repo repasse en privé un jour, le jeton pourra être ajouté à ce moment-là.

**Réponse (chef de suivi, 12/11/2026).** Option 1 retenue. Le paramètre « jeton » est supprimé, l'updater lit les Releases publiques. Repo public = aucun secret, jamais.

---

## Q2 — Origine des structures de paquets, dans un repo public (2026-11-11)

**Contexte.** Les structures exactes des paquets ont été recoupées avec un outil tiers installé sur le PC de Gio, avec son accord. Aucun code repris. Question : faut-il nommer cet outil dans un repo public ?

**Réponse (chef de suivi et Gio, 12/11/2026).** Ne pas le nommer. Les structures vérifiées sur nos captures sont présentées comme telles. Les autres sont marquées « non vérifié en capture, à confirmer au premier usage réel ».
