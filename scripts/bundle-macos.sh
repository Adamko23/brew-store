#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

if [ "$(uname -s)" != "Darwin" ]; then
    echo 'The application bundle requires macOS.' >&2
    exit 1
fi

sparkle=$(python3 scripts/fetch-sparkle.py)
update_key=$(python3 - <<'PY'
import base64
from pathlib import Path
key = Path('assets/update-public-key.txt').read_text().strip()
if len(base64.b64decode(key, validate=True)) != 32:
    raise SystemExit('Invalid Sparkle public key.')
print(key)
PY
)

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
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources" "$bundle/Contents/Frameworks"
cp "target/$profile/brew-store" "$bundle/Contents/MacOS/brew-store"
cp assets/BrewStore.icns "$bundle/Contents/Resources/BrewStore.icns"
cp LICENSE "$bundle/Contents/Resources/LICENSE"
python3 scripts/collect-licenses.py "$bundle/Contents/Resources/ThirdPartyNotices.txt"
ditto "$sparkle/Sparkle.framework" "$bundle/Contents/Frameworks/Sparkle.framework"
cp "$sparkle/LICENSE" "$bundle/Contents/Resources/Sparkle-LICENSE.txt"
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
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleLocalizations</key><array><string>en</string><string>sk</string></array>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>Brew Store</string>
  <key>SUFeedURL</key><string>https://raw.githubusercontent.com/Adamko23/brew-store/main/appcast.xml</string>
  <key>SUPublicEDKey</key><string>$update_key</string>
  <key>SUEnableAutomaticChecks</key><true/>
  <key>SUAutomaticallyUpdate</key><true/>
  <key>SUScheduledCheckInterval</key><integer>86400</integer>
  <key>SUEnableSystemProfiling</key><false/>
  <key>SUVerifyUpdateBeforeExtraction</key><true/>
  <key>SURequireSignedFeed</key><true/>
</dict></plist>
PLIST
plutil -lint "$bundle/Contents/Info.plist"
identity="${BREW_STORE_SIGN_IDENTITY:--}"
if [ "$identity" = - ]; then
    codesign --force --sign - "$bundle"
else
    # Inside-out signing is required; never use --deep when signing code.
    framework="$bundle/Contents/Frameworks/Sparkle.framework/Versions/B"
    codesign --force --sign "$identity" --options runtime --timestamp "$framework/XPCServices/Installer.xpc"
    codesign --force --sign "$identity" --options runtime --timestamp --preserve-metadata=entitlements "$framework/XPCServices/Downloader.xpc"
    codesign --force --sign "$identity" --options runtime --timestamp "$framework/Autoupdate"
    codesign --force --sign "$identity" --options runtime --timestamp "$framework/Updater.app"
    codesign --force --sign "$identity" --options runtime --timestamp "$bundle/Contents/Frameworks/Sparkle.framework"
    codesign --force --sign "$identity" --options runtime --timestamp "$bundle"
fi
codesign --verify --deep --strict "$bundle"
printf 'Created %s\n' "$bundle"
