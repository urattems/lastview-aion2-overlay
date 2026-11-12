# Test 2 — ton perso, le groupe, les buffs (étape 0, section 2.4)

**Optionnel depuis le 12/11/2026** : les structures de ta fiche et du groupe sont connues (voir `docs/protocol.md`), mais pas encore vues en capture. Ce test sert seulement à les voir passer en vrai, quand l'occasion se présente.

But : capter les paquets qu'on n'a pas encore vus. Durée : 5 minutes. Pas de bips cette fois.

Il te faut un joueur ami (un membre de légion par exemple) qui accepte de grouper avec toi.

## Avant

1. Ouvre PowerShell et lance :
   ```
   C:\Users\Giovanni\aion2-work\lastview-aion2-overlay\tools\recorder\target\release\lv-recorder.exe record C:\Users\Giovanni\aion2-work\lastview-aion2-overlay\tools\captures\test2 --sans-bips
   ```
   Tu dois voir `AION2.exe trouvé` et une ligne `connexion ...:13328`.

## Dans le jeu, dans cet ordre

2. **Change de zone** : téléporte-toi, ou entre et sors d'un donjon. C'est ce qui renvoie ta propre fiche.
3. **Groupe** : invite ton ami, ou rejoins son groupe. Reste groupé une minute.
4. **Buff** : demande-lui de te lancer un buff. Toi aussi, lance-lui un buff.
5. **Optionnel, si vous voulez** : un duel rapide entre vous deux. Ça permet de tester « Combat », « Kill » et « Mort ».
6. **Quitte le groupe.**
7. Dans le terminal : **Ctrl+C**.

## Après

8. Dis-moi que c'est fait, et donne-moi le pseudo de ton ami. Je fais l'analyse. Les fichiers restent sur le PC.
