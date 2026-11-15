import AppKit

enum RobotIcon {
    static let menuBarSize = NSSize(width: 26, height: 18)

    static func image(paused: Bool, size: NSSize = menuBarSize) -> NSImage {
        let image = NSImage(size: size, flipped: false) { bounds in
            guard let context = NSGraphicsContext.current?.cgContext else { return false }
            context.setAlpha(paused ? 0.4 : 1)
            context.beginTransparencyLayer(auxiliaryInfo: nil)
            draw(in: bounds)
            context.endTransparencyLayer()
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = "Wally"
        return image
    }

    private static func draw(in bounds: NSRect) {
        let scale = min(bounds.width / menuBarSize.width, bounds.height / menuBarSize.height)
        let origin = NSPoint(
            x: bounds.midX - menuBarSize.width * scale / 2,
            y: bounds.midY - menuBarSize.height * scale / 2
        )
        func point(_ x: CGFloat, _ y: CGFloat) -> NSPoint {
            NSPoint(x: origin.x + x * scale, y: origin.y + y * scale)
        }
        NSColor.black.set()

        let eyeRadius: CGFloat = 4.7
        let eyeCenters = [point(5.8, 11.6), point(20.2, 11.6)]
        for center in eyeCenters {
            let housing = NSBezierPath(ovalIn: NSRect(
                x: center.x - eyeRadius * scale, y: center.y - eyeRadius * scale,
                width: 2 * eyeRadius * scale, height: 2 * eyeRadius * scale
            ))
            housing.lineWidth = 1.6 * scale
            housing.stroke()

            let lensRadius: CGFloat = 1.9
            NSBezierPath(ovalIn: NSRect(
                x: center.x - lensRadius * scale, y: center.y - lensRadius * scale,
                width: 2 * lensRadius * scale, height: 2 * lensRadius * scale
            )).fill()
        }

        for y in [10.3, 12.9] as [CGFloat] {
            let bar = NSBezierPath()
            bar.move(to: point(11, y))
            bar.line(to: point(15, y))
            bar.lineWidth = 1.4 * scale
            bar.lineCapStyle = .round
            bar.stroke()
        }

        let neck = NSBezierPath()
        neck.move(to: point(13, 10.3))
        neck.line(to: point(13, 3.2))
        neck.lineWidth = 1.7 * scale
        neck.lineCapStyle = .round
        neck.stroke()

        let shoulders = NSBezierPath(roundedRect: NSRect(
            x: origin.x + 7.5 * scale, y: origin.y + 0.4 * scale,
            width: 11 * scale, height: 2.4 * scale
        ), xRadius: 1.2 * scale, yRadius: 1.2 * scale)
        shoulders.fill()
    }
}
