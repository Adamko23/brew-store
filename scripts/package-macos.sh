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
preview=false
notary_profile=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --preview) preview=true; shift ;;
        --notary-profile)
            [ "$#" -ge 2 ] || { echo '--notary-profile requires a Keychain profile name.' >&2; exit 1; }
            notary_profile=$2; shift 2 ;;
        *) echo 'Usage: package-macos.sh --preview | --notary-profile PROFILE' >&2; exit 1 ;;
    esac
done
identity="${BREW_STORE_SIGN_IDENTITY:--}"
if [ "$preview" = false ] && { [ "$identity" = - ] || [ -z "$notary_profile" ]; }; then
    echo 'Public releases require BREW_STORE_SIGN_IDENTITY and --notary-profile PROFILE.' >&2
    echo 'Use --preview explicitly for an ad-hoc, non-notarized preview.' >&2
    exit 1
fi
if [ "$preview" = true ] && [ -n "$notary_profile" ]; then
    echo 'Choose either --preview or --notary-profile.' >&2
    exit 1
fi
python3 scripts/generate-appcast.py --check-key
work=$(mktemp -d "${TMPDIR:-/tmp}/brew-store-release.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM
stage="$work/disk"
output="$work/output"
mkdir -p "$stage" "$output"
mkdir -p dist
BREW_STORE_BUNDLE_PATH="$stage/Brew Store.app" sh scripts/bundle-macos.sh

binary="$stage/Brew Store.app/Contents/MacOS/brew-store"
architecture=$(lipo -archs "$binary")
case "$architecture" in
    arm64) ;;
    *) echo "Unsupported release architecture: $architecture" >&2; exit 1 ;;
esac
actual=$("$binary" --version)
if [ "$actual" != "Brew Store $version" ]; then
    echo 'The binary and release versions do not match.' >&2
    exit 1
fi
codesign --verify --deep --strict "$stage/Brew Store.app"
name="Brew-Store-$version-macos-$architecture"
for file in "$name.zip" "$name.dmg" "$name-SHA256SUMS.txt"; do
    if [ -e "dist/$file" ]; then
        echo "Refusing to replace existing release artifact dist/$file." >&2
        exit 1
    fi
done

if [ "$preview" = false ]; then
    ditto -c -k --sequesterRsrc --keepParent "$stage/Brew Store.app" "$work/notarization.zip"
    xcrun notarytool submit "$work/notarization.zip" --keychain-profile "$notary_profile" --wait --output-format json > "$work/notary-app.json"
    python3 -c 'import json,sys; result=json.load(open(sys.argv[1])); assert result.get("status")=="Accepted", result' "$work/notary-app.json"
    xcrun stapler staple "$stage/Brew Store.app"
    spctl --assess --type execute "$stage/Brew Store.app"
fi
ditto -c -k --sequesterRsrc --keepParent "$stage/Brew Store.app" "$output/$name.zip"
ln -s /Applications "$stage/Applications"
cp docs/INSTALL.txt "$stage/INSTALL.txt"
hdiutil create -volname "Brew Store $version" -srcfolder "$stage" -fs HFS+ -format UDZO "$output/$name.dmg"
if [ "$preview" = false ]; then
    codesign --force --sign "$identity" --timestamp "$output/$name.dmg"
    xcrun notarytool submit "$output/$name.dmg" --keychain-profile "$notary_profile" --wait --output-format json > "$work/notary-dmg.json"
    python3 -c 'import json,sys; result=json.load(open(sys.argv[1])); assert result.get("status")=="Accepted", result' "$work/notary-dmg.json"
    xcrun stapler staple "$output/$name.dmg"
fi
hdiutil verify "$output/$name.dmg"
python3 scripts/generate-appcast.py "$output/$name.zip"
(
    cd "$output"
    shasum -a 256 "$name.dmg" "$name.zip" appcast.xml > "$name-SHA256SUMS.txt"
)
mv "$output/$name.dmg" "$output/$name.zip" "$output/$name-SHA256SUMS.txt" "$output/appcast.xml" dist/
printf 'Release ready: dist/%s.dmg\n' "$name"
