#!/usr/bin/env bash
# Étape « Rust » du projet Xcode: compile le client (serveur intégré) pour iOS et le place dans l'app.
# Adapté de l'exemple mobile de Bevy (examples/mobile/build_rust_deps.sh).
set -eux

PATH=$PATH:$HOME/.cargo/bin

PROFILE=debug
RELFLAG=
if [[ "$CONFIGURATION" != "Debug" ]]; then
    PROFILE=release
    RELFLAG=--release
fi

# Homebrew (cmake sur les Mac Apple Silicon).
export PATH="$PATH:/opt/homebrew/bin"

# Cargo travaille dans les dossiers de Xcode.
export CARGO_TARGET_DIR="$DERIVED_FILE_DIR/cargo"

# Xcode met sa propre chaîne d'outils en tête du PATH, ce qui casse l'édition de liens
# (`ld: library 'System' not found`, https://github.com/rust-lang/rust/issues/80817).
export PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

IS_SIMULATOR=0
if [ "${LLVM_TARGET_TRIPLE_SUFFIX-}" = "-simulator" ]; then
  IS_SIMULATOR=1
fi

EXECUTABLES=
for arch in $ARCHS; do
  case "$arch" in
    x86_64)
      if [ $IS_SIMULATOR -eq 0 ]; then
        echo "x86_64 n'existe que pour le simulateur." >&2
        exit 2
      fi
      export CFLAGS_x86_64_apple_ios="-target x86_64-apple-ios"
      TARGET=x86_64-apple-ios
      ;;
    arm64)
      if [ $IS_SIMULATOR -eq 0 ]; then
        TARGET=aarch64-apple-ios
      else
        TARGET=aarch64-apple-ios-sim
      fi
      ;;
  esac

  # La cible iOS de Rust s'installe d'elle-même la première fois.
  rustup target add "$TARGET" >/dev/null 2>&1 || true
  (cd "$SRCROOT/.." && cargo build $RELFLAG --target "$TARGET" -p castle_client --bin castle_client)

  EXECUTABLES="$EXECUTABLES $CARGO_TARGET_DIR/$TARGET/$PROFILE/castle_client"
done

# Un seul exécutable (toutes architectures), à l'endroit attendu par Xcode.
lipo -create -output "$TARGET_BUILD_DIR/$EXECUTABLE_PATH" $EXECUTABLES
