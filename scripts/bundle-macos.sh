#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

if [ "$(uname -s)" != "Darwin" ]; then
    echo 'The application bundle requires macOS.' >&2
    exit 1
fi

if [ "${1:-}" = "--debug" ]; then
    cargo build --locked
    profile=debug
else
    cargo build --release --locked
    profile=release
fi

version=$(sed -n 's/^version = "\([0-9][0-9.]*\)"$/\1/p' Cargo.toml | head -n 1)
if [ -z "$version" ]; then
    echo 'Cargo.toml must contain a numeric package version.' >&2
    exit 1
fi
bundle="${BREW_STORE_BUNDLE_PATH:-dist/Brew Store.app}"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "target/$profile/brew-store" "$bundle/Contents/MacOS/brew-store"
cp assets/BrewStore.icns "$bundle/Contents/Resources/BrewStore.icns"
cp LICENSE "$bundle/Contents/Resources/LICENSE"
python3 scripts/collect-licenses.py "$bundle/Contents/Resources/ThirdPartyNotices.txt"
cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Brew Store</string>
  <key>CFBundleDisplayName</key><string>Brew Store</string>
  <key>CFBundleIdentifier</key><string>io.github.adamko23.brew-store</string>
  <key>CFBundleExecutable</key><string>brew-store</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleIconFile</key><string>BrewStore</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleLocalizations</key><array><string>en</string><string>sk</string></array>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>Brew Store</string>
</dict></plist>
PLIST
plutil -lint "$bundle/Contents/Info.plist"
codesign --force --sign - "$bundle"
printf 'Created %s\n' "$bundle"
