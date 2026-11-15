import AppKit
import SwiftUI
import WallyCore

struct SettingsView: View {
    @Environment(AppModel.self) private var model
    @State private var config: Config?
    @State private var intervalDraft = ""
    @State private var loginItem = LoginItem()
    @State private var configError: String?

    private let intervalPresets = ["1m", "5m", "15m", "30m", "1h", "2h", "6h", "12h", "24h"]

    var body: some View {
        Form {
            sourcesSection
            rotationSection
            startupSection
        }
        .formStyle(.grouped)
        .navigationTitle(LocalizedKey.settings.text)
        .task {
            if config == nil, let status = model.status {
                show(Config(
                    sources: status.sources,
                    recursive: status.recursive,
                    interval: status.interval,
                    order: status.order,
                    screens: status.screensMode
                ))
            }
            await load()
        }
        .onChange(of: model.status?.sources) { Task { await load() } }
        .onAppear { loginItem.refresh() }
    }

    private var sourcesSection: some View {
        Section {
            if let sources = config?.sources, !sources.isEmpty {
                ForEach(sources, id: \.self) { source in
                    SourceRow(source: source) {
                        Task { await apply(.sourceRemove(folder: source)) }
                    }
                }
            } else {
                Text(LocalizedKey.noSources.text)
                    .foregroundStyle(.secondary)
            }
            Toggle(LocalizedKey.includeSubfolders.text, isOn: binding(\.recursive, key: .recursive) { $0 ? "true" : "false" })
        } header: {
            HStack {
                Text(LocalizedKey.sources.text)
                Spacer()
                Button {
                    chooseFolder()
                } label: {
                    Label(LocalizedKey.addFolder.text, systemImage: "plus")
                }
                .buttonStyle(.borderless)
                .controlSize(.small)
            }
        } footer: {
            if let status = model.status {
                Text(LocalizedKey.librarySummary.format(status.images, status.favorites, status.banned))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var rotationSection: some View {
        Section(LocalizedKey.rotation.text) {
            LabeledContent(LocalizedKey.changeEvery.text) {
                HStack(spacing: 6) {
                    TextField(LocalizedKey.changeEvery.text, text: $intervalDraft, prompt: Text(LocalizedKey.intervalExamples.text))
                        .labelsHidden()
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 110)
                        .onSubmit(applyInterval)
                    Menu {
                        ForEach(intervalPresets, id: \.self) { preset in
                            Button(preset) {
                                intervalDraft = preset
                                applyInterval()
                            }
                        }
                    } label: {
                        Image(systemName: "clock")
                    }
                    .menuStyle(.borderlessButton)
                    .fixedSize()
                    if intervalDraft != (config?.interval ?? "") {
                        Button(LocalizedKey.apply.text, action: applyInterval)
                            .controlSize(.small)
                    }
                }
            }
            Picker(LocalizedKey.order.text, selection: binding(\.order, key: .order) { $0 }) {
                Text(LocalizedKey.orderRandom.text).tag(RotationOrder.random.rawValue)
                Text(LocalizedKey.orderSequential.text).tag(RotationOrder.sequential.rawValue)
            }
            .pickerStyle(.segmented)
            Picker(LocalizedKey.screens.text, selection: binding(\.screens, key: .screens) { $0 }) {
                Text(LocalizedKey.screensDistinct.text).tag(ScreensMode.distinct.rawValue)
                Text(LocalizedKey.screensSame.text).tag(ScreensMode.same.rawValue)
            }
            if let configError {
                Label(configError, systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(.red)
                    .font(.callout)
            }
        }
    }

    private var startupSection: some View {
        Section(LocalizedKey.startup.text) {
            Toggle(LocalizedKey.openAtLogin.text, isOn: Binding(
                get: { loginItem.isEnabled },
                set: { loginItem.setEnabled($0) }
            ))
            if loginItem.needsApproval {
                HStack {
                    Text(LocalizedKey.loginApprovalNeeded.text)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                    Spacer()
                    Button(LocalizedKey.openSystemSettings.text) { loginItem.openSystemSettings() }
                }
            }
            if let failure = loginItem.failure {
                Text(failure)
                    .font(.caption)
                    .foregroundStyle(.red)
            }
        }
    }

    private func binding<Value>(
        _ keyPath: WritableKeyPath<Config, Value>,
        key: ConfigKey,
        encode: @escaping (Value) -> String
    ) -> Binding<Value> where Value: Equatable & DefaultConfigValue {
        Binding(
            get: { config?[keyPath: keyPath] ?? Value.placeholder },
            set: { newValue in
                guard config?[keyPath: keyPath] != newValue else { return }
                config?[keyPath: keyPath] = newValue
                Task { await apply(.configSet(key: key, value: encode(newValue))) }
            }
        )
    }

    private func applyInterval() {
        let value = intervalDraft.trimmingCharacters(in: .whitespaces)
        guard !value.isEmpty, value != config?.interval else { return }
        Task { await apply(.configSet(key: .interval, value: value)) }
    }

    private func chooseFolder() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = true
        panel.message = LocalizedKey.chooseFolderMessage.text
        panel.prompt = LocalizedKey.add.text
        panel.directoryURL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Pictures")
        guard panel.runModal() == .OK else { return }
        let folders = panel.urls.map { PathDisplay.tilde($0.path) }
        Task {
            for folder in folders {
                await apply(.sourceAdd(folder: folder))
            }
        }
    }

    private func load() async {
        guard let loaded = await model.changeConfig(.configGet) else { return }
        show(loaded)
    }

    private func apply(_ command: DaemonCommand) async {
        if let updated = await model.changeConfig(command) {
            configError = nil
            show(updated)
        } else {
            configError = model.lastCommandError
            await load()
        }
    }

    private func show(_ loaded: Config) {
        config = loaded
        intervalDraft = loaded.interval
    }
}

protocol DefaultConfigValue {
    static var placeholder: Self { get }
}

extension Bool: DefaultConfigValue {
    static var placeholder: Bool { false }
}

extension String: DefaultConfigValue {
    static var placeholder: String { "" }
}

private struct SourceRow: View {
    let source: String
    let onRemove: () -> Void
    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "folder.fill")
                .foregroundStyle(.tint)
            Text(source)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer()
            Button {
                Finder.reveal(PathDisplay.expand(source))
            } label: {
                Image(systemName: "arrow.up.forward.square")
            }
            .buttonStyle(.borderless)
            .help(LocalizedKey.revealInFinder.text)
            .opacity(isHovered ? 1 : 0)
            Button(role: .destructive, action: onRemove) {
                Image(systemName: "minus.circle.fill")
                    .foregroundStyle(isHovered ? .red : .secondary)
            }
            .buttonStyle(.borderless)
            .help(LocalizedKey.remove.text)
        }
        .onHover { isHovered = $0 }
        .contextMenu {
            Button(LocalizedKey.revealInFinder.text) { Finder.reveal(PathDisplay.expand(source)) }
            Button(LocalizedKey.remove.text, role: .destructive, action: onRemove)
        }
    }
}
