# Brew Store

macOS desktop app, Rust + Iced 0.14 with Sparkle for native app updates. Current scope and evidence:
[docs/v1.md](docs/v1.md). Preserve the two-screen product: Search and Installed.
No inferred categories, dashboards or separate update/history sections.

## Commands

- `cargo fmt --check`
- `cargo test --locked` — behavioral tests use fake executables for mutations.
- `cargo clippy --locked --all-targets -- -D warnings`
- `sh scripts/bundle-macos.sh` — release bundle, local ad-hoc signature.
- `sh scripts/package-macos.sh --preview` — non-notarized Apple Silicon DMG,
  ZIP, checksums and signed appcast. Requires the publisher's Keychain update key.
  Normal packaging needs Developer ID and `--notary-profile PROFILE`; see
  [docs/updates.md](docs/updates.md). Bundling fetches a pinned Sparkle binary.
- `cargo run --release --locked -- --diagnose --data-dir .cache/diagnostic` — reads
  real Homebrew state and metadata, never changes packages.
- `cargo run --release --locked -- --benchmark-search --data-dir .cache/diagnostic`
  measures the actual cached catalogue without network requests.
- `sh scripts/make-icon.sh` — rebuild ICNS/header PNG from the checked-in master.
  macOS iconutil may require an unsandboxed asset conversion.
- `swift scripts/make-ui-icons.swift` — author the bundled SF Symbols masks with
  AppKit. Swift is an asset-generation tool, not an application runtime.

Dependencies are pinned in Cargo.lock. Rust 1.91.1 is available locally. Prefer
release builds when measuring performance. Do not compare compile CPU with app CPU.

## Boundaries

- `model.rs`: tolerant Homebrew JSON, package identities and prepared text index.
- `catalog.rs`: public API and atomic public-only cache; no local inventory in cache.
- `brew.rs`: actual CLI inventory/details, validated argument arrays, serial writes.
- `assets.rs`: declared app paths, bounded icons/sizing and validated `open -a`.
- `dependencies.rs`: installed typed dependency graph, rebuilt only with inventory.
- `native.rs`: Dock icon and native language/decimal preferences.
- `updater.rs`: main-thread Sparkle controller and native menu; Sparkle owns
  verification, scheduling and replacement. Never add an Iced update timer.
- `i18n.rs` / `locales/en.json`: Slovak source strings, English translations.
- `tools.rs`: recognize CLT diagnostics and use Apple-supported repair paths.
- `search.rs`: pure prepared search, dispatched off the UI thread.
- `app.rs`: application state/tasks; `ui/`: presentation and keyboard controls.

## Invariants and pitfalls

- Homebrew is authoritative. Keep installed data when refresh fails, but disable
  mutations until a successful current inventory is available.
- Search has a single layout for empty input and live results. Input schedules a
  cancellable 180 ms background search, retaining existing rows until the new
  result is ready. Enter only flushes the debounce; it never navigates. Share the
  catalogue, prepare tokens once and reject stale generations.
- Do not scan all installed directories on startup. Load visible assets lazily and
  cap concurrency. An unchanged inventory must not rebuild the catalogue.
- Use the GPU/Metal renderer. The initial tiny-skia debug build was measured near
  100% CPU; profile the actual process before making performance claims.
- Cache custom theme objects; follow native system appearance and update semantic
  text/surface colors together. Do not add a render timer when idle.
- Two destinations use a compact, stable segmented toolbar, without a sidebar.
  Use cached SF Symbols masks for familiar actions, localized tooltips and focus
  feedback. Keep full-page details. Load the system font from macOS, never bundle it.
- Keep interactive controls below the native titlebar (`fullsize_content_view`
  false). CUA reproduced AppKit swallowing clicks in controls painted underneath.
- Follow the ordered macOS preferred languages (English/Slovak, English fallback),
  not the region. Localize app copy, preserve original Brew output/package data.
- Homebrew cask fields may be null, including tap and auto_updates. Do not replace
  legitimate unknown data with fabricated versions, sizes or icons.
- Formula/cask and third-party-tap identities must remain distinct. Prefer installed
  runtime receipts for formula dependencies. No recorded consumers does not prove
  a package is unused outside Homebrew.
- Formula runtime receipts include recursive entries. Only declared_directly=true
  is a direct declaration; false or missing belongs in the additional runtime
  group. Do not fabricate a precise chain from legacy or undeclared relationships.
- Never interpolate package names into a shell or use zap/ignore-dependencies.
  The only force exception is RemoveStale for a cask: freshly verify all declared
  app paths are absent, invoke brew uninstall --cask --force, and verify the record
  disappears. Never directly delete Homebrew directories or user data.
- Read-only diagnostics do not authorize installing/removing/updating real software.
- A tap can migrate a formula to a cask while the old formula is still installed.
  Before formula uninstall, re-read installed inventory, require one matching
  formula token with the exact original tap/key, then use `--formula TOKEN`.
  A qualified name may fail Homebrew's definition lookup before reaching its keg.
  Never switch to a same-named cask or another tap. Check the record disappeared.
- Set HOMEBREW_NO_AUTOREMOVE=1 for individual removals: Homebrew otherwise may
  remove unrelated orphaned dependencies even after the selected removal fails.
- When current detail lookup fails, use only an exact typed installed receipt,
  mark its version as recorded at installation and explain the fallback in UI.
- Update All runs one `brew upgrade` with normal Homebrew metadata refresh policy;
  no greedy or pin bypass. Preview may open during a background refresh, but
  confirmation cannot execute until fresh inventory has been verified.
- Open is available only for a real declared installed .app. Validate the path
  again before calling `/usr/bin/open -a` with separate literal arguments.
- A failed process can have applied a package change. Preserve the exit error,
  then reconcile fresh inventory against the expected target. Explain a confirmed
  change followed by failure; only offer retry while the action is still needed.
- Apple tool repair may install only a validated compatible CLT label returned by
  softwareupdate, after in-app confirmation. Never install all updates, restart,
  agree to licenses automatically, delete CLT directories or silently update macOS.
  If no CLT package is offered, show installer/settings/Apple download fallbacks.
  An offered macOS update alone does not prove CLT requires a newer OS.
- Show and enforce known cask minimum macOS requirements. Unknown requirements
  remain unknown; Homebrew performs the final platform checks.
- Use CUA for native window inspection. This app is not a browser page. Browser work,
  if needed elsewhere, uses the user's in-app browser preference.
- Preserve unrelated changes; do not delegate or create chats without authorization.
- Quit the running app before replacing its signed bundle during development.
- Never export/commit the update private key. Public feeds and archives must be
  signed with the key matching `assets/update-public-key.txt`. Publish archives
  before the feed. Archive signatures do not substitute for Apple notarization.
