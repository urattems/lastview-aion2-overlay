# Test de ciblage — à faire en jeu (étape 0, section 2.3)

But : savoir si le serveur envoie un paquet quand tu cibles un joueur. Durée : 2 minutes, tout est guidé par des bips.

Pourquoi des bips : le jeu est protégé, et un programme normal ne voit pas tes clics quand le jeu a le focus (constaté au premier essai du 10/11). L'enregistreur te dit donc **quand** cliquer, et c'est l'heure des bips qui sert de référence.

## Avant

1. Mets-toi en jeu dans un endroit **calme**, avec deux joueurs **immobiles** à l'écran : A et B. Garde le son du PC allumé.
2. Ouvre PowerShell et lance :
   ```
   C:\Users\Giovanni\aion2-work\lastview-aion2-overlay\tools\recorder\target\release\lv-recorder.exe record C:\Users\Giovanni\aion2-work\lastview-aion2-overlay\tools\captures\ciblage2
   ```
   Tu dois voir `AION2.exe trouvé`, une ligne `connexion ...:13328`, puis `=== MÉTRONOME ===`.
3. Tu as **30 secondes** pour retourner dans le jeu.

## Le test (tu n'as rien à toucher dans le terminal)

4. **3 bips courts** : début de la référence. Pendant **20 secondes**, ne cible personne. Tu peux bouger la caméra.
5. Ensuite, **12 bips**, un toutes les 5 secondes :
   - **bip aigu** : maintiens Alt et clique sur **A** ;
   - **bip grave** : maintiens Alt et clique sur **B**.
   Clique bien sur le joueur : sa bannière doit apparaître en haut. Rien d'autre entre les bips.
6. **2 bips longs** : c'est fini, ne cible plus personne. L'enregistrement s'arrête seul 10 secondes plus tard.

## Après

7. Lance l'analyse :
   ```
   C:\Users\Giovanni\aion2-work\lastview-aion2-overlay\tools\recorder\target\release\lv-recorder.exe analyze C:\Users\Giovanni\aion2-work\lastview-aion2-overlay\tools\captures\ciblage2
   ```
8. Dis-moi que c'est fait. Je lis le rapport. Il ne quitte pas le PC.

Si tu rates un bip, ce n'est pas grave : continue au suivant.
