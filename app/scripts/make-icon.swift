import AppKit
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let canvas: CGFloat = 1024

func color(_ hex: UInt32, alpha: CGFloat = 1) -> CGColor {
    CGColor(
        srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
        green: CGFloat((hex >> 8) & 0xFF) / 255,
        blue: CGFloat(hex & 0xFF) / 255,
        alpha: alpha
    )
}

let bodyYellow = color(0xF2B33D)
let rust = color(0xA85420)
let stain = color(0x6E2E12)
let eyeGrey = color(0xAAAAA0)
let backgroundTop = color(0x34302B)
let backgroundBottom = color(0x1D1B19)

func linearGradient(_ colors: [CGColor], _ locations: [CGFloat]) -> CGGradient {
    CGGradient(
        colorsSpace: CGColorSpace(name: CGColorSpace.sRGB),
        colors: colors as CFArray,
        locations: locations
    )!
}

struct SeededRandom {
    var state: UInt64

    mutating func next() -> CGFloat {
        state = state &* 6364136223846793005 &+ 1442695040888963407
        return CGFloat((state >> 33) % 1_000_000) / 1_000_000
    }
}

let tile = CGRect(x: 100, y: 100, width: 824, height: 824)
let tileRadius: CGFloat = 185

let body = CGRect(x: 232, y: 536, width: 560, height: 330)
let bodyRadius: CGFloat = 46

let leftEyeCenter = CGPoint(x: 360, y: 340)
let rightEyeCenter = CGPoint(x: 664, y: 340)
let eyeRadius: CGFloat = 128

func drawTile(in context: CGContext) {
    context.saveGState()
    context.setShadow(offset: CGSize(width: 0, height: 14), blur: 28, color: color(0x000000, alpha: 0.45))
    context.addPath(CGPath(roundedRect: tile, cornerWidth: tileRadius, cornerHeight: tileRadius, transform: nil))
    context.setFillColor(backgroundBottom)
    context.fillPath()
    context.restoreGState()

    context.saveGState()
    context.addPath(CGPath(roundedRect: tile, cornerWidth: tileRadius, cornerHeight: tileRadius, transform: nil))
    context.clip()
    context.drawLinearGradient(
        linearGradient([backgroundTop, backgroundBottom], [0, 1]),
        start: CGPoint(x: 0, y: tile.minY),
        end: CGPoint(x: 0, y: tile.maxY),
        options: []
    )
    context.drawRadialGradient(
        linearGradient([color(0xF2B33D, alpha: 0.16), color(0xF2B33D, alpha: 0)], [0, 1]),
        startCenter: CGPoint(x: 512, y: 380), startRadius: 0,
        endCenter: CGPoint(x: 512, y: 380), endRadius: 460,
        options: []
    )
    context.restoreGState()
}

func drawNeck(in context: CGContext) {
    let neck = CGRect(x: 494, y: 400, width: 36, height: 150)
    context.saveGState()
    context.clip(to: neck)
    context.drawLinearGradient(
        linearGradient([color(0x6E6E66), color(0xB4B4AA), color(0x55554F)], [0, 0.4, 1]),
        start: CGPoint(x: neck.minX, y: 0),
        end: CGPoint(x: neck.maxX, y: 0),
        options: []
    )
    context.restoreGState()

    let bridge = CGRect(x: leftEyeCenter.x + 80, y: leftEyeCenter.y - 20, width: rightEyeCenter.x - leftEyeCenter.x - 160, height: 40)
    context.saveGState()
    context.addPath(CGPath(roundedRect: bridge, cornerWidth: 12, cornerHeight: 12, transform: nil))
    context.clip()
    context.drawLinearGradient(
        linearGradient([color(0xC4C4BA), color(0x8A8A82), color(0x5E5E58)], [0, 0.5, 1]),
        start: CGPoint(x: 0, y: bridge.minY),
        end: CGPoint(x: 0, y: bridge.maxY),
        options: []
    )
    context.restoreGState()
}

