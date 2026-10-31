#!/bin/sh
# Client Windows 64 bits, compilé depuis un Mac ou un Linux avec mingw-w64 (macOS: `brew install mingw-w64`):
# dist/Castle-v<version>-Windows-x64/ et .zip
. "$(dirname "$0")/common.sh"
command -v x86_64-w64-mingw32-gcc >/dev/null || { echo "mingw-w64 est requis (brew install mingw-w64)" >&2; exit 2; }
rustup target add x86_64-pc-windows-gnu >/dev/null
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++
export AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar
cargo build --release --target x86_64-pc-windows-gnu -p castle_client
package "Castle-v$VERSION-Windows-x64" windows target/x86_64-pc-windows-gnu/release/castle_client.exe
