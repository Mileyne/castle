# Fonctions communes aux scripts de build (à charger avec `. scripts/common.sh`).
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' client/Cargo.toml | head -1)"
DIST="$ROOT/dist"
mkdir -p "$DIST"

# La chaîne d'outils rustup passe avant un éventuel Rust de Homebrew (qui n'a pas les cibles iOS ni Windows).
if command -v rustup >/dev/null 2>&1; then
  PATH="$(dirname "$(rustup which cargo)"):$PATH"
  export PATH
fi

# package <dossier> <lisez-moi> <fichier>...: assemble un dossier jouable (exécutables, assets, lisez-moi) dans dist/ et
# le zippe.
package() {
  name="$1"
  out="$DIST/$name"
  readme="$2"
  shift 2
  rm -rf "$out" "$out.zip"
  mkdir -p "$out"
  for file in "$@"; do cp "$file" "$out/"; done
  cp -R client/assets "$out/assets"
  find "$out" -name .DS_Store -delete
  sed "s/@VERSION@/$VERSION/" "$ROOT/scripts/lisezmoi/$readme.txt" > "$out/LISEZMOI.txt"
  (cd "$DIST" && zip -qr "$name.zip" "$name")
  echo "==> $out.zip"
}