func drawEye(at center: CGPoint, in context: CGContext) {
    let outer = CGRect(x: center.x - eyeRadius, y: center.y - eyeRadius, width: eyeRadius * 2, height: eyeRadius * 2)

    context.saveGState()
    context.setShadow(offset: CGSize(width: 0, height: 10), blur: 18, color: color(0x000000, alpha: 0.5))
    context.addEllipse(in: outer)
    context.setFillColor(eyeGrey)
    context.fillPath()
    context.restoreGState()

    context.saveGState()
    context.addEllipse(in: outer)
    context.clip()
    context.drawLinearGradient(
        linearGradient([color(0xD6D6CC), eyeGrey, color(0x6A6A62)], [0, 0.45, 1]),
        start: CGPoint(x: 0, y: outer.minY),
        end: CGPoint(x: 0, y: outer.maxY),
        options: []
    )
    context.restoreGState()

    let rimRadius = eyeRadius * 0.78
    context.addEllipse(in: CGRect(x: center.x - rimRadius, y: center.y - rimRadius, width: rimRadius * 2, height: rimRadius * 2))
    context.setFillColor(color(0x55554F))
    context.fillPath()

    let lensRadius = eyeRadius * 0.68
    let lens = CGRect(x: center.x - lensRadius, y: center.y - lensRadius, width: lensRadius * 2, height: lensRadius * 2)
    context.saveGState()
    context.addEllipse(in: lens)
    context.clip()
    context.drawRadialGradient(
        linearGradient([color(0x3A3F44), color(0x16181A)], [0, 1]),
        startCenter: CGPoint(x: center.x, y: center.y + 10), startRadius: 0,
        endCenter: center, endRadius: lensRadius,
        options: [.drawsAfterEndLocation]
    )
    context.restoreGState()

    let irisRadius = eyeRadius * 0.34
    context.addEllipse(in: CGRect(x: center.x - irisRadius, y: center.y - irisRadius, width: irisRadius * 2, height: irisRadius * 2))
    context.setStrokeColor(color(0x7C8288))
    context.setLineWidth(eyeRadius * 0.06)
    context.strokePath()

    let pupilRadius = eyeRadius * 0.16
    context.addEllipse(in: CGRect(x: center.x - pupilRadius, y: center.y - pupilRadius, width: pupilRadius * 2, height: pupilRadius * 2))
    context.setFillColor(color(0x0A0B0C))
    context.fillPath()

    let glint = CGRect(x: center.x - lensRadius * 0.55, y: center.y - lensRadius * 0.62, width: lensRadius * 0.42, height: lensRadius * 0.3)
    context.addEllipse(in: glint)
    context.setFillColor(color(0xFFFFFF, alpha: 0.55))
    context.fillPath()

    let spark = eyeRadius * 0.06
    context.addEllipse(in: CGRect(x: center.x + lensRadius * 0.3, y: center.y + lensRadius * 0.25, width: spark * 2, height: spark * 2))
    context.setFillColor(color(0xFFFFFF, alpha: 0.3))
    context.fillPath()
}

func drawStains(in context: CGContext) {
    var random = SeededRandom(state: 2008)
    for cluster in 0..<16 {
        let depth = 0.55 + 0.45 * sqrt(random.next())
        let hugsEdge = cluster % 3 == 0
        let x = hugsEdge
            ? (random.next() < 0.5 ? body.minX + random.next() * 50 : body.maxX - random.next() * 50)
            : body.minX + random.next() * body.width
        let y = body.minY + body.height * depth
        let blobs = 3 + Int(random.next() * 4)
        for _ in 0..<blobs {
            let width = 14 + random.next() * 40
            let height = width * (0.45 + random.next() * 0.45)
            let dx = (random.next() - 0.5) * 60
            let dy = (random.next() - 0.5) * 28
            context.addEllipse(in: CGRect(x: x + dx - width / 2, y: y + dy - height / 2, width: width, height: height))
            context.setFillColor(color(0x6E2E12, alpha: 0.18 + 0.4 * (depth - 0.55) / 0.45))
            context.fillPath()
        }
    }
    for _ in 0..<14 {
        let depth = 0.5 + 0.5 * random.next()
        let x = body.minX + random.next() * body.width
        let y = body.minY + body.height * depth
        let size = 4 + random.next() * 7
        context.addEllipse(in: CGRect(x: x, y: y, width: size, height: size * 0.8))
        context.setFillColor(color(0x6E2E12, alpha: 0.55))
        context.fillPath()
    }
}

func drawGauge(in context: CGContext) {
    let panel = CGRect(x: body.midX - 170, y: body.minY + 70, width: 340, height: 104)
    context.addPath(CGPath(roundedRect: panel, cornerWidth: 16, cornerHeight: 16, transform: nil))
    context.setFillColor(color(0x2A1C12))
    context.fillPath()

    let cells = 5
    let filled = 3
    let gap: CGFloat = 12
    let inset: CGFloat = 16
    let cellWidth = (panel.width - inset * 2 - gap * CGFloat(cells - 1)) / CGFloat(cells)
    for index in 0..<cells {
        let cell = CGRect(
            x: panel.minX + inset + CGFloat(index) * (cellWidth + gap),
            y: panel.minY + inset,
            width: cellWidth,
            height: panel.height - inset * 2
        )
        let path = CGPath(roundedRect: cell, cornerWidth: 6, cornerHeight: 6, transform: nil)
        if index < filled {
            context.saveGState()
            context.addPath(path)
            context.clip()
            context.drawLinearGradient(
                linearGradient([color(0xFFE08A), color(0xF2B33D)], [0, 1]),
                start: CGPoint(x: 0, y: cell.minY),
                end: CGPoint(x: 0, y: cell.maxY),
                options: []
            )
            context.restoreGState()
        } else {
            context.addPath(path.copy(using: nil)!)
            context.setStrokeColor(color(0x8A5A2A))
            context.setLineWidth(5)
            context.strokePath()
        }
    }
}

