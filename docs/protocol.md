# Protocole réseau Aion 2 — ce qu'on sait

Tout ce qui est connu des paquets utilisés par LastView : opcodes, structures, position des champs, et **date de vérification**. Seul le sens serveur → client est lisible (le sens client → serveur est chiffré).

Source principale : [A2Tools DPS Meter](https://github.com/taengu/A2Tools-DPS-Meter). À compléter à l'étape 0.

## Paquets connus (d'après A2Tools, à revérifier à l'étape 0)

| Paquet | Opcode | Champs | Vérifié le | Statut |
|---|---|---|---|---|
| Spawn joueur | _(à relever dans A2Tools)_ | pseudo, classe, niveau, iNiv, CP, légion, serveur (si légion) | — | à vérifier |
| Mort | `0x3642` (anciennement `0x3641`, décalage +1 de juin 2026) | id d'entité | — | à vérifier |
| Changement de zone | `23 36` | id d'entité (0 = joueur local) | — | à vérifier |
| Chargement de carte | `21 36` | — | — | à vérifier |
| Dégâts entre entités | _(à relever)_ | source, cible, valeur | — | à vérifier |
| Groupe | _(à relever)_ | — | — | à vérifier |
| Ciblage | inconnu | — | — | **test étape 0 (2.3)** |
| Buff reçu | inconnu | — | — | test étape 0 (2.4) |

## Verdict ciblage (étape 0, section 2.3)

_(en attente du test en jeu avec Gio)_

## Reporter une correction après un patch

À documenter : tous les opcodes et offsets sont centralisés dans un seul module du back-end (`protocol/opcodes.rs` ou équivalent). Procédure à écrire une fois le module en place.
