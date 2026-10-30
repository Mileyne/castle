#!/usr/bin/env bash
# Compilateur C/C++ pour la cible tvOS déguisée en iOS: le crate `cc` y voit « ios » et vise iOS; on impose tvOS.
args=()
skip=0
for a in "$@"; do
  if [ $skip -eq 1 ]; then skip=0; continue; fi
  case "$a" in
    -target|-isysroot) skip=1 ;;
    -miphoneos-version-min=*|-mios-version-min=*|--target=*) ;;
    *) args+=("$a") ;;
  esac
done
exec clang -target arm64-apple-tvos15.0 -isysroot "$SDKROOT" "${args[@]}"
