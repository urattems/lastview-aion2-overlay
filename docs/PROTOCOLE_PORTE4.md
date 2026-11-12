# Test de la porte 4 — le cœur en jeu

But : vérifier en vrai que la mémoire silencieuse se remplit et qu'une rencontre crée une ligne en base avec les bons chiffres. Durée : 5 minutes. Rien à installer.

## Avant

1. Ouvre PowerShell et lance :
   ```
   cd C:\Users\Giovanni\aion2-work\lastview-aion2-overlay
   .\target\release\lastview-dump.exe live --minutes 6
   ```
   Tu dois voir `[état] Active { serveur: "…:13328" }`.

## Dans le jeu

2. **Change de zone** (téléportation, ou entre puis sors d'un donjon). Ça envoie ta fiche : la ligne `moi` doit passer à `(confirmé)` dans les 30 s.
3. Va dans un endroit **avec du monde** (capitale, zone de quêtes). Le nombre de joueurs `mémoire` doit monter.
4. **Cible 3 joueurs différents** (Alt + clic), à quelques secondes d'intervalle. Une ligne `Ciblé` s'affiche pour chacun.
5. Optionnel : si un joueur te buffe, si tu groupes, ou en PvP, les lignes `Buff`, `Groupe`, `Combat`, `Kill` ou `Mort` doivent apparaître aussi.

## Après

6. L'outil s'arrête seul après 6 minutes et affiche la liste en base.
7. Compare pour un des joueurs ciblés : pseudo, classe, GS et CP de la ligne contre sa bannière en jeu, ou contre shugo.gg.
8. Envoie-moi la fin de l'affichage (tu peux masquer les pseudos si tu veux). La base reste sur ton PC (`%APPDATA%\LastView`).
