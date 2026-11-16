import SwiftUI
import WallyCore

enum WindowSection: String, CaseIterable, Identifiable, Hashable {
    case library
    case history
    case settings
    case about

    var id: String { rawValue }

    var title: String {
        switch self {
        case .library: LocalizedKey.library.text
        case .history: LocalizedKey.history.text
        case .settings: LocalizedKey.settings.text
        case .about: LocalizedKey.about.text
        }
    }

    var systemImage: String {
        switch self {
        case .library: "photo.on.rectangle.angled"
        case .history: "clock.arrow.circlepath"
        case .settings: "gearshape"
        case .about: "info.circle"
        }
    }

    var needsDaemon: Bool { self != .about }
}

struct MainWindow: View {
    @Environment(AppModel.self) private var model
    @State private var selection: WindowSection? = .library

    var body: some View {
        NavigationSplitView {
            List(WindowSection.allCases, selection: $selection) { section in
                Label(section.title, systemImage: section.systemImage)
                    .tag(section)
            }
            .navigationSplitViewColumnWidth(min: 170, ideal: 190, max: 240)
            .safeAreaInset(edge: .bottom) {
                DaemonStateBadge(state: DaemonDisplayState(gate: model.gate, status: model.status))
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        } detail: {
            detail
                .frame(minWidth: 560, minHeight: 440)
        }
    }

    @ViewBuilder
    private var detail: some View {
        let section = selection ?? .library
        if section.needsDaemon && !(model.gate.isConnected && model.status != nil) {
            GateView()
                .frame(maxWidth: 420)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .navigationTitle(section.title)
        } else {
            switch section {
            case .library: LibraryView()
            case .history: HistoryView()
            case .settings: SettingsView()
            case .about: AboutView()
            }
        }
    }
}
