#!/usr/bin/env bash
# Étape « Rust » du projet Xcode tvOS: compile le client pour l'Apple TV et le place dans l'app.
set -eux
# Xcode ne lit pas le PATH du terminal: rustup (Homebrew, ou installé par rustup.rs) est cherché ici.
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"
"$SRCROOT/build.sh"
cp "$SRCROOT/../target-tvos/aarch64-tvos-as-ios/release/castle_client" "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
