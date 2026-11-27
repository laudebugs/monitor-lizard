#!/usr/bin/env bash
# Rebuild the app icon set from the SVG sources in src-tauri/icons/source/.
#   scripts/make-icons.sh
# Needs inkscape and cargo-tauri. Writes icon.icns, icon.png, 32x32.png, 128x128.png, 128x128@2x.png
# and tray.png into src-tauri/icons/. The favicon in web/index.html is a data URI of mark.svg:
# paste it by hand after changing mark.svg (see docs/agents/layout.md).
set -euo pipefail
cd "$(dirname "$0")/../src-tauri/icons"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
inkscape source/icon.svg -o "$tmp/icon.png" -w 1024 -h 1024 >/dev/null 2>&1
inkscape source/tray.svg -o tray.png -w 36 -h 36 >/dev/null 2>&1
(cd "$tmp" && cargo tauri icon icon.png -o out >/dev/null)
for f in icon.icns icon.png 32x32.png 128x128.png 128x128@2x.png; do cp "$tmp/out/$f" "$f"; done
echo "icons rebuilt in src-tauri/icons"
