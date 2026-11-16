import SwiftUI
import WallyCore

struct LibraryView: View {
    @Environment(AppModel.self) private var model
    @State private var query = ""
    @State private var items: [LibraryImage] = []
    @State private var isLoading = true
    @State private var loadedQuery = ""

    private let columns = [GridItem(.adaptive(minimum: 170, maximum: 240), spacing: 14)]

    var body: some View {
        Group {
            if items.isEmpty && !isLoading {
                emptyState
            } else {
                ScrollView {
                    LazyVGrid(columns: columns, spacing: 16) {
                        ForEach(items) { item in
                            LibraryCell(item: item) { favorite in
                                setFavorite(item, favorite)
                            } onBan: {
                                ban(item)
                            }
                        }
                    }
                    .padding(18)
                }
            }
        }
        .overlay(alignment: .top) {
            if isLoading && items.isEmpty {
                ProgressView().padding(.top, 40)
            }
        }
        .navigationTitle(LocalizedKey.library.text)
        .navigationSubtitle(LocalizedKey.imageCount.format(items.count))
        .searchable(text: $query, placement: .toolbar, prompt: LocalizedKey.searchImages.text)
        .task(id: LibraryRequest(query: query, revision: libraryRevision)) {
            if !loadedQuery.isEmpty || !query.isEmpty {
                try? await Task.sleep(for: .milliseconds(250))
                guard !Task.isCancelled else { return }
            }
            isLoading = true
            let found = await model.library(query: query)
            guard !Task.isCancelled else { return }
            if let found { items = found }
            loadedQuery = query
            isLoading = false
        }
    }

    private var libraryRevision: String {
        guard let status = model.status else { return "" }
        return "\(status.images)-\(status.favorites)-\(status.banned)-\(status.sources.joined(separator: "|"))-\(status.recursive)"
    }

    @ViewBuilder
    private var emptyState: some View {
        if query.isEmpty {
            ContentUnavailableView(
                LocalizedKey.noImagesTitle.text,
                systemImage: "photo.on.rectangle",
                description: Text(LocalizedKey.noImagesMessage.text)
            )
        } else {
            ContentUnavailableView(
                LocalizedKey.noResultsTitle.text,
                systemImage: "magnifyingglass",
                description: Text(LocalizedKey.noResultsMessage.format(query))
            )
        }
    }

    private func setFavorite(_ item: LibraryImage, _ favorite: Bool) {
        if let index = items.firstIndex(of: item) {
            items[index].favorite = favorite
        }
        model.send(.favorite(.image(item.path), on: favorite))
    }

    private func ban(_ item: LibraryImage) {
        withAnimation { items.removeAll { $0.path == item.path } }
        model.send(.ban(.image(item.path), on: true))
    }
}

private struct LibraryRequest: Hashable {
    let query: String
    let revision: String
}

private struct LibraryCell: View {
    @Environment(AppModel.self) private var model
    let item: LibraryImage
    let onFavorite: (Bool) -> Void
    let onBan: () -> Void
    @State private var isHovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Thumbnail(path: item.path, side: 240, cornerRadius: 10)
                .aspectRatio(16 / 10, contentMode: .fit)
                .overlay(alignment: .topTrailing) { favoriteBadge }
                .overlay(alignment: .bottom) {
                    if isHovered { hoverActions.transition(.opacity) }
                }
                .shadow(color: .black.opacity(isHovered ? 0.18 : 0.06), radius: isHovered ? 8 : 3, y: 2)
                .scaleEffect(isHovered ? 1.015 : 1)
            Text(item.name)
                .font(.callout)
                .lineLimit(1)
                .truncationMode(.middle)
                .foregroundStyle(isHovered ? .primary : .secondary)
                .help(PathDisplay.tilde(item.path))
        }
        .onHover { hovering in
            withAnimation(.easeOut(duration: 0.15)) { isHovered = hovering }
        }
        .onTapGesture(count: 2) { Finder.reveal(item.path) }
        .contextMenu { menu }
    }

    @ViewBuilder
    private var favoriteBadge: some View {
        if item.favorite {
            Image(systemName: "star.fill")
                .font(.caption)
                .foregroundStyle(.yellow)
                .padding(6)
                .background(.ultraThinMaterial, in: Circle())
                .padding(6)
        }
    }

    private var hoverActions: some View {
        HStack(spacing: 2) {
            if let screens = model.status?.screens {
                Menu {
                    ForEach(screens) { screen in
                        Button(LocalizedKey.setOnScreen.format(screenTitle(screen, among: screens))) {
                            model.send(.set(screen: screen.id, image: item.path))
                        }
                    }
                } label: {
                    Image(systemName: "display")
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .fixedSize()
                .frame(width: 26, height: 26)
                .help(LocalizedKey.setOnScreen.format("…"))
            }
            IconButton(
                systemImage: item.favorite ? "star.fill" : "star",
                help: item.favorite ? LocalizedKey.unfavorite.text : LocalizedKey.favorite.text,
                isActive: item.favorite,
                activeTint: .yellow
            ) {
                onFavorite(!item.favorite)
            }
            IconButton(systemImage: "nosign", help: LocalizedKey.banHelp.text, action: onBan)
            IconButton(systemImage: "folder", help: LocalizedKey.revealInFinder.text) {
                Finder.reveal(item.path)
            }
        }
        .padding(3)
        .background(.regularMaterial, in: Capsule())
        .padding(8)
    }

    @ViewBuilder
    private var menu: some View {
        if let screens = model.status?.screens {
            ForEach(screens) { screen in
                Button(LocalizedKey.setOnScreen.format(screenTitle(screen, among: screens))) {
                    model.send(.set(screen: screen.id, image: item.path))
                }
            }
            Divider()
        }
        Button(item.favorite ? LocalizedKey.unfavorite.text : LocalizedKey.favorite.text) {
            onFavorite(!item.favorite)
        }
        Button(LocalizedKey.ban.text, role: .destructive, action: onBan)
        Divider()
        Button(LocalizedKey.revealInFinder.text) { Finder.reveal(item.path) }
    }

    private func screenTitle(_ screen: Screen, among screens: [Screen]) -> String {
        screens.count > 1 ? "\(screen.index) · \(screen.name)" : screen.name
    }
}
