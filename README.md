# Castle

RPG d'action multijoueur en Rust (Bevy). Un seul code (`client/`, `server/`, `shared/`) pour tous les clients; le serveur
est une bibliothèque, lancée par le client (sur iOS et tvOS, sur un fil du client).

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
- `tools/sprite_forge`: génère les planches de sprites (`cd` à la racine puis `cargo run --release -p sprite_forge -- <dossier>`).

## Documentation

[CASTLE.md](CASTLE.md): conception du jeu, fonctionnalités et feuille de route.
