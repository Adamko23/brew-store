<p align="center"><img src="assets/brew-store-logo.png" width="96" height="96" alt="Brew Store icon"></p>

# Brew Store

A lightweight macOS desktop app for Homebrew, written in Rust with Iced.
Search for software, see what is actually installed, and manage packages from
one clean interface.

**[Download v0.1.0 for Apple Silicon](https://github.com/Adamko23/brew-store/releases/tag/v0.1.0)**

## Install

1. Download `Brew-Store-0.1.0-macos-arm64.dmg` from the release page.
2. Open it and drag **Brew Store** into **Applications**.
3. Eject the disk image and open the app from Applications.

Requires an **Apple Silicon Mac**, **macOS 13+**, and a working
[Homebrew installation](https://brew.sh). The ZIP contains the same app bundle.
SHA-256 checksums are attached to each release. Intel Macs can build from source.

The first preview is **ad-hoc signed, not notarized by Apple**. macOS may block
its first launch. For a developer-verification warning, Apple's documented
per-app option is **System Settings → Privacy & Security → Open Anyway**.
See [Apple's explanation](https://support.apple.com/102445) before using it.

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
Development has been verified with Rust 1.91.1. Dependencies are pinned in
`Cargo.lock`; the application itself remains Rust.

```sh
cargo run --release --locked
```

Create a locally signed app bundle:

```sh
sh scripts/bundle-macos.sh
open "dist/Brew Store.app"
```

Create a DMG, ZIP and checksum file for the build host's architecture:

```sh
sh scripts/package-macos.sh
```

The package version comes from `Cargo.toml`. To choose a separate output bundle,
set `BREW_STORE_BUNDLE_PATH`. Packaging collects dependency notices offline from
Cargo's downloaded source packages; finish a normal build first.

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
Third-party dependencies retain their licenses; collected notices are included
in the app bundle. Apple's SF Symbols artwork remains subject to Apple's terms,
as described in [the symbol notes](assets/ui-icons/README.md). The system font
is read from macOS and is not bundled.

The native UI was informed by [macos-design](https://github.com/ceorkm/macos-design-skill).
See [the icon source and prompt](assets/ICON.md), [architecture and behavior](docs/v1.md),
and [release notes](docs/releases/v0.1.0.md).
