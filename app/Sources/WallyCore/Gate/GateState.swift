import Foundation

public enum HandshakeOutcome: Equatable, Sendable {
    case unreachable
    case answered(Hello)
}

public enum GateState: Equatable, Sendable {
    case checking
    case cliMissing
    case daemonUnreachable(cliPath: String)
    case protocolMismatch(daemonVersion: String, daemonProtocol: Int)
    case connected(cliPath: String, daemonVersion: String)

    public static func resolve(cliPath: String?, handshake: HandshakeOutcome) -> GateState {
        switch handshake {
        case .answered(let hello):
            guard hello.protocolVersion == supportedProtocolVersion else {
                return .protocolMismatch(daemonVersion: hello.version, daemonProtocol: hello.protocolVersion)
            }
            return .connected(cliPath: cliPath ?? "", daemonVersion: hello.version)
        case .unreachable:
            guard let cliPath else { return .cliMissing }
            return .daemonUnreachable(cliPath: cliPath)
        }
    }

    public var isConnected: Bool {
        if case .connected = self { return true }
        return false
    }

    public var cliPath: String? {
        switch self {
        case .daemonUnreachable(let cliPath), .connected(let cliPath, _):
            cliPath.isEmpty ? nil : cliPath
        case .checking, .cliMissing, .protocolMismatch:
            nil
        }
    }
}

public enum DaemonStartAction: Equatable, Sendable {
    case install
    case start
    case restart

    public static func toLaunch(launchAgentInstalled: Bool) -> DaemonStartAction {
        launchAgentInstalled ? .start : .install
    }

    public var arguments: [String] {
        switch self {
        case .install: ["daemon", "install"]
        case .start: ["daemon", "start"]
        case .restart: ["daemon", "restart"]
        }
    }
}
