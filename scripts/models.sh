#!/bin/sh
# Visionneuse des modèles 3D des personnages (un exécutable à part du jeu): voir tools/model_viewer.
# Manette: stick droit = tourner, L / R = zoom, croix = modèle / animation, A = étape, X = comparer, Y = rotation, Select = recentrer.
cd "$(dirname "$0")/.." && cargo build --release -p model_viewer && exec ./target/release/model_viewer "$@"
