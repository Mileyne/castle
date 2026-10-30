#!/usr/bin/env bash
# Étape « Rust » du projet Xcode tvOS: compile le client (serveur intégré) pour l'Apple TV et le place dans l'app.
set -eux
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
"$SRCROOT/build.sh"
cp "$SRCROOT/../target-tvos/aarch64-tvos-as-ios/release/castle_client" "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
