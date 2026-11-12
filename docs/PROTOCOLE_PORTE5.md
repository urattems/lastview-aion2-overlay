# Séance de jeu de la porte 5 — la bulle, et ce qui reste à voir en vrai

Une seule séance, d'environ 45 minutes. Elle couvre :
- le raccourci clavier ;
- la bulle (survol, clics traversants, masquage) ;
- ta fiche (`33 36`) ;
- le groupe ;
- les dégâts entre joueurs.

Joue normalement : c'est le but. Rien n'est envoyé ailleurs, tout reste sur ton PC.

## 1. Le raccourci clavier (3 minutes, avant la bulle)

1. Ouvre PowerShell et lance :
   ```
   cd C:\Users\Giovanni\aion2-work\lastview-aion2-overlay
   .\target\release\lastview-dump.exe raccourci --minutes 3
   ```
2. Va dans le jeu et appuie **3 fois** sur **Ctrl + Maj + F9**, avec quelques secondes d'écart.
3. Reviens sur le bureau et appuie encore **une fois**.
4. Attends la fin (3 min). Garde la fenêtre ouverte : je veux les lignes `appui n° … jeu au premier plan : OUI/non`.

## 2. Démarrer l'enregistreur et l'app

5. Dans une **première** fenêtre PowerShell :
   ```
   cd C:\Users\Giovanni\aion2-work\lastview-aion2-overlay
   .\tools\recorder\target\release\lv-recorder.exe record .\tools\captures\porte5 --sans-bips
   ```
6. Dans une **deuxième** fenêtre PowerShell :
   ```
   cd C:\Users\Giovanni\aion2-work\lastview-aion2-overlay
   .\target\release\lastview.exe --diagnostic
   ```
   L'icône LastView (un œil ambre) apparaît dans la zone de notification. Aucune fenêtre ne s'ouvre.

## 3. Dans le jeu

7. **Change de zone** tout de suite : téléporte-toi, ou entre puis sors d'un donjon. C'est ce qui envoie ta fiche.
8. La bulle apparaît **en haut à droite**, à 30 % d'opacité. Elle affiche « Aucune rencontre », ou le dernier joueur.
9. **Survol** : tiens **Alt**, mets le curseur sur la bulle. Elle passe à 100 %.
   - Clique sur la pastille : elle se déroule.
   - Fais défiler la liste.
   - Survole une ligne et clique sur l'étoile.
   - Clique sur un « + » : une fenêtre de fiche s'ouvre (vide pour l'instant, c'est la section 6). Ferme-la.
   - Déplace la bulle en tirant sur l'espace vide de son en-tête.
10. **Lâche Alt** : en un tiers de seconde, la bulle revient à 30 % et laisse passer les clics.
11. **Alt + Tab** vers le bureau : la bulle disparaît. Reviens dans le jeu : elle revient.
12. **Joue normalement au moins 30 minutes**, en combattant, en ciblant, en bougeant la caméra. Note si **un seul clic** a semblé « mangé » par la bulle : à quelle heure, et ce que tu faisais.
13. Pendant ces 30 minutes, si tu peux :
    - **Groupe** : entre dans un donjon en **recherche de groupe automatique**. Pas besoin d'amis.
    - **Combat entre joueurs** : un duel, ou un peu de PvP en Abysses.
14. **Captures pour le chef de suivi** (Win + Maj + S), quelques-unes :
    - la bulle repliée en jeu ;
    - la bulle déroulée pendant le survol ;
    - la bulle au repos (30 %) par-dessus le jeu.

## 4. Après

15. Ferme le jeu, ou arrête simplement :
    - Ctrl + C dans la fenêtre de l'enregistreur ;
    - clic droit sur l'icône LastView, puis « Quitter ».
16. Dis-moi que c'est fini, avec :
    - le résultat du test du raccourci (point 4) ;
    - les clics « mangés », s'il y en a eu ;
    - ce qui ne t'a pas plu dans la bulle ;
    - tes captures.

    Je lirai le journal (`%APPDATA%\LastView\logs\lastview.log`) et l'enregistrement.
