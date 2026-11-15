import SwiftUI

struct HoverHighlightStyle: ButtonStyle {
    var cornerRadius: CGFloat = 6

    func makeBody(configuration: Configuration) -> some View {
        HoverHighlight(configuration: configuration, cornerRadius: cornerRadius)
    }

    private struct HoverHighlight: View {
        let configuration: Configuration
        let cornerRadius: CGFloat
        @State private var isHovered = false

        var body: some View {
            configuration.label
                .contentShape(Rectangle())
                .background(
                    RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                        .fill(fill)
                )
                .onHover { isHovered = $0 }
        }

        private var fill: Color {
            if configuration.isPressed { return Color.primary.opacity(0.14) }
            return isHovered ? Color.primary.opacity(0.07) : .clear
        }
    }
}

struct IconButton: View {
    let systemImage: String
    let help: String
    var isActive = false
    var activeTint: Color = .accentColor
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(isActive ? AnyShapeStyle(activeTint) : AnyShapeStyle(.secondary))
                .frame(width: 26, height: 26)
        }
        .buttonStyle(HoverHighlightStyle(cornerRadius: 6))
        .help(help)
        .accessibilityLabel(help)
    }
}

struct DaemonStateBadge: View {
    let state: DaemonDisplayState

    var body: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(state.tint)
                .frame(width: 7, height: 7)
                .shadow(color: state.tint.opacity(0.6), radius: 2)
            Text(state.label)
                .font(.caption.weight(.medium))
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .background(Capsule().fill(state.tint.opacity(0.12)))
    }
}
