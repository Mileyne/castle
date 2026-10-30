#!/bin/sh
# Client Apple TV (il faut un Mac avec Xcode et la chaîne d'outils nightly de rustup, voir tvos/README.md).
# Compile pour l'appareil (pas de simulateur); l'Apple TV doit être appairée dans Xcode et l'équipe choisie dans
# tvos/Castle.xcodeproj. Pour l'installer: TVOS_DEVICE=<identifiant> scripts/build_tvos.sh
# Résultat: dist/Castle-v<version>-tvOS.app
. "$(dirname "$0")/common.sh"
rustup toolchain list | grep -q nightly || { echo "La chaîne d'outils nightly est requise (rustup toolchain install nightly -c rust-src)" >&2; exit 2; }
destination="${TVOS_DEVICE:+id=$TVOS_DEVICE}"
destination="${destination:-generic/platform=tvOS}"
(cd tvos && xcodebuild -project Castle.xcodeproj -scheme Castle -configuration Release -destination "$destination" \
  -derivedDataPath build -allowProvisioningUpdates build)
out="$DIST/Castle-v$VERSION-tvOS.app"
rm -rf "$out"
cp -R tvos/build/Build/Products/Release-appletvos/Castle.app "$out"
echo "==> $out"
if [ -n "${TVOS_DEVICE:-}" ]; then
  xcrun devicectl device install app --device "$TVOS_DEVICE" "$out"
fi
