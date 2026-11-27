#!/usr/bin/env bash
# Rebuild the app icon set from the SVG sources in src-tauri/icons/source/.
#   scripts/make-icons.sh
# Needs inkscape and cargo-tauri. Writes icon.icns, icon.png, 32x32.png, 128x128.png, 128x128@2x.png
# and tray.png into src-tauri/icons/, and rewrites the favicon data URI in web/index.html from mark.svg.
set -euo pipefail
cd "$(dirname "$0")/../src-tauri/icons"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
inkscape source/icon.svg -o "$tmp/icon.png" -w 1024 -h 1024 >/dev/null 2>&1
inkscape source/tray.svg -o tray.png -w 36 -h 36 >/dev/null 2>&1
(cd "$tmp" && cargo tauri icon icon.png -o out >/dev/null)
for f in icon.icns icon.png 32x32.png 128x128.png 128x128@2x.png; do cp "$tmp/out/$f" "$f"; done
python3 - <<'PY'
import re, urllib.parse
svg = re.sub(r"\s*\n\s*", "", open("source/mark.svg").read().strip()).replace('"', "'")
uri = "data:image/svg+xml," + urllib.parse.quote(svg, safe="/:=' ").replace(" ", "%20")
page = "../../web/index.html"
html = open(page).read()
new, n = re.subn(r'<link rel="icon" type="image/svg\+xml" href="[^"]*">', lambda _: f'<link rel="icon" type="image/svg+xml" href="{uri}">', html, count=1)
if n != 1: raise SystemExit("favicon link not found in web/index.html")
open(page, "w").write(new)
PY
echo "icons and favicon rebuilt"
