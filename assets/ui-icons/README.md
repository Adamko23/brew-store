# UI symbols

SF Symbols rendered locally through Apple's AppKit by
`scripts/make-ui-icons.swift`. The masks are bundled only for this macOS app;
Apple's applicable SF Symbols / SDK terms apply to the symbol artwork.

The Rust UI caches theme-colored handles. It does not invoke AppKit drawing,
download icons or decode a symbol on every redraw. The Swift script is an asset
authoring helper; the shipped app and all package management remain Rust.
