#!/bin/sh
# Exporte les modèles 3D du jeu en glTF (`.glb`) pour les ouvrir dans Blender: voir docs/blender.md.
# Sans argument, tous les modèles; sinon leurs clés (hero:Knight, monster:Orc, prop:Coin...).
cd "$(dirname "$0")/.." && cargo run --release -q -p sprite_forge -- export-glb client/assets/exports "$@"
