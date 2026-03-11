import AppKit
import Foundation

let outputPath = CommandLine.arguments.dropFirst().first ?? "dist/dmg-background.png"
let outputURL = URL(fileURLWithPath: outputPath)
let width = 680.0
let height = 470.0
let size = NSSize(width: width, height: height)

let image = NSImage(size: size)
image.lockFocus()

guard let context = NSGraphicsContext.current?.cgContext else {
  fputs("failed to create graphics context\n", stderr)
  exit(1)
}

let rect = CGRect(origin: .zero, size: size)

let colors = [
  NSColor(calibratedRed: 0.96, green: 0.98, blue: 1.0, alpha: 1.0).cgColor,
  NSColor(calibratedRed: 0.88, green: 0.94, blue: 0.99, alpha: 1.0).cgColor,
]

let gradient = CGGradient(
  colorsSpace: CGColorSpaceCreateDeviceRGB(),
  colors: colors as CFArray,
  locations: [0.0, 1.0]
)!
context.drawLinearGradient(
  gradient,
  start: CGPoint(x: 0, y: height),
  end: CGPoint(x: width, y: 0),
  options: []
)

context.setFillColor(NSColor(calibratedRed: 1.0, green: 1.0, blue: 1.0, alpha: 0.75).cgColor)
let panel = NSBezierPath(roundedRect: NSRect(x: 22, y: 22, width: width - 44, height: height - 44), xRadius: 28, yRadius: 28)
panel.fill()

context.setStrokeColor(NSColor(calibratedRed: 0.73, green: 0.84, blue: 0.95, alpha: 0.9).cgColor)
context.setLineWidth(2)
panel.lineWidth = 2
panel.stroke()

let orbCenterX: CGFloat = 170
let orbCenterY: CGFloat = 244

let arrowColor = NSColor(calibratedRed: 0.38, green: 0.60, blue: 0.84, alpha: 0.48)
arrowColor.setFill()
let arrow = NSBezierPath()
arrow.move(to: CGPoint(x: 286, y: orbCenterY - 7))
arrow.line(to: CGPoint(x: 404, y: orbCenterY - 7))
arrow.line(to: CGPoint(x: 404, y: orbCenterY - 23))
arrow.line(to: CGPoint(x: 444, y: orbCenterY))
arrow.line(to: CGPoint(x: 404, y: orbCenterY + 23))
arrow.line(to: CGPoint(x: 404, y: orbCenterY + 7))
arrow.line(to: CGPoint(x: 286, y: orbCenterY + 7))
arrow.close()
arrow.fill()

let paragraph = NSMutableParagraphStyle()
paragraph.alignment = .center
paragraph.lineSpacing = -1

let titleAttrs: [NSAttributedString.Key: Any] = [
  .font: NSFont.systemFont(ofSize: 34, weight: .semibold),
  .foregroundColor: NSColor(calibratedRed: 0.13, green: 0.24, blue: 0.38, alpha: 1.0),
  .paragraphStyle: paragraph,
]

let subtitleAttrs: [NSAttributedString.Key: Any] = [
  .font: NSFont.systemFont(ofSize: 18, weight: .medium),
  .foregroundColor: NSColor(calibratedRed: 0.28, green: 0.39, blue: 0.52, alpha: 1.0),
  .paragraphStyle: paragraph,
]

let hintAttrs: [NSAttributedString.Key: Any] = [
  .font: NSFont.systemFont(ofSize: 13, weight: .regular),
  .foregroundColor: NSColor(calibratedRed: 0.41, green: 0.50, blue: 0.61, alpha: 1.0),
  .paragraphStyle: paragraph,
]

NSAttributedString(string: "Downshift", attributes: titleAttrs)
  .draw(in: NSRect(x: 50, y: 332, width: 240, height: 48))

NSAttributedString(string: "Drag the app into Applications", attributes: subtitleAttrs)
  .draw(in: NSRect(x: 38, y: 96, width: 280, height: 28))

NSAttributedString(string: "A quiet breathing companion for your desktop.", attributes: hintAttrs)
  .draw(in: NSRect(x: 18, y: 68, width: 320, height: 20))

image.unlockFocus()

guard
  let tiffData = image.tiffRepresentation,
  let bitmap = NSBitmapImageRep(data: tiffData),
  let pngData = bitmap.representation(using: .png, properties: [:])
else {
  fputs("failed to encode png\n", stderr)
  exit(1)
}

try FileManager.default.createDirectory(
  at: outputURL.deletingLastPathComponent(),
  withIntermediateDirectories: true,
  attributes: nil
)
try pngData.write(to: outputURL)
