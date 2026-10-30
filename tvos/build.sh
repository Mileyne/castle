#!/usr/bin/env bash
# Compile le client pour l'Apple TV (tvOS) avec une toolchain nightly: la cible est déclarée « os = ios » pour que les
# dépendances prennent leur code iOS; cc.sh / linker.sh remettent les objets C et l'édition de liens sur tvOS.
set -eu
cd "$(dirname "$0")/.."
export PATH="$(dirname "$(rustup +nightly which cargo)"):$PATH"
export SDKROOT="$(xcrun --sdk appletvos --show-sdk-path)" TVOS_DEPLOYMENT_TARGET=15.0 IPHONEOS_DEPLOYMENT_TARGET=15.0
export CC_aarch64_tvos_as_ios="$PWD/tvos/cc.sh" CXX_aarch64_tvos_as_ios="$PWD/tvos/cxx.sh"
exec cargo build -Zbuild-std=std,panic_abort -Zjson-target-spec \
  --config "target.aarch64-tvos-as-ios.linker=\"$PWD/tvos/linker.sh\"" \
  --target tvos/aarch64-tvos-as-ios.json -p castle_client --bin castle_client --release --target-dir target-tvos
