import SwiftUI
import WallyCore

enum DaemonDisplayState: Equatable {
    case running
    case paused
    case failing(String)
    case stopped

    init(gate: GateState, status: Status?) {
        guard gate.isConnected, let status else {
            self = .stopped
            return
        }
        if let error = status.error, !error.isEmpty {
            self = .failing(error)
        } else {
            self = status.paused ? .paused : .running
        }
    }

    var label: String {
        switch self {
        case .running: LocalizedKey.running.text
        case .paused: LocalizedKey.paused.text
        case .failing: LocalizedKey.daemonError.text
        case .stopped: LocalizedKey.stopped.text
        }
    }

    var tint: Color {
        switch self {
        case .running: .green
        case .paused: .orange
        case .failing: .red
        case .stopped: .gray
        }
    }
}
