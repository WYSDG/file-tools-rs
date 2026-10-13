#!/usr/bin/env bash
# Run the app headless, perform scripted mouse/keyboard steps, and screenshot.
#   EXTRA='xdotool mousemove 96 580 click 1; sleep 2; import -window root /tmp/a.png' \
#     tools/interact.sh <theme> [image]
# Needs: xvfb xdotool imagemagick openbox (see NOTES.md §3). Uses a throwaway HOME.
# Coordinates are for a 1500x950 virtual screen with a window manager (title bar ≈ 20px).
# The rail layout shifts whenever tools/icons change — take a screenshot first and re-measure.
theme=${1:-dark}; img=${2:-}
root=$(cd "$(dirname "$0")/.." && pwd)
export HOME=$(mktemp -d); mkdir -p "$HOME/.config/file-tools" "$HOME/.config/gtk-4.0" "$HOME/Pictures/FileTools"
printf "[Settings]\ngtk-application-prefer-dark-theme=1\n" > "$HOME/.config/gtk-4.0/settings.ini"
cat > "$HOME/.config/file-tools/settings.json" <<JSON
{"output_dir":null,"strip_metadata":false,"keep_date":true,"keep_camera":true,"keep_gps":false,
 "sound_on":false,"sound_volume":0.5,"default_model":"silueta","live_preview_delay_ms":500,
 "confirm_overwrite":false,"show_tooltips":true,"theme":"$theme","theme_v":2}
JSON
export DISPLAY=:99 GDK_BACKEND=x11 GTK_A11Y=none NO_AT_BRIDGE=1 GSK_RENDERER=cairo
pgrep Xvfb >/dev/null || { Xvfb :99 -screen 0 1500x950x24 >/dev/null 2>&1 & sleep 1; }
pgrep openbox >/dev/null || { openbox >/dev/null 2>&1 & sleep 1; }
"$root/target/debug/file-tools" $img >/tmp/interact.log 2>&1 &
pid=$!; sleep "${WAIT:-6}"
eval "${EXTRA:-}"
kill $pid 2>/dev/null; wait $pid 2>/dev/null; true
