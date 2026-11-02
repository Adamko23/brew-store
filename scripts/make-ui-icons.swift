// Asset authoring only. The shipped application remains entirely Rust.
import AppKit
import Foundation

let symbols = [
    "search": "magnifyingglass", "installed": "square.stack.3d.up",
    "refresh": "arrow.clockwise", "tools": "wrench.and.screwdriver",
    "open": "arrow.up.forward.app", "update": "arrow.down.circle",
    "update-all": "arrow.down.to.line", "trash": "trash",
    "close": "xmark", "back": "chevron.left", "forward": "chevron.right",
    "external": "arrow.up.right", "log": "text.alignleft",
    "apps": "app", "terminal": "terminal", "all": "square.grid.2x2",
    "warning": "exclamationmark.triangle", "check": "checkmark.circle"
]
let output = URL(fileURLWithPath: "assets/ui-icons", isDirectory: true)
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
for (name, symbol) in symbols {
    guard let original = NSImage(systemSymbolName: symbol, accessibilityDescription: nil),
          let image = original.withSymbolConfiguration(.init(pointSize: 32, weight: .regular)),
          let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 48, pixelsHigh: 48,
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
            colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
          let context = NSGraphicsContext(bitmapImageRep: bitmap) else {
        fatalError("Cannot render SF Symbol: \(symbol)")
    }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    NSColor.clear.setFill()
    NSRect(x: 0, y: 0, width: 48, height: 48).fill()
    let scale = 40.0 / max(image.size.width, image.size.height)
    let size = NSSize(width: image.size.width * scale, height: image.size.height * scale)
    image.draw(in: NSRect(x: (48-size.width)/2, y: (48-size.height)/2,
                         width: size.width, height: size.height),
               from: .zero, operation: .sourceOver, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()
    guard let png = bitmap.representation(using: .png, properties: [:]) else {
        fatalError("Cannot encode \(symbol)")
    }
    try png.write(to: output.appendingPathComponent("\(name).png"))
}
print("Rendered \(symbols.count) SF Symbols masks")
print("System font family: \(NSFont.systemFont(ofSize: 13).familyName ?? "unknown")")
