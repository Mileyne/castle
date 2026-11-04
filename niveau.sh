#!/bin/sh
# Lance le jeu directement au niveau N (1 à 20), sans écran titre ni prologue, dans une sauvegarde jetable.
# Usage: ./niveau.sh 11
[ -n "$1" ] || { echo "usage: $0 N   (N de 1 à 20)"; exit 2; }
cd "$(dirname "$0")" && cargo build --release -p castle_client && exec ./target/release/castle_client --niveau="$1"
