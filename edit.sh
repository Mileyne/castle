#!/bin/sh
# Lance l'éditeur d'art (tuiles et sprites). Option: --tuiles pour ouvrir l'onglet des tuiles.
cd "$(dirname "$0")" && cargo build --release -p sprite_editor && exec ./target/release/sprite_editor "$@"
