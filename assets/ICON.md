# Brew Store icon

## Beer and package — 2 November 2026

Edited with the built-in ImageGen tool, using the original package icon as the
edit target. The amber beer tankard connects Brew Store to Homebrew; the raised
ivory package keeps the software-manager cue. The green macOS tile and genuinely
transparent outer padding are retained. No private inventory was sent.

Active source: `brew-store-icon.png`. The selected generated variant is also
saved as `brew-store-icon-beer-v2.png`; the original source remains
`brew-store-icon-package-v1.png`.

`scripts/make-icon.sh` resamples the source to the standard macOS ICNS sizes,
`brew-store-logo.png` (128 px header source) and `brew-store-dock.png` (512 px).
These are static assets; the application performs no image generation at runtime.

Verified: transparency in the master/header/Dock PNGs, native ICNS conversion,
matching bundle ICNS, release build and strict code signature verification. The
new header mark was visually checked in the running app. No Rust behavior changed.

Final prompt (built-in tool, transparent background):

```text
Use case: precise-object-edit
Asset type: final macOS application icon for Brew Store.
Input image: edit target — the existing Brew Store icon. Preserve its deep sage-green rounded-square tile, balanced transparent outer padding, camera angle, premium restrained relief and soft upper-left lighting.
Primary change: replace the current large open parcel and magnifying glass with one beautifully simplified beer tankard, to express the Homebrew connection. A warm amber-gold beer body, a compact ivory foam cap with three broad soft lobes, and one generous rounded handle on the right. Integrate a simple raised warm-ivory open-package glyph into the front face of the tankard, as a subtle secondary software-store cue. The beer mug must dominate the silhouette; the package glyph is part of its surface, not a separate floating badge or extra object.
Composition: one centered strong symbol, comfortably inside the tile; preserve the original tile's size and equal safe padding. Make the result legible at 32 and 48 pixels. Use broad clean geometry, satin materials, quiet highlights, and confident native Mac utility styling.
Constraints: no magnifying glass, no text, no letters, no watermark, no scene, no mockup, no existing company logo, no tiny decorative details. Keep the sage, ivory and amber palette coherent. The area outside the rounded-square tile must remain genuinely transparent.
```

## Original package and magnifying glass — 1 November 2026

Generated with the built-in ImageGen tool on 1 November 2026. The source PNG has
transparency; `scripts/make-icon.sh` creates standard macOS ICNS representations
and the small header logo. No private inventory was sent to ImageGen.

Prompt:

> Create one finished premium macOS application icon for Brew Store, a lightweight
> Homebrew app manager and software search app. A single centered macOS rounded-square
> tile in deep sage green with subtle polished depth and soft natural highlights.
> Main symbol: a beautifully simplified warm ivory open software package box, with a
> small magnifying glass integrated into the package mark to convey finding and
> installing apps. Strong clear silhouette readable at 32px. Refined native Mac
> utility aesthetic, restrained 3D relief, clean geometry, no clutter, no text,
> no letters, no logos of existing companies, no watermark, no mockup or surrounding
> device. Square high resolution composition. Tile occupies about 82 percent of
> canvas width, equal safe padding all sides, transparent outside the tile.

Archived master: `brew-store-icon-package-v1.png`.
