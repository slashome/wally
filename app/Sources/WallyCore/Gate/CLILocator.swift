import Foundation

public protocol FileProbing: Sendable {
    func isExecutableFile(atPath path: String) -> Bool
    func fileExists(atPath path: String) -> Bool
}

public struct LocalFileProbe: FileProbing {
    public init() {}

    public func isExecutableFile(atPath path: String) -> Bool {
        FileManager.default.isExecutableFile(atPath: path)
    }

    public func fileExists(atPath path: String) -> Bool {
        FileManager.default.fileExists(atPath: path)
    }
}

public struct CLILocator: Sendable {
    public typealias ShellLookup = @Sendable () async -> String?

    private let fileProbe: FileProbing
    private let homeDirectory: String
    private let shellLookup: ShellLookup

    public init(fileProbe: FileProbing, homeDirectory: String, shellLookup: @escaping ShellLookup) {
        self.fileProbe = fileProbe
        self.homeDirectory = homeDirectory
        self.shellLookup = shellLookup
    }

    public var wellKnownPaths: [String] {
        [
            "/opt/homebrew/bin/wally",
            "/usr/local/bin/wally",
            (homeDirectory as NSString).appendingPathComponent(".local/bin/wally"),
        ]
    }

    public func locate() async -> String? {
        if let known = wellKnownPaths.first(where: fileProbe.isExecutableFile(atPath:)) {
            return known
        }
        guard let found = await shellLookup() else { return nil }
        let candidate = Self.firstAbsolutePath(in: found)
        guard let candidate, fileProbe.isExecutableFile(atPath: candidate) else { return nil }
        return candidate
    }

    static func firstAbsolutePath(in output: String) -> String? {
        output
            .split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .first { $0.hasPrefix("/") }
    }

    public static func loginShellLookup(runner: ProcessRunning = ProcessRunner()) -> ShellLookup {
        {
            let outcome = try? await runner.run(
                executable: "/bin/zsh",
                arguments: ["-lc", "command -v wally"],
                timeout: 5
            )
            guard let outcome, outcome.succeeded else { return nil }
            return outcome.output
        }
    }
}
