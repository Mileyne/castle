#!/bin/sh
# Client macOS (Apple Silicon ou Intel selon le Mac): dist/Castle-v<version>-macOS-<arch>/Castle.app et .zip
. "$(dirname "$0")/common.sh"
cargo build --release -p castle_client
arch="$(uname -m)"
name="Castle-v$VERSION-macOS-$arch"
out="$DIST/$name"
app="$out/Castle.app"
rm -rf "$out" "$out.zip"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/castle_client "$app/Contents/MacOS/"
cp -R client/assets "$app/Contents/Resources/assets"
cp scripts/macos/Castle.icns "$app/Contents/Resources/"
sed "s/@VERSION@/$VERSION/g" scripts/macos/Info.plist > "$app/Contents/Info.plist"
find "$out" -name .DS_Store -delete
# Signature ad hoc: sans elle, Apple Silicon refuse de lancer le programme modifie apres compilation.
codesign --force --deep --sign - "$app"
sed "s/@VERSION@/$VERSION/" scripts/lisezmoi/macos.txt > "$out/LISEZMOI.txt"
(cd "$DIST" && zip -qry "$name.zip" "$name")
echo "==> $out.zip"
