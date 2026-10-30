#!/bin/sh
# Client Linux 64 bits, à compiler sous Linux (Bevy demande les bibliothèques système: alsa, udev, wayland/x11):
# dist/Castle-v<version>-Linux-x64/ et .zip
. "$(dirname "$0")/common.sh"
[ "$(uname -s)" = Linux ] || { echo "Le client Linux se compile sous Linux." >&2; exit 2; }
cargo build --release -p castle_server -p castle_client
package "Castle-v$VERSION-Linux-x64" linux target/release/castle_client target/release/castle_server
