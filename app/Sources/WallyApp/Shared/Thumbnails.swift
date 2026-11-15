import AppKit
@preconcurrency import QuickLookThumbnailing
import SwiftUI

@MainActor
final class ThumbnailCache {
    static let shared = ThumbnailCache()

    private let images = NSCache<NSString, NSImage>()
    private var inFlight: [String: Task<NSImage?, Never>] = [:]

    init() {
        images.countLimit = 600
    }

    func cachedImage(for path: String, side: CGFloat) -> NSImage? {
        images.object(forKey: key(path, side) as NSString)
    }

    func image(for path: String, side: CGFloat) async -> NSImage? {
        let key = key(path, side)
        if let cached = images.object(forKey: key as NSString) { return cached }
        if let running = inFlight[key] { return await running.value }
        let scale = NSScreen.main?.backingScaleFactor ?? 2
        let task = Task<NSImage?, Never> {
            let request = QLThumbnailGenerator.Request(
                fileAt: URL(fileURLWithPath: path),
                size: CGSize(width: side, height: side),
                scale: scale,
                representationTypes: .thumbnail
            )
            guard let representation = try? await QLThumbnailGenerator.shared.generateBestRepresentation(for: request) else {
                return nil
            }
            return representation.nsImage
        }
        inFlight[key] = task
        let image = await task.value
        inFlight[key] = nil
        if let image {
            images.setObject(image, forKey: key as NSString)
        }
        return image
    }

    private func key(_ path: String, _ side: CGFloat) -> String {
        "\(Int(side))|\(path)"
    }
}

struct Thumbnail: View {
    let path: String?
    var side: CGFloat = 64
    var cornerRadius: CGFloat = 6
    var contentMode: ContentMode = .fill

    @State private var image: NSImage?

    var body: some View {
        Rectangle()
            .fill(.quaternary)
            .overlay {
                if let image {
                    Image(nsImage: image)
                        .resizable()
                        .aspectRatio(contentMode: contentMode)
                        .transition(.opacity)
                } else {
                    Image(systemName: path == nil ? "photo.badge.exclamationmark" : "photo")
                        .foregroundStyle(.tertiary)
                }
            }
        .clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                .strokeBorder(.separator, lineWidth: 0.5)
        )
        .task(id: path) {
            guard let path else {
                image = nil
                return
            }
            if let cached = ThumbnailCache.shared.cachedImage(for: path, side: side) {
                image = cached
                return
            }
            image = nil
            let loaded = await ThumbnailCache.shared.image(for: path, side: side)
            withAnimation(.easeOut(duration: 0.15)) { image = loaded }
        }
    }
}
