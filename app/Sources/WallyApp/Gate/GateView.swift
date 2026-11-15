import AppKit
import SwiftUI
import WallyCore

struct GateView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        VStack(spacing: 14) {
            content
            if model.gate != .checking {
                Text(LocalizedKey.retrying.text)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
        }
        .multilineTextAlignment(.center)
        .padding(.horizontal, 20)
        .padding(.vertical, 22)
        .frame(maxWidth: .infinity)
    }

    @ViewBuilder
    private var content: some View {
        switch model.gate {
        case .checking, .connected:
            ProgressView()
                .controlSize(.small)
            Text(LocalizedKey.checking.text)
                .foregroundStyle(.secondary)
        case .cliMissing:
            GateHeading(
                systemImage: "shippingbox",
                title: LocalizedKey.cliMissingTitle.text,
                message: LocalizedKey.cliMissingMessage.text
            )
            CommandSnippet(command: "brew install slashome/tap/wally")
        case .daemonUnreachable:
            GateHeading(
                systemImage: "moon.zzz",
                title: LocalizedKey.daemonUnreachableTitle.text,
                message: LocalizedKey.daemonUnreachableMessage.text
            )
            Button {
                model.startDaemon()
            } label: {
                HStack(spacing: 6) {
                    if model.isStartingDaemon {
                        ProgressView().controlSize(.small)
                    } else {
                        Image(systemName: "play.fill")
                    }
                    Text(model.isStartingDaemon ? LocalizedKey.starting.text : LocalizedKey.startWally.text)
                }
                .frame(minWidth: 140)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .disabled(model.isStartingDaemon)
            if let failure = model.startFailure, !failure.isEmpty {
                Text(LocalizedKey.startFailed.format(failure))
                    .font(.caption)
                    .foregroundStyle(.red)
                    .textSelection(.enabled)
            }
        case .protocolMismatch(let daemonVersion, let daemonProtocol):
            GateHeading(
                systemImage: "arrow.triangle.2.circlepath",
                title: LocalizedKey.protocolMismatchTitle.text,
                message: LocalizedKey.protocolMismatchMessage.format(supportedProtocolVersion, daemonVersion, daemonProtocol)
            )
            CommandSnippet(command: "brew upgrade wally")
        }
    }
}

private struct GateHeading: View {
    let systemImage: String
    let title: String
    let message: String

    var body: some View {
        Image(systemName: systemImage)
            .font(.system(size: 30, weight: .light))
            .foregroundStyle(.secondary)
            .symbolRenderingMode(.hierarchical)
        Text(title)
            .font(.headline)
        Text(message)
            .font(.callout)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }
}

struct CommandSnippet: View {
    let command: String
    @State private var copied = false

    var body: some View {
        HStack(spacing: 8) {
            Text(command)
                .font(.system(size: 12, design: .monospaced))
                .textSelection(.enabled)
                .lineLimit(1)
                .minimumScaleFactor(0.75)
            Spacer(minLength: 4)
            Button {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(command, forType: .string)
                withAnimation { copied = true }
                Task {
                    try? await Task.sleep(for: .seconds(1.5))
                    withAnimation { copied = false }
                }
            } label: {
                Label(
                    copied ? LocalizedKey.copied.text : LocalizedKey.copy.text,
                    systemImage: copied ? "checkmark" : "doc.on.doc"
                )
                .font(.caption.weight(.medium))
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
        }
        .padding(.leading, 12)
        .padding(.trailing, 6)
        .padding(.vertical, 6)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(.background.opacity(0.6))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(.separator, lineWidth: 0.5)
        )
    }
}
