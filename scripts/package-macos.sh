#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

if [ "$(uname -s)" != Darwin ]; then
    echo 'Release packaging requires macOS.' >&2
    exit 1
fi

version=$(sed -n 's/^version = "\([0-9][0-9.]*\)"$/\1/p' Cargo.toml | head -n 1)
if [ -z "$version" ]; then
    echo 'Cargo.toml must contain a numeric package version.' >&2
    exit 1
fi
stage=$(mktemp -d "${TMPDIR:-/tmp}/brew-store-release.XXXXXX")
trap 'rm -rf "$stage"' EXIT HUP INT TERM
mkdir -p dist
BREW_STORE_BUNDLE_PATH="$stage/Brew Store.app" sh scripts/bundle-macos.sh

binary="$stage/Brew Store.app/Contents/MacOS/brew-store"
architecture=$(lipo -archs "$binary")
case "$architecture" in
    arm64|x86_64) ;;
    *) echo "Unsupported release architecture: $architecture" >&2; exit 1 ;;
esac
actual=$("$binary" --version)
if [ "$actual" != "Brew Store $version" ]; then
    echo 'The binary and release versions do not match.' >&2
    exit 1
fi
codesign --verify --deep --strict "$stage/Brew Store.app"
name="Brew-Store-$version-macos-$architecture"

ditto -c -k --sequesterRsrc --keepParent "$stage/Brew Store.app" "dist/$name.zip"
ln -s /Applications "$stage/Applications"
cp docs/INSTALL.txt "$stage/INSTALL.txt"
hdiutil create -volname "Brew Store $version" -srcfolder "$stage" -ov -fs HFS+ -format UDZO "dist/$name.dmg"
hdiutil verify "dist/$name.dmg"
(
    cd dist
    shasum -a 256 "$name.dmg" "$name.zip" > "$name-SHA256SUMS.txt"
)
printf 'Release ready: dist/%s.dmg\n' "$name"
