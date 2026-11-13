# Livraison : Releases et mise à jour automatique

Ce document décrit comment publier une version de LastView et comment fonctionne la mise à jour automatique (cahier, section 9).

## Principe

- Chaque tag `vX.Y.Z` poussé sur GitHub lance le workflow [`release.yml`](../.github/workflows/release.yml). Il fait, dans l'ordre :
  1. vérifie que le tag correspond à la version de l'app ;
  2. lance les tests ;
  3. compile et signe l'installateur ;
  4. vérifie que les pages sont intégrées au binaire ;
  5. crée la Release.
- La Release contient :
  - `LastView_X.Y.Z_x64-setup.exe` : l'installateur (recommandé) ;
  - `LastView_X.Y.Z_portable.exe` : la version sans installation ;
  - `LastView_X.Y.Z_x64-setup.exe.sig` et `latest.json` : pour la mise à jour automatique.
- L'app installée lit `latest.json` sur la dernière Release publique, une minute après son démarrage puis toutes les 6 h. Elle vérifie la signature avec la **clé publique** inscrite dans `src-tauri/tauri.conf.json`.
- Une mise à jour est proposée, jamais forcée. Elle s'installe sur clic (zone de notification) ou à la fermeture de LastView, et jamais à la fermeture si le jeu tourne.
- Une compilation locale (dans un dossier `target`) ne cherche jamais de mise à jour.

## La clé de signature

La **clé privée** signe les installateurs. Elle n'est **jamais** dans le repo, qui est public, ni dans aucun fichier du projet.

Elle a été générée sur le PC de Gio, hors du repo, dans `%USERPROFILE%\.lastview-signature\` :

| Fichier | Contenu | Où il va |
|---|---|---|
| `lastview.key` | clé privée (chiffrée par le mot de passe) | secret GitHub + sauvegarde hors du PC |
| `mot-de-passe.txt` | mot de passe de la clé | secret GitHub + gestionnaire de mots de passe |
| `lastview.key.pub` | clé publique | déjà dans `tauri.conf.json` (sans risque) |

> **Si la clé privée ou son mot de passe sont perdus, plus aucune mise à jour automatique n'est possible pour les versions déjà installées.** Il faudrait désinstaller et réinstaller à la main partout.

### 1. Sauvegarder la clé hors du PC (une fois)

Au choix, idéalement les deux :

- **Gestionnaire de mots de passe** : créer une entrée « LastView — clé de l'updater ». Mettre le contenu de `mot-de-passe.txt` dans le champ mot de passe, et le contenu complet de `lastview.key` dans les notes (ouvrir le fichier avec le Bloc-notes, tout sélectionner, copier).
- **Clé USB** : copier le dossier `%USERPROFILE%\.lastview-signature\` entier sur une clé USB rangée à part.

Une fois le mot de passe sauvegardé, `mot-de-passe.txt` peut être supprimé du PC.

### 2. Ajouter les deux secrets au repo GitHub (une fois)

**Par le site** :

1. Ouvrir <https://github.com/urattems/lastview-aion2-overlay/settings/secrets/actions>.
2. « New repository secret » :
   - Name : `TAURI_SIGNING_PRIVATE_KEY` ;
   - Secret : le contenu complet de `lastview.key` (Bloc-notes, tout sélectionner, copier) ;
   - « Add secret ».
3. « New repository secret » encore :
   - Name : `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` ;
   - Secret : le contenu de `mot-de-passe.txt` ;
   - « Add secret ».

**Ou en ligne de commande** (PowerShell, `gh` connecté au compte) :

```powershell
cd C:\Users\Giovanni\aion2-work\lastview-aion2-overlay
Get-Content "$env:USERPROFILE\.lastview-signature\lastview.key" -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY
Get-Content "$env:USERPROFILE\.lastview-signature\mot-de-passe.txt" -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
gh secret list
```

`gh secret list` doit montrer les deux noms. GitHub ne réaffiche jamais leur valeur.

## Publier une version

1. La version est la même dans `src-tauri/tauri.conf.json`, `Cargo.toml` (workspace) et `package.json`. Le workflow refuse un tag qui ne correspond pas.
2. Ajouter l'entrée dans `CHANGELOG.md`.
3. Pousser la branche puis le tag :

```powershell
git push origin etape-9
git tag v0.9.0
git push origin v0.9.0
```

4. Suivre le build dans l'onglet « Actions » du repo (une dizaine de minutes). La Release apparaît à la fin dans « Releases », avec les 4 fichiers.

## Tester la mise à jour automatique (porte 9)

1. **v0.9.0** : sur le 2e PC, télécharger `LastView_0.9.0_x64-setup.exe` **depuis la page de la Release uniquement**, installer Npcap si besoin, puis installer LastView. SmartScreen : « Informations complémentaires » puis « Exécuter quand même ».
2. Vérifier : icône dans la zone de notification, « À propos de LastView 0.9.0 » dans son menu.
3. **v0.9.1** : une petite version de test est préparée sur le PC principal, puis son tag est poussé.
4. Sur le 2e PC, LastView trouve la mise à jour une minute après son démarrage, ou tout de suite par « Rechercher une mise à jour » dans le menu. Une notification annonce « La version 0.9.1 est prête ».
5. Cliquer sur « Installer la version 0.9.1 » dans le menu, ou quitter LastView (jeu fermé). L'installateur tourne seul, puis LastView redémarre.
6. Vérifier « À propos de LastView 0.9.1 ».

## Pannes possibles

| Symptôme | Cause probable |
|---|---|
| Le workflow s'arrête sur « secret TAURI_SIGNING_PRIVATE_KEY absent » | Secrets pas encore ajoutés (étape 2) |
| Le workflow s'arrête sur « tag … mais version … » | Version pas montée partout avant le tag |
| « Mise à jour : vérification impossible » dans le journal | Pas d'Internet, ou `latest.json` absent de la dernière Release |
| Signature refusée | Release signée avec une autre clé que celle de `tauri.conf.json` |
