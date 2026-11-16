import Foundation

public enum WallyLocations {
    public static let launchAgentLabel = "me.slashome.wally"

    public static func socketPath(environment: [String: String], homeDirectory: String) -> String {
        stateDirectory(environment: environment, homeDirectory: homeDirectory) + "/wally.sock"
    }

    public static func stateDirectory(environment: [String: String], homeDirectory: String) -> String {
        let base: String
        if let stateHome = environment["XDG_STATE_HOME"], !stateHome.isEmpty {
            base = stateHome
        } else {
            base = homeDirectory + "/.local/state"
        }
        return trimmingTrailingSlash(base) + "/wally"
    }

    public static func launchAgentPlistPath(homeDirectory: String) -> String {
        trimmingTrailingSlash(homeDirectory) + "/Library/LaunchAgents/\(launchAgentLabel).plist"
    }

    private static func trimmingTrailingSlash(_ path: String) -> String {
        guard path.count > 1, path.hasSuffix("/") else { return path }
        return String(path.dropLast())
    }
}
