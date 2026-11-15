import SwiftUI
import WallyCore

struct HistoryView: View {
    @Environment(AppModel.self) private var model
    @State private var entries: [HistoryEntry] = []
    @State private var isLoading = true

    var body: some View {
        Group {
            if entries.isEmpty && !isLoading {
                ContentUnavailableView(
                    LocalizedKey.noHistoryTitle.text,
                    systemImage: "clock",
                    description: Text(LocalizedKey.noHistoryMessage.text)
                )
            } else {
                List(entries) { entry in
                    HistoryRow(entry: entry)
                }
                .listStyle(.inset(alternatesRowBackgrounds: false))
            }
        }
        .overlay {
            if isLoading && entries.isEmpty { ProgressView() }
        }
        .navigationTitle(LocalizedKey.history.text)
        .toolbar {
            Button {
                Task { await load() }
            } label: {
                Label(LocalizedKey.refresh.text, systemImage: "arrow.clockwise")
            }
            .help(LocalizedKey.refresh.text)
        }
        .task(id: model.historyRevision) { await load() }
    }

    private func load() async {
        isLoading = true
        if let loaded = await model.history(limit: 100) {
            withAnimation { entries = loaded }
        }
        isLoading = false
    }
}

private struct HistoryRow: View {
    let entry: HistoryEntry
    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 12) {
            Thumbnail(path: entry.image, side: 96, cornerRadius: 6)
                .frame(width: 84, height: 54)
            VStack(alignment: .leading, spacing: 3) {
                Text(entry.imageFileName)
                    .font(.body.weight(.medium))
                    .lineLimit(1)
                    .truncationMode(.middle)
                Label(entry.screenName, systemImage: "display")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .labelStyle(.titleAndIcon)
            }
            Spacer()
            Text(entry.at, format: .relative(presentation: .named))
                .font(.callout)
                .foregroundStyle(.secondary)
                .help(entry.at.formatted(date: .abbreviated, time: .standard))
            Button {
                Finder.reveal(entry.image)
            } label: {
                Image(systemName: "folder")
            }
            .buttonStyle(.borderless)
            .help(LocalizedKey.revealInFinder.text)
            .opacity(isHovered ? 1 : 0)
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .onHover { isHovered = $0 }
        .contextMenu {
            Button(LocalizedKey.revealInFinder.text) { Finder.reveal(entry.image) }
        }
    }
}
