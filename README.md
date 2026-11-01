# Castle

RPG d'action multijoueur en Rust (Bevy). Un seul code (`client/`, `server/`, `shared/`) pour tous les clients; le serveur
est une bibliothèque, lancée sur un fil du client: un seul exécutable partout (`castle_server` reste disponible pour un serveur dédié).

## Clients

| Plateforme | Script | Résultat dans `dist/` | Prérequis |
|---|---|---|---|
| macOS | `scripts/build_macos.sh` | `Castle-v<version>-macOS-<arch>/` et `.zip` | Rust |
| Windows 64 bits | `scripts/build_windows.sh` | `Castle-v<version>-Windows-x64/` et `.zip` | Rust, mingw-w64 (`brew install mingw-w64`) |
| Linux 64 bits | `scripts/build_linux.sh` (sous Linux) | `Castle-v<version>-Linux-x64/` et `.zip` | Rust, bibliothèques alsa, udev, wayland/x11 |
| iPhone / iPad | `scripts/build_ios.sh [simulator\|device]` | `Castle-v<version>-iOS-<cible>.app` | Mac, Xcode, Rust ([ios/README.md](ios/README.md)) |
| Apple TV | `scripts/build_tvos.sh` | `Castle-v<version>-tvOS.app` | Mac, Xcode, Rust nightly ([tvos/README.md](tvos/README.md)) |

`scripts/build_all.sh` construit tout ce que la machine sait construire (sur un Mac: macOS, Windows, iOS simulateur, tvOS);
on peut lui donner la liste des plateformes voulues: `scripts/build_all.sh macos windows`.

Pour jouer directement depuis les sources: `./play.sh` (le client démarre le serveur local tout seul).

## Outils

- `./edit.sh`: éditeur d'art (tuiles et sprites).
- Décor au sol: les niveaux listent des éléments de décor (`decor`, herbes, cailloux, flaques, ossements, tapis...), assemblés aux tuiles par le client; `castle_server --decorate shared/levels [--force]` en sème selon la région, et l'outil Décor de l'éditeur en pose à la main. Les grands motifs (chênes, sapins, rochers, colonnes, statues, fontaines: `Thing::Landmark`) bloquent le passage par leur pied et passent devant ou derrière les personnages selon leur position. Quand deux régions se touchent (champ `style`), leurs tuiles se fondent l'une dans l'autre.
- Rendu 3D: le jeu est une scène 3D. Les héros, les monstres et les objets sont de vrais modèles 3D de sprite_forge, chacun un seul maillage fusionné par pose (`client/src/merged.rs`); les poses (repos, marche, coup) de tous les modèles d'un niveau sont construites à l'avance au lancement du niveau, derrière un écran « Veuillez patienter » avec son pourcentage. Plus de mode sprites. `CASTLE_SHOT=<fichier.png>` n'existe plus.
- Décor en 3D: le sol est texturé par les blocs de décor, les murs sont des cubes complets (matière calculée par le shader selon la région), éclairés par un soleil, des ombres portées et une lanterne qui suit le héros dirigé. Hauteur des murs 3 tuiles, cadrage 125 %, inclinaison 52°, angle -15° par défaut. Manette principale pendant la partie: stick droit = tourner et incliner la vue, L / R (ou ZL / ZR) = zoom, Select = remise par défaut; les touches de direction suivent l'écran. `CASTLE_FOV=<degrés>` passe en perspective (essayez 30). F3 affiche ou cache l'indicateur d'images par seconde (en haut à droite).
- `tools/live3d`: essai de rendu en direct (les modèles de sprite_forge dessinés en 3D par la carte graphique, sans planche). `BEVY_ASSET_ROOT=$PWD/tools/live3d cargo run --release -p live3d [-- --bench]`.
- `tools/sprite_forge`: génère les planches de sprites du style médiéval (`cd` à la racine puis `cargo run --release -p sprite_forge -- <dossier>`): une planche par personnage, par monstre et par objet (`heroes/`, `monsters/`, `objects/`), à 288 px la case (constante `DETAIL`), rognées à ce qu'elles dessinent et compressées en `.basis` (Basis Universal: le jeu les décompresse vers BC7, ASTC ou ETC2 selon la carte graphique). Après une génération, copier `sprite_meta.rs` dans `client/src/`.

## Documentation

[CASTLE.md](CASTLE.md): conception du jeu, fonctionnalités et feuille de route.
- `tools/model_viewer`: visionneuse des modèles 3D du jeu, un exécutable à part (`scripts/models.sh`). Elle montre le modèle fusionné tel que le jeu le dessine, sous tous les angles, avec le même pré-calcul des poses au lancement et son écran « Veuillez patienter ». Manette: stick droit pour tourner, L / R pour le zoom, croix pour le modèle (gauche / droite) et l'action (haut / bas: repos, marche, coup), Y rotation automatique, Select recentrer. X (ou F) bascule entre le rendu du jeu et le même modèle sous trois rendus côte à côte (sans éclairage, éclairé sans texture, éclairé avec le tissage des vêtements).
- Rendu des personnages: des matériaux de Bevy (`StandardMaterial`), éclairés par le soleil du décor. Chaque pièce a une matière d'après son reflet et sa couche (tissu mat avec une texture de tissage, peau, mailles, métal poli, pièces lumineuses sans éclairage), donc un maillage par matière et par pose. Le réglage « Rendu des personnages » (menu de pause et réglages du titre; `CASTLE_SHADER=lit|flat` pour essayer) choisit entre éclairé (le défaut) et sans lumière (couleurs pures, sans texture, le plus léger). Le shader « toon » maison a été retiré. Dans la visionneuse, `MODEL_VIEWER_MODEL=hero:Sorcerer MODEL_VIEWER_FUSION=shaders MODEL_VIEWER_DISTANCE=3` ouvre directement sur un modèle.
- Visages des héros: le crâne de chaque héros porte la peau de sa photo de référence (`client/assets/art/medieval/faces/<classe>.png`, 24 x 24 pixels: le visage de `photos/` réduit, qui ne garde que les teintes; les yeux, le nez et la bouche restent des pièces 3D). Les pièces du corps (visage, cheveux, barbe, oreilles) sont dessinées avec plus de facettes. Pour cadrer une tête dans la visionneuse: `MODEL_VIEWER_MODEL=hero:Ranger MODEL_VIEWER_HEIGHT=1.45 MODEL_VIEWER_DISTANCE=1.7 scripts/models.sh`.
