#!/bin/sh
# Client macOS (Apple Silicon ou Intel selon le Mac): dist/Castle-v<version>-macOS-<arch>/ et .zip
. "$(dirname "$0")/common.sh"
cargo build --release -p castle_server -p castle_client
arch="$(uname -m)"
package "Castle-v$VERSION-macOS-$arch" macos target/release/castle_client target/release/castle_server
