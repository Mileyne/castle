#!/bin/sh
# Application de choix des sons: écoute les variantes de chaque bruitage et applique les choix au jeu.
cd "$(dirname "$0")" && exec python3 tools/sfx/app/server.py "$@"
