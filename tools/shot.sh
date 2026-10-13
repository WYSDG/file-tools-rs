#!/usr/bin/env bash
# Headless UI screenshot for visual checks (needs: xvfb, imagemagick `import`).
#   tools/shot.sh <theme> <out.png> [image-to-open]
# Runs the debug binary in a virtual display with a throwaway HOME, so it never
# touches your real settings. SYSDARK=0 simulates a light system theme (default: dark).
set -e
theme=${1:-blue}; out=${2:-/tmp/shot.png}; img=${3:-}
root=$(cd "$(dirname "$0")/.." && pwd)
export HOME=$(mktemp -d); mkdir -p "$HOME/.config/file-tools" "$HOME/.config/gtk-4.0"
[ "${SYSDARK:-1}" = "1" ] && printf "[Settings]\ngtk-application-prefer-dark-theme=1\n" > "$HOME/.config/gtk-4.0/settings.ini"
cat > "$HOME/.config/file-tools/settings.json" <<JSON
{"output_dir":null,"strip_metadata":false,"keep_date":true,"keep_camera":true,"keep_gps":false,
 "sound_on":false,"sound_volume":0.5,"default_model":"silueta","live_preview_delay_ms":400,
 "confirm_overwrite":true,"show_tooltips":true,"theme":"$theme"}
JSON
export DISPLAY=:99 GDK_BACKEND=x11 GTK_A11Y=none NO_AT_BRIDGE=1 GSK_RENDERER=cairo
pgrep Xvfb >/dev/null || { Xvfb :99 -screen 0 1500x950x24 >/dev/null 2>&1 & sleep 1; }
"$root/target/debug/file-tools" $img >/dev/null 2>&1 &
pid=$!; sleep "${WAIT:-6}"
import -window root "$out"
kill $pid 2>/dev/null || true
echo "saved $out"
