#!/bin/sh
# Compile (si besoin) et lance Castle: le client démarre le serveur local tout seul.
cd "$(dirname "$0")" && cargo build --release -p castle_server -p castle_client && exec ./target/release/castle_client "$@"
