#!/usr/bin/env bash
# Rebuild the app icon set from the SVG sources in src-tauri/icons/source/.
#   scripts/make-icons.sh
# Needs inkscape and iconutil (macOS). Writes icon.icns, icon.png (512), 32x32.png, 128x128.png,
# 128x128@2x.png and tray.png into src-tauri/icons/, and rewrites the favicon data URI in web/index.html
# from mark.svg. Every output is a pure function of the SVGs: running it twice gives identical bytes.
set -euo pipefail
cd "$(dirname "$0")/../src-tauri/icons"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
render() { inkscape source/icon.svg -o "$2" -w "$1" -h "$1" >/dev/null 2>&1; }
set_dir="$tmp/icon.iconset"
mkdir "$set_dir"
for n in 16 32 128 256 512; do
  render "$n" "$set_dir/icon_${n}x${n}.png"
  render "$((n * 2))" "$set_dir/icon_${n}x${n}@2x.png"
done
iconutil -c icns "$set_dir" -o icon.icns
cp "$set_dir/icon_512x512.png" icon.png
cp "$set_dir/icon_32x32.png" 32x32.png
cp "$set_dir/icon_128x128.png" 128x128.png
cp "$set_dir/icon_128x128@2x.png" 128x128@2x.png
inkscape source/tray.svg -o tray.png -w 36 -h 36 >/dev/null 2>&1
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
