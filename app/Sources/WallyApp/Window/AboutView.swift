import SwiftUI
import WallyCore

struct AboutView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        VStack(spacing: 18) {
            Spacer()
            Image(nsImage: RobotIcon.image(paused: false, size: NSSize(width: 104, height: 72)))
                .renderingMode(.template)
                .foregroundStyle(
                    LinearGradient(
                        colors: [Color(red: 0.95, green: 0.70, blue: 0.24), Color(red: 0.66, green: 0.33, blue: 0.13)],
                        startPoint: .top,
                        endPoint: .bottom
                    )
                )
            VStack(spacing: 4) {
                Text(LocalizedKey.appName.text)
                    .font(.largeTitle.weight(.semibold))
                Text(LocalizedKey.tagline.text)
                    .foregroundStyle(.secondary)
            }
            Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 8) {
                row(LocalizedKey.appVersion.text, Self.appVersion)
                row(LocalizedKey.daemonVersion.text, model.status?.version ?? LocalizedKey.notConnected.text)
                row(LocalizedKey.protocolVersion.text, protocolDescription)
                row(LocalizedKey.commandLine.text, model.cliPath.map { PathDisplay.tilde($0) } ?? "—")
            }
            .padding(16)
            .background(.background.secondary, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            Link(destination: URL(string: "https://github.com/slashome/wally")!) {
                Label(LocalizedKey.sourceCode.text, systemImage: "chevron.left.forwardslash.chevron.right")
            }
            Spacer()
        }
        .padding(30)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .navigationTitle(LocalizedKey.about.text)
    }

    private var protocolDescription: String {
        guard let daemonProtocol = model.status?.protocolVersion else { return "\(supportedProtocolVersion)" }
        return daemonProtocol == supportedProtocolVersion
            ? "\(supportedProtocolVersion)"
            : "\(supportedProtocolVersion) / \(daemonProtocol)"
    }

    private func row(_ title: String, _ value: String) -> some View {
        GridRow {
            Text(title)
                .foregroundStyle(.secondary)
                .gridColumnAlignment(.trailing)
            Text(value)
                .font(.system(.body, design: .monospaced))
                .textSelection(.enabled)
        }
    }

    private static var appVersion: String {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String ?? "dev"
        guard let build = info?["CFBundleVersion"] as? String, build != version else { return version }
        return "\(version) (\(build))"
    }
}
