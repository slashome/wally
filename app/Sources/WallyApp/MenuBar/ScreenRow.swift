import SwiftUI
import WallyCore

struct ScreenRow: View {
    @Environment(AppModel.self) private var model
    let screen: Screen
    let showsScreenNumber: Bool
    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 10) {
            Button {
                if let image = screen.image { Finder.reveal(image) }
            } label: {
                HStack(spacing: 10) {
                    Thumbnail(path: screen.image, side: 72, cornerRadius: 6)
                        .frame(width: 72, height: 46)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(screen.imageFileName ?? LocalizedKey.noImage.text)
                            .font(.callout.weight(.medium))
                            .lineLimit(1)
                            .truncationMode(.middle)
                        Text(screenLabel)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(screen.image.map { PathDisplay.tilde($0) } ?? "")
            .disabled(screen.image == nil)

            HStack(spacing: 0) {
                IconButton(
                    systemImage: screen.favorite ? "star.fill" : "star",
                    help: screen.favorite ? LocalizedKey.unfavorite.text : LocalizedKey.favorite.text,
                    isActive: screen.favorite,
                    activeTint: .yellow
                ) {
                    model.send(.favorite(.screen(screen.id), on: !screen.favorite))
                }
                IconButton(systemImage: "nosign", help: LocalizedKey.banHelp.text) {
                    model.send(.ban(.screen(screen.id), on: true))
                }
                IconButton(systemImage: "forward", help: LocalizedKey.nextOnScreen.text) {
                    model.send(.next(screen: screen.id))
                }
            }
            .opacity(isHovered || screen.favorite ? 1 : 0.55)
        }
        .padding(6)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(isHovered ? Color.primary.opacity(0.05) : .clear)
        )
        .onHover { hovering in
            withAnimation(.easeOut(duration: 0.12)) { isHovered = hovering }
        }
        .contextMenu {
            if let image = screen.image {
                Button(LocalizedKey.revealInFinder.text) { Finder.reveal(image) }
            }
            Button(LocalizedKey.nextOnScreen.text) { model.send(.next(screen: screen.id)) }
        }
    }

    private var screenLabel: String {
        guard showsScreenNumber else { return screen.name }
        return "\(LocalizedKey.screenNumber.format(screen.index)) · \(screen.name)"
    }
}
