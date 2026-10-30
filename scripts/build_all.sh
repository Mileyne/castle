#!/bin/sh
# Construit tous les clients que cette machine sait construire, dans dist/. Chaque plateforme a son script:
#   scripts/build_macos.sh  build_windows.sh  build_linux.sh  build_ios.sh  build_tvos.sh
# Arguments: les plateformes voulues (macos windows linux ios tvos); sans argument, toutes celles qui sont possibles ici.
cd "$(dirname "$0")/.." || exit 1
if [ "$#" -eq 0 ]; then
  case "$(uname -s)" in
    Darwin) set -- macos windows ios tvos ;;
    Linux) set -- linux windows ;;
    *) echo "Système non géré: lancez le script de la plateforme voulue." >&2; exit 2 ;;
  esac
fi
failed=""
for platform in "$@"; do
  echo "######## $platform"
  if ! sh "scripts/build_$platform.sh"; then
    failed="$failed $platform"
  fi
done
echo
if [ -n "$failed" ]; then
  echo "Échec:$failed" >&2
  exit 1
fi
echo "Tout est dans dist/."
