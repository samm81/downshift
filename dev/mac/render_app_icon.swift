import AppKit
import Foundation

let args = Array(CommandLine.arguments.dropFirst())
let sourcePath = args.first ?? "docs/assets/icon.png"
let outputDirPath = args.dropFirst().first ?? "dist/downshift.iconset"
let sourceURL = URL(fileURLWithPath: sourcePath)
let outputDirURL = URL(fileURLWithPath: outputDirPath)

let iconSizes = [
  ("icon_16x16.png", 16.0),
  ("icon_16x16@2x.png", 32.0),
  ("icon_32x32.png", 32.0),
  ("icon_32x32@2x.png", 64.0),
  ("icon_128x128.png", 128.0),
  ("icon_128x128@2x.png", 256.0),
  ("icon_256x256.png", 256.0),
  ("icon_256x256@2x.png", 512.0),
  ("icon_512x512.png", 512.0),
  ("icon_512x512@2x.png", 1024.0),
]

guard let sourceImage = NSImage(contentsOf: sourceURL) else {
  fputs("failed to load source icon at \(sourcePath)\n", stderr)
  exit(1)
}

func renderIcon(size: CGFloat) -> Data? {
  guard let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: Int(size),
    pixelsHigh: Int(size),
    bitsPerSample: 8,
    samplesPerPixel: 4,
    hasAlpha: true,
    isPlanar: false,
    colorSpaceName: .deviceRGB,
    bytesPerRow: 0,
    bitsPerPixel: 0
  ) else {
    return nil
  }

  NSGraphicsContext.saveGraphicsState()
  guard let graphicsContext = NSGraphicsContext(bitmapImageRep: bitmap) else {
    return nil
  }
  NSGraphicsContext.current = graphicsContext
  let context = graphicsContext.cgContext
  context.clear(CGRect(x: 0, y: 0, width: size, height: size))
  sourceImage.draw(in: CGRect(x: 0, y: 0, width: size, height: size))

  NSGraphicsContext.restoreGraphicsState()
  return bitmap.representation(using: .png, properties: [:])
}

try FileManager.default.createDirectory(at: outputDirURL, withIntermediateDirectories: true)

for (filename, size) in iconSizes {
  guard let pngData = renderIcon(size: size) else {
    fputs("failed to render \(filename)\n", stderr)
    exit(1)
  }
  try pngData.write(to: outputDirURL.appendingPathComponent(filename))
}
