# Application distribution and updates

## Intent and acceptance

Install a release build made from local sources in Applications and let existing
installations obtain future app releases without manually downloading DMGs.
Keep Search and Installed as the only product destinations. App updates must
verify the publisher and the archive, preserve user data, avoid UI polling and
never disable macOS security globally.

Current preview releases use ad-hoc signing. A locally built app can run locally;
public DMGs remain explicitly non-notarized previews until
a Developer ID certificate and Apple notarization are available. These facts are
separate from cryptographically signed app updates.

## Implementation

- Sparkle 2.10.1 is pinned by SHA-256 in `scripts/fetch-sparkle.py`. It is copied
  into `Contents/Frameworks`, with its license in `Contents/Resources`.
- `src/updater.rs` loads only that bundled framework after the first Iced window
  opens. The controller stays retained on the main thread and adds one native
  menu action. Plain `cargo run` and diagnostics do not require the framework.
- Bundle defaults enable automatic checks/downloads, a daily check interval,
  signed-feed verification and archive verification before extraction. The
  optional initial background check respects Sparkle's stored user preference.
  Sparkle owns scheduling and install-on-quit. No updater render timer is added.
- The public update key is `assets/update-public-key.txt`. Its private key is in
  macOS Keychain under Sparkle account `io.github.adamko23.brew-store`. It is not
  committed, exported to the build tree, stored in GitHub or bundled with the app.
- The HTTPS feed is `https://raw.githubusercontent.com/Adamko23/brew-store/main/appcast.xml`.
  Archives point to immutable, versioned GitHub release assets. Apple Silicon
  and minimum macOS compatibility are included in the generated feed.
- `CFBundleVersion` and `CFBundleShortVersionString` both follow Cargo's numeric
  version. Every public archive needs a new version; never reuse release assets.

Sparkle's Ed25519 signature authenticates an update to existing installations.
It does not certify the developer to Apple or make a new downloaded preview
eligible for the official Homebrew cask repository.

## Build and publish

Build a locally signed bundle with `sh scripts/bundle-macos.sh`. Quit the running
app before copying a replacement to Applications. Verify its nested signature
with `codesign --verify --deep --strict` before launching it.

Publisher previews explicitly use:

```sh
sh scripts/package-macos.sh --preview
```

For normal notarized distribution after obtaining an identity and a configured
`notarytool` Keychain profile:

```sh
BREW_STORE_SIGN_IDENTITY='Developer ID Application: YOUR NAME (TEAMID)' \
  sh scripts/package-macos.sh --notary-profile PROFILE_NAME
```

The Developer ID path signs nested components inside-out, notarizes and staples
the app and DMG, and checks Gatekeeper acceptance. It cannot be exercised without
the publisher's Apple membership, certificate and credentials.

Both paths require the existing Keychain update signing key. Packaging verifies
it matches the embedded public key, produces a signed appcast using Sparkle's
tools, verifies the feed and archive signatures, and refuses to replace existing
versioned output files. Preserve a secure backup of the Keychain. Without a
Developer ID identity, losing this key requires a new manual installation to
establish a replacement update key.

Publish in this order:

1. Review and test the source, bump Cargo's version/lockfile, add release notes,
   commit and tag the version.
2. Package the app. Upload the DMG, ZIP, checksum file and signed `dist/appcast.xml`
   to a draft GitHub release. Verify uploaded digests and anonymous downloads.
3. Publish the release first. Only then copy `dist/appcast.xml` to the repository
   root, commit and push it. This prevents the feed from pointing at absent assets.
4. Update the project tap's cask version and DMG checksum. Mark it `auto_updates`
   because Sparkle can update the installed app independently of Homebrew.
5. Verify a real update using a separate older test bundle. Keep the publisher's
   installed app and unrelated Homebrew packages intact.

## Verification and recovery

Use the existing Rust checks, strict bundle signature checks, version/key/plist
inspection, and actual menu/update tests. Validate signed feeds and update
archives with Sparkle's `sign_update --verify`. A modified archive or feed must
be rejected. Validate the final public downloads against their checksums.

If a release is faulty, withdraw its item from the feed, sign the feed again,
and publish a fixed version with a higher version number. Do not replace a
signed archive in place. Retain the last working bundle before a local install
replacement. Homebrew package state and the app's cache are outside app updates.
