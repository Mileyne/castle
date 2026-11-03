#!/bin/sh
# Lance l'éditeur de niveaux: le jeu lui-même, avec des panneaux et des outils. Option: --niveau=N pour ouvrir ce niveau.
cd "$(dirname "$0")" && cargo build --release -p castle_client --features editor --bin castle_editor && exec ./target/release/castle_editor "$@"