func drawBody(in context: CGContext) {
    let path = CGPath(roundedRect: body, cornerWidth: bodyRadius, cornerHeight: bodyRadius, transform: nil)

    context.saveGState()
    context.setShadow(offset: CGSize(width: 0, height: 12), blur: 24, color: color(0x000000, alpha: 0.5))
    context.addPath(path)
    context.setFillColor(rust)
    context.fillPath()
    context.restoreGState()

    context.saveGState()
    context.addPath(path)
    context.clip()
    context.drawLinearGradient(
        linearGradient([color(0xF7C65A), bodyYellow, bodyYellow, rust], [0, 0.12, 0.4, 1]),
        start: CGPoint(x: 0, y: body.minY),
        end: CGPoint(x: 0, y: body.maxY),
        options: []
    )
    drawStains(in: context)
    context.setFillColor(color(0xFFFFFF, alpha: 0.18))
    context.fill(CGRect(x: body.minX, y: body.minY, width: body.width, height: 10))
    context.restoreGState()

    drawGauge(in: context)

    let seam = body.minY + body.height - 64
    context.setStrokeColor(stain)
    context.setLineWidth(5)
    context.move(to: CGPoint(x: body.minX + 24, y: seam))
    context.addLine(to: CGPoint(x: body.maxX - 24, y: seam))
    context.strokePath()
}

func drawIcon(in context: CGContext) {
    drawTile(in: context)
    context.saveGState()
    context.addPath(CGPath(roundedRect: tile, cornerWidth: tileRadius, cornerHeight: tileRadius, transform: nil))
    context.clip()
    drawNeck(in: context)
    drawBody(in: context)
    drawEye(at: leftEyeCenter, in: context)
    drawEye(at: rightEyeCenter, in: context)
    context.restoreGState()
}

func renderPNG(pixels: Int, to url: URL) throws {
    let context = CGContext(
        data: nil,
        width: pixels,
        height: pixels,
        bitsPerComponent: 8,
        bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    )!
    let scale = CGFloat(pixels) / canvas
    context.translateBy(x: 0, y: CGFloat(pixels))
    context.scaleBy(x: scale, y: -scale)
    context.interpolationQuality = .high
    drawIcon(in: context)
    let image = context.makeImage()!
    let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else {
        throw NSError(domain: "make-icon", code: 1, userInfo: [NSLocalizedDescriptionKey: "cannot write \(url.path)"])
    }
}

func iconsetEntries() -> [(name: String, pixels: Int)] {
    [16, 32, 128, 256, 512].flatMap { size in
        [("icon_\(size)x\(size).png", size), ("icon_\(size)x\(size)@2x.png", size * 2)]
    }
}

func runIconutil(iconset: URL, output: URL) throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
    process.arguments = ["-c", "icns", "-o", output.path, iconset.path]
    try process.run()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else {
        throw NSError(domain: "make-icon", code: 2, userInfo: [NSLocalizedDescriptionKey: "iconutil failed"])
    }
}

func repositoryRoot() -> URL {
    let script = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
    return script.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
}

let root = CommandLine.arguments.count > 1 ? URL(fileURLWithPath: CommandLine.arguments[1]) : repositoryRoot()
let fileManager = FileManager.default
let iconset = fileManager.temporaryDirectory.appendingPathComponent("AppIcon-\(UUID().uuidString).iconset")
try fileManager.createDirectory(at: iconset, withIntermediateDirectories: true)
defer { try? fileManager.removeItem(at: iconset) }

for entry in iconsetEntries() {
    try renderPNG(pixels: entry.pixels, to: iconset.appendingPathComponent(entry.name))
}

let resources = root.appendingPathComponent("app/Resources")
let assets = root.appendingPathComponent("assets")
try fileManager.createDirectory(at: resources, withIntermediateDirectories: true)
try fileManager.createDirectory(at: assets, withIntermediateDirectories: true)
try runIconutil(iconset: iconset, output: resources.appendingPathComponent("AppIcon.icns"))
try renderPNG(pixels: 512, to: assets.appendingPathComponent("app-icon.png"))
print("wrote \(resources.appendingPathComponent("AppIcon.icns").path)")
print("wrote \(assets.appendingPathComponent("app-icon.png").path)")
