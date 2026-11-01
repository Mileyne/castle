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
- Rendu 3D en direct dans le jeu (par défaut sur ordinateur): les héros et les monstres sont dessinés en 3D à partir des modèles de sprite_forge, dans une grande image dont chaque personnage a une case, et restent des sprites dans le monde 2D. `CASTLE_LIVE3D=0` revient aux planches de sprites; Apple TV et appareils à peu de mémoire les gardent d'office (`--no-default-features` retire aussi le code 3D de la compilation). `CASTLE_SHOT=<fichier.png>` enregistre une capture au bout de 20 s.
- Décor en 3D (prototype, ordinateur seulement): `CASTLE_DECOR3D=1`. Le monde devient une vraie scène 3D vue de biais: sol texturé par les blocs de décor, murs en cubes complets (six faces, matière calculée par le shader selon la région: briques de pierre, ou troncs et feuillage), éclairés par un soleil, des ombres portées et une lanterne qui suit le héros dirigé. Les héros, les monstres et les objets qui ont un modèle (pièces, nourriture, clés, potions, pièges, générateurs, tourelles...) sont de vrais modèles 3D dans la scène, vus sous l'angle et l'inclinaison réglés, nets à tout cadrage et masqués par les murs; les autres sprites du monde (grands motifs, portes, taches au sol) sont dessinés dans la scène, debout ou à plat; seules la météo et l'interface restent en 2D. Hauteur des murs (3 tuiles), cadrage (125 %), inclinaison (52°) et angle de la vue (-15°) sont fixes par défaut. Pendant la partie, la manette principale (la première branchée) règle la caméra: le stick droit fait tourner la vue autour du décor (gauche/droite) et l'incline (haut/bas), la gâchette gauche (L ou ZL) éloigne le cadrage et la droite (R ou ZR) le rapproche, et Select remet tout par défaut. Le stick gauche et les boutons restent ceux du joueur, et les autres manettes commandent les autres joueurs; les touches de direction suivent l'écran (« haut » va vers le haut de l'écran). `CASTLE_FOV=<degrés>` passe en perspective (essayez 30). F3 affiche ou cache l'indicateur d'images par seconde (en haut à droite).
- `tools/live3d`: essai de rendu en direct (les modèles de sprite_forge dessinés en 3D par la carte graphique, sans planche). `BEVY_ASSET_ROOT=$PWD/tools/live3d cargo run --release -p live3d [-- --bench]`.
- `tools/sprite_forge`: génère les planches de sprites du style médiéval (`cd` à la racine puis `cargo run --release -p sprite_forge -- <dossier>`): une planche par personnage, par monstre et par objet (`heroes/`, `monsters/`, `objects/`), à 288 px la case (constante `DETAIL`), rognées à ce qu'elles dessinent et compressées en `.basis` (Basis Universal: le jeu les décompresse vers BC7, ASTC ou ETC2 selon la carte graphique). Après une génération, copier `sprite_meta.rs` dans `client/src/`.

## Documentation

[CASTLE.md](CASTLE.md): conception du jeu, fonctionnalités et feuille de route.
