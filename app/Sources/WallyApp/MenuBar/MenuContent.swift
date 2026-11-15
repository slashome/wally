import AppKit
import SwiftUI
import WallyCore

struct MenuContent: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        VStack(spacing: 0) {
            MenuHeader()
                .padding(.horizontal, 14)
                .padding(.top, 12)
                .padding(.bottom, 10)
            Divider()
            if model.gate.isConnected, let status = model.status {
                ConnectedMenu(status: status)
            } else {
                GateView()
            }
            Divider()
            footer
                .padding(6)
        }
        .frame(width: 348)
    }

    private var footer: some View {
        VStack(spacing: 0) {
            MenuActionRow(title: LocalizedKey.openWally.text, systemImage: "macwindow", shortcut: "O") {
                openWindow(id: WindowID.main)
                NSApp.activate(ignoringOtherApps: true)
            }
            MenuActionRow(title: LocalizedKey.quit.text, systemImage: "power", shortcut: "Q") {
                NSApp.terminate(nil)
            }
        }
    }
}

private struct MenuHeader: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let state = DaemonDisplayState(gate: model.gate, status: model.status)
        HStack(spacing: 10) {
            Image(nsImage: RobotIcon.image(paused: false, size: NSSize(width: 30, height: 21)))
                .renderingMode(.template)
                .foregroundStyle(Color(red: 0.95, green: 0.70, blue: 0.24))
            Text(LocalizedKey.appName.text)
                .font(.headline)
            Spacer()
            DaemonStateBadge(state: state)
            switch model.gate {
            case .connected:
                IconButton(systemImage: "arrow.clockwise", help: LocalizedKey.restart.text) {
                    model.restartDaemon()
                }
                .disabled(model.isStartingDaemon || model.cliPath == nil)
            case .daemonUnreachable:
                IconButton(systemImage: "play.fill", help: LocalizedKey.start.text) {
                    model.startDaemon()
                }
                .disabled(model.isStartingDaemon)
            case .checking, .cliMissing, .protocolMismatch:
                EmptyView()
            }
        }
    }
}

private struct ConnectedMenu: View {
    @Environment(AppModel.self) private var model
    let status: Status

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if case .failing(let message) = DaemonDisplayState(gate: model.gate, status: status) {
                Label(message, systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundStyle(.red)
                    .symbolRenderingMode(.multicolor)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.horizontal, 14)
            }
            if let failure = model.lastCommandError {
                Label(failure, systemImage: "xmark.octagon")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 14)
                    .onTapGesture { model.dismissCommandError() }
            }
            VStack(spacing: 2) {
                ForEach(status.screens) { screen in
                    ScreenRow(screen: screen, showsScreenNumber: status.screens.count > 1)
                }
            }
            .padding(.horizontal, 6)

            Countdown(status: status)
                .padding(.horizontal, 14)
            HStack(spacing: 8) {
                Button {
                    model.send(status.paused ? .resume : .pause)
                } label: {
                    Label(
                        status.paused ? LocalizedKey.resume.text : LocalizedKey.pause.text,
                        systemImage: status.paused ? "play.fill" : "pause.fill"
                    )
                    .frame(maxWidth: .infinity)
                }
                Button {
                    model.send(.next(screen: nil))
                } label: {
                    Label(LocalizedKey.next.text, systemImage: "forward.fill")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut("n")
            }
            .controlSize(.large)
            .labelStyle(.titleAndIcon)
            .padding(.horizontal, 14)
        }
        .padding(.vertical, 10)
    }
}

private struct Countdown: View {
    let status: Status

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            HStack(spacing: 5) {
                Image(systemName: status.paused ? "pause.circle" : "timer")
                    .foregroundStyle(.secondary)
                Text(text(now: context.date))
                    .monospacedDigit()
                    .lineLimit(1)
                    .foregroundStyle(.secondary)
                    .contentTransition(.numericText())
            }
            .font(.callout)
        }
    }

    private func text(now: Date) -> String {
        if status.paused { return LocalizedKey.paused.text }
        guard let next = status.nextChange else { return "" }
        guard next.timeIntervalSince(now) > 0 else { return LocalizedKey.changingNow.text }
        return LocalizedKey.nextChangeIn.format(CountdownFormatter.format(until: next, now: now))
    }
}

struct MenuActionRow: View {
    let title: String
    let systemImage: String
    var shortcut: KeyEquivalent?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: systemImage)
                    .frame(width: 18)
                    .foregroundStyle(.secondary)
                Text(title)
                Spacer()
                if let shortcut {
                    Text("⌘\(String(shortcut.character).uppercased())")
                        .font(.callout)
                        .foregroundStyle(.tertiary)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
        }
        .buttonStyle(HoverHighlightStyle())
        .modifier(OptionalShortcut(shortcut: shortcut))
    }
}

private struct OptionalShortcut: ViewModifier {
    let shortcut: KeyEquivalent?

    func body(content: Content) -> some View {
        if let shortcut {
            content.keyboardShortcut(shortcut)
        } else {
            content
        }
    }
}
