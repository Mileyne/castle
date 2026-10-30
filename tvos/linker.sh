#!/usr/bin/env bash
# Linker pour la cible tvOS déguisée en iOS: rustc marque ses propres objets « iOS »; on les remet en tvOS avant `cc`.
for a in "$@"; do
  case "$a" in
    */symbols.o) vtool -remove-build-version ios -output "$a" "$a" ;;
  esac
done
exec cc "$@"
