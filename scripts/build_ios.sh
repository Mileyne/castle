#!/bin/sh
# Client iPhone / iPad (il faut un Mac avec Xcode). Sans argument, compile pour le simulateur (sans signature);
# `device` compile pour l'appareil branché (il faut choisir son équipe dans ios/Castle.xcodeproj, voir ios/README.md).
# Pour l'installer: IOS_DEVICE=<identifiant> scripts/build_ios.sh device (identifiants: xcrun devicectl list devices)
# Résultat: dist/Castle-v<version>-iOS-<simulator|device>.app
. "$(dirname "$0")/common.sh"
target="${1:-simulator}"
if [ "$target" = device ]; then
  destination="${IOS_DEVICE:+id=$IOS_DEVICE}"
  destination="${destination:-generic/platform=iOS}"
  signing="-allowProvisioningUpdates"
  product=Release-iphoneos
else
  destination='generic/platform=iOS Simulator'
  signing="CODE_SIGNING_ALLOWED=NO"
  product=Release-iphonesimulator
fi
(cd ios && xcodebuild -project Castle.xcodeproj -scheme Castle -configuration Release -destination "$destination" \
  -derivedDataPath build $signing build)
out="$DIST/Castle-v$VERSION-iOS-$target.app"
rm -rf "$out"
cp -R "ios/build/Build/Products/$product/Castle.app" "$out"
echo "==> $out"
if [ "$target" = device ] && [ -n "${IOS_DEVICE:-}" ]; then
  xcrun devicectl device install app --device "$IOS_DEVICE" "$out"
fi
