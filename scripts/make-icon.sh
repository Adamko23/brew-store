#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
iconset='.cache/BrewStore.iconset'
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    doubled=$((size * 2))
    sips -z "$size" "$size" assets/brew-store-icon.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    sips -z "$doubled" "$doubled" assets/brew-store-icon.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o assets/BrewStore.icns
sips -z 128 128 assets/brew-store-icon.png --out assets/brew-store-logo.png >/dev/null
sips -z 512 512 assets/brew-store-icon.png --out assets/brew-store-dock.png >/dev/null
