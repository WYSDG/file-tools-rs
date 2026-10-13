#!/usr/bin/env bash
# Builds File Tools in release mode and installs a launcher so it shows up
# in your app search (Caelestia launcher, etc.). Re-run after every update.
set -euo pipefail
cd "$(dirname "$0")"

BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
ID="dev.local.filetools"

echo "==> Building (release, CPU-capped)…"
./cool.sh build --release

echo "==> Installing…"
install -Dm755 target/release/file-tools "$BIN_DIR/file-tools"
install -Dm644 "packaging/$ID.svg" "$ICON_DIR/$ID.svg"
mkdir -p "$APP_DIR"
# Use the absolute path so the launcher works even if ~/.local/bin isn't on PATH.
sed "s|^Exec=.*|Exec=$BIN_DIR/file-tools %f|" "packaging/$ID.desktop" > "$APP_DIR/$ID.desktop"
update-desktop-database "$APP_DIR" 2>/dev/null || true
gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

echo "==> Checking runtime tools…"
missing=()
for t in magick wl-copy sox exiftool qpdf pdftoppm fc-match; do
  command -v "$t" >/dev/null 2>&1 || missing+=("$t")
done
[ -x "$HOME/bg-remover-env/bin/rembg" ] || missing+=("rembg (expected at ~/bg-remover-env/bin/rembg)")
# Tool icons are SVGs and need gdk-pixbuf's SVG loader (package: librsvg). Without it the app falls back to text-only buttons.
if ! gdk-pixbuf-query-loaders 2>/dev/null | grep -qi svg; then
  echo "   Note: SVG loader not found -> install 'librsvg' to see the tool icons (sudo pacman -S librsvg)"
fi
if [ ${#missing[@]} -gt 0 ]; then
  echo "   Missing (some tabs won't work): ${missing[*]}"
else
  echo "   All runtime tools found."
fi

echo "Done. Search for \"File Tools\" in your launcher (you may need to log out/in once)."
