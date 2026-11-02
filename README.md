<p align="center"><img src="assets/brew-store-logo.png" width="96" height="96" alt="Brew Store icon"></p>

# Brew Store

A lightweight macOS desktop app for Homebrew, written in Rust with Iced.
Search for software, see what is actually installed, and manage packages from
one clean interface.

**[Install with Homebrew](#install)** ·
**[Download v0.1.1 for Apple Silicon](https://github.com/Adamko23/brew-store/releases/tag/v0.1.1)**

## Install

Requires an **Apple Silicon Mac**, **macOS 13+**, and a working
[Homebrew installation](https://brew.sh). Intel Macs can build from source.

Install through the [project's Homebrew tap](https://github.com/Adamko23/homebrew-tap):

```sh
brew install --cask adamko23/tap/brew-store
```

Homebrew adds the tap automatically, downloads the published DMG and verifies
its SHA-256 checksum. To update an installation managed by Homebrew:

```sh
brew update
brew upgrade --cask adamko23/tap/brew-store
```

To uninstall, run `brew uninstall --cask adamko23/tap/brew-store`.

For a manual installation:

1. Download `Brew-Store-0.1.1-macos-arm64.dmg` from the release page.
2. Open it and drag **Brew Store** into **Applications**.
3. Eject the disk image and open the app from Applications.

The ZIP contains the same app bundle. SHA-256 checksums are attached to each release.

The first preview is **ad-hoc signed, not notarized by Apple**. macOS may block
its first launch. For a developer-verification warning, Apple's documented
per-app option is **System Settings → Privacy & Security → Open Anyway**.
See [Apple's explanation](https://support.apple.com/102445) before using it.

The project tap is maintained by Adamko23. Brew Store is not currently listed in
the official `homebrew/cask` catalogue. Installing through Homebrew preserves
macOS quarantine and does not remove the first-launch developer warning.

## Automatic app updates

Starting with **0.1.1**, Brew Store includes [Sparkle](https://sparkle-project.org).
It checks for app updates on launch and daily while running, downloads them in
the background, and installs them when the app quits when no authorization is
needed. Updates use HTTPS, signed feeds and Ed25519 archive signatures verified
before extraction. The updater does not add an Iced render timer or enable system
profiling. Sparkle respects update preferences changed by the user.

Use **Brew Store → Check for App Updates…** for a manual check. The version 0.1.0
preview needs a one-time update to 0.1.1 to gain this feature. Apple notarization
and Sparkle update signatures are separate: the latter protect future updates
and do not remove the first-launch warning on a newly downloaded preview.

## What it does

- **Search:** live results from a cached public Homebrew catalogue, package
  details and confirmed installation. No network requests or Brew process on
  each keystroke.
- **Installed:** real formulae and casks from this Mac, installed versions,
  available updates, local application icons and measured installation sizes.
- Open existing apps, update or uninstall a package, and run **Update All** as
  one general `brew upgrade` command.
- Inspect recorded dependencies and installed consumers, keeping formula,
  cask and tap identities separate.
- Follow macOS light/dark appearance and preferred language: **English/Slovak**.
  Package descriptions and Homebrew output remain in their original language.
- Diagnose Command Line Tools issues and install an eligible Apple update
  after confirmation and local administrator authentication.

There are two navigation destinations: Search and Installed. Categories are not
inferred from package names. Details occupy the main window with a back action.

## Data and package management

Homebrew owns the installed state. The app reads `brew info --json=v2 --installed`
and `brew outdated --json=v2` at startup, after operations, on manual refresh and
when returning to the window. Refresh reads metadata; it does not update packages.

Package changes require confirmation and run serially with bounded output logs.
Update All follows normal Homebrew metadata refresh, pin and self-updater rules.
A failed command can still have changed packages; fresh inventory is reconciled
before presenting the result. Individual removals disable automatic removal of
unrelated orphaned dependencies and verify that the selected record disappeared.

A formula whose definition disappeared or migrated to a cask can still be removed
using its freshly verified installed identity. A cask with a missing declared app
bundle offers guarded record cleanup through Homebrew; it does not delete user
data directly or use `--zap`.

Icons come from declared local app bundles. Sizes use allocated installation
space, excluding user documents and other external app data. Unknown paths or
sizes remain unknown. Recorded dependencies cannot identify use outside Brew.

The public catalogue, derived icons and authentication helper are cached under
`~/Library/Caches/local.brew-store/`. Installed inventory is not uploaded.
Administrator passwords pass from the local helper directly to sudo and are
not stored in the Rust application or its logs. Homebrew itself can request
administrator privileges for packages that need them.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| ⌘F | Focus search |
| ⌘1 / ⌘2 | Search / Installed |
| ⌘R | Refresh data |
| ⌘O | Open selected app or the only matching app |
| ⌘⇧U | Review Update All |
| ⌘, | Apple tools |
| Esc | Cancel, close utility sheet or return from details |
| Tab / Shift+Tab | Move focus |

## Build from source

Use macOS with Command Line Tools, Rust 1.88+ and Python 3.9+ for packaging.
Development has been verified with Rust 1.91.1. Rust dependencies are pinned in
`Cargo.lock`; the application and UI remain Rust. Bundles include the native
Sparkle update framework, downloaded at a pinned version and checksum into
`.cache/sparkle/` by the bundle script.

```sh
cargo run --release --locked
```

Create a locally signed app bundle:

```sh
sh scripts/bundle-macos.sh
open "dist/Brew Store.app"
```

Release packaging also requires the publisher's update signing key in macOS
Keychain. For an explicitly non-notarized Apple Silicon preview:

```sh
sh scripts/package-macos.sh --preview
```

The package version comes from `Cargo.toml`. To choose a separate output bundle,
set `BREW_STORE_BUNDLE_PATH`. Packaging collects dependency notices offline from
Cargo's downloaded source packages; finish a normal build first.
Normal public packaging requires a Developer ID identity and notarization
profile; [the update and release guide](docs/updates.md) explains both paths.

## Validate

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

The behavioral tests use fake processes for package changes and Apple tool
installation. The test host needs macOS and Homebrew discovery; tests do not
change its installed packages.

Read-only diagnostics and cached search benchmark:

```sh
cargo run --release --locked -- --diagnose --data-dir .cache/diagnostic
cargo run --release --locked -- --benchmark-search --data-dir .cache/diagnostic
```

`--version` prints the compiled version. `--theme light|dark|auto` and
`--language en|sk|auto` are QA overrides; normal startup uses macOS preferences.

## Current limits

This is the first public preview. VoiceOver semantics are incomplete. Only known
minimum macOS requirements can be preflighted; Homebrew performs other platform
checks. The log keeps its latest 400 lines. No background idle render timer or
recurring developer-tools scan is used.

## License and assets

Brew Store's source and original artwork are [MIT licensed](LICENSE).
Third-party dependencies retain their licenses; collected Rust notices and the
Sparkle license are included in the app bundle. Apple's SF Symbols artwork remains subject to Apple's terms,
as described in [the symbol notes](assets/ui-icons/README.md). The system font
is read from macOS and is not bundled.

The native UI was informed by [macos-design](https://github.com/ceorkm/macos-design-skill).
See [the icon source and prompt](assets/ICON.md), [architecture and behavior](docs/v1.md),
and [release notes](docs/releases/v0.1.1.md).
