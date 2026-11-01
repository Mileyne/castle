#!/bin/sh
# Programme de réglage de la lumière (soleil, ambiance, lanternes, héros, ennemis, objets). Options: --niveau 1..12, --sol terre|herbe|dense.
cd "$(dirname "$0")" && exec cargo run -p castle_client --bin lumiere -- "$@"
