# Travailler un personnage dans Blender

Les personnages du jeu sont des **listes de pièces** (sphères, cubes, capsules, cônes tronqués) décrites en code dans
`tools/sprite_forge`. Pour les éditer dans Blender, on les exporte en **glTF** (`.glb`), le format que Blender lit et écrit nativement.

## 1. Exporter

```
scripts/blender_export.sh                       # tous les modèles
scripts/blender_export.sh hero:Knight monster:Orc   # certains seulement
```

Les fichiers sont écrits dans `client/assets/exports/` (ignoré par git): `hero_Knight.glb`, `hero_Sorcerer.glb`, `hero_Ranger.glb`,
`hero_Forgebraise.glb`, `monster_*.glb`, `prop_*.glb`. Les clés de modèle sont celles de `tools/model_viewer` (`hero:Knight`...).

Chaque fichier contient:

| Dans le fichier | Dans Blender |
|---|---|
| une pièce du modèle | un objet nommé `012_Metal_cone` (numéro, matière, forme), enfant de l'objet racine `hero_Knight` |
| une matière (acier, tissu, peau...) | un matériau nommé `Metal_8a8e98` (matière et couleur), avec sa rugosité et son métal |
| une pièce qui brille | un matériau avec émission |
| la peau du crâne d'un héros | le matériau `Face_...` avec la photo réduite en texture |
| les poses de repos, marche et coup | trois actions `Idle` (25 images), `Walk` (16) et `Attack` (10), à 25 images par seconde |

Les matières sont déduites du reflet de chaque pièce dans `sprite_forge`: tissu et cuir (mat), peau, mailles, métal (poli), lumière.

**Repère**: le mètre, y vers le haut (Blender convertit en z vers le haut à l'import), le personnage regarde vers +z (vers -y dans Blender)
et a les pieds au sol. Un héros mesure environ 1,8 m.

## 2. Voir le résultat sans lancer le jeu

```
MODEL_VIEWER_GLB=exports/hero_Knight.glb scripts/models.sh
```

La visionneuse ouvre le fichier à la place du modèle du jeu (chemin sous `client/assets/`), avec le stick droit pour tourner et L / R pour le
zoom. C'est aussi la façon de vérifier un fichier après l'avoir sauvé depuis Blender (« File > Export > glTF 2.0 », format `.glb`).

## 3. Ce que l'export ne fait pas (encore)

- **Pas de squelette ni de peau.** Les animations déplacent des objets, pièce par pièce. Pour un personnage lisse et animé par os, il faut
  le remodeler dans Blender (en s'aidant de l'export comme gabarit), lui poser une armature, puis animer `Idle`, `Walk` et `Attack`.
- **Pas de retour dans le jeu.** Le jeu fabrique toujours les personnages à partir du code. Charger un `.glb` édité (un fichier par
  personnage, qui remplacerait le modèle du code) est l'étape suivante; elle demande de fixer les conventions ci-dessous.
- **Les traînées des armes** (pièces lumineuses qui apparaissent pendant un coup) ne sont pas exportées: elles viennent du code.
- La texture de tissage des vêtements n'est pas exportée (seule la peau du crâne l'est).

## 4. Conventions à respecter pour un futur import dans le jeu

- Un fichier par modèle: `client/assets/models/hero_Knight.glb`, même clé que l'export.
- Échelle: 1 unité = 1 mètre; pieds à l'origine; face vers +z (dans Blender: vers -y); y vers le haut dans le fichier glTF.
- Trois animations nommées exactement `Idle`, `Walk` et `Attack`; `Idle` et `Walk` en boucle.
- Un matériau par matière; les noms de matériaux commencent par `Cloth`, `Skin`, `Mail`, `Metal`, `Glow` ou `Face`: le jeu leur applique sa
  rugosité, son éclairage et ses textures.
- Pas plus de quelques milliers de triangles par personnage (aujourd'hui, 10 à 20 mille sommets par pose).
