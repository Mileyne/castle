#!/bin/sh
# Compile (profil dev, rapide à recompiler) et lance Castle: le client démarre le serveur local tout seul.
cd "$(dirname "$0")" && exec cargo run -p castle_client -- "$@"
