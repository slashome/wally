import AppKit
import Observation
import WallyCore

@MainActor
@Observable
final class AppModel {
    private(set) var gate: GateState = .checking
    private(set) var status: Status?
    private(set) var isStartingDaemon = false
    private(set) var startFailure: String?
    private(set) var lastCommandError: String?
    private(set) var historyRevision = 0

    private let environment = ProcessInfo.processInfo.environment
    private let homeDirectory = NSHomeDirectory()
    private let fileProbe = LocalFileProbe()
    private let processRunner = ProcessRunner()
    private var connection: DaemonConnection?
    private var supervisor: Task<Void, Never>?
    private var retryRequested = false

    var socketPath: String {
        WallyLocations.socketPath(environment: environment, homeDirectory: homeDirectory)
    }

    var cliPath: String? { gate.cliPath ?? lastKnownCLIPath }
    private var lastKnownCLIPath: String?

    func start() {
        guard supervisor == nil else { return }
        supervisor = Task { [weak self] in
            await self?.superviseConnection()
        }
    }

    private func superviseConnection() async {
        var backoff = Backoff(initialDelay: 1, maximumDelay: 4)
        while !Task.isCancelled {
            let reachedDaemon = await connectAndFollow()
            if reachedDaemon { backoff.reset() }
            await waitBeforeRetry(seconds: backoff.nextDelay())
        }
    }

    private func waitBeforeRetry(seconds: Double) async {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline, !retryRequested, !Task.isCancelled {
            try? await Task.sleep(for: .milliseconds(200))
        }
        retryRequested = false
    }

    func retryNow() {
        retryRequested = true
    }

    private func connectAndFollow() async -> Bool {
        let locator = CLILocator(
            fileProbe: fileProbe,
            homeDirectory: homeDirectory,
            shellLookup: CLILocator.loginShellLookup(runner: processRunner)
        )
        let foundCLI: String?
        if let known = lastKnownCLIPath, fileProbe.isExecutableFile(atPath: known) {
            foundCLI = known
        } else {
            foundCLI = await locator.locate()
            lastKnownCLIPath = foundCLI
        }

        let path = socketPath
        let opened = try? await withTimeout(seconds: 3) {
            let connection = try await DaemonConnection.open(socketPath: path)
            return (connection, try await connection.hello())
        }
        guard let opened else {
            gate = .resolve(cliPath: foundCLI, handshake: .unreachable)
            status = nil
            return false
        }
        let (connection, hello) = opened

        gate = .resolve(cliPath: foundCLI, handshake: .answered(hello))
        guard gate.isConnected else {
            await connection.close()
            status = nil
            return false
        }

        self.connection = connection
        startFailure = nil
        do {
            for try await update in try StatusSubscription.open(socketPath: path) {
                apply(update)
            }
        } catch {}
        await connection.close()
        self.connection = nil
        status = nil
        return true
    }

    private func apply(_ update: Status) {
        if update.lastChange != status?.lastChange {
            historyRevision += 1
        }
        status = update
    }

    var launchAgentInstalled: Bool {
        fileProbe.fileExists(atPath: WallyLocations.launchAgentPlistPath(homeDirectory: homeDirectory))
    }

    func startDaemon() {
        let action = DaemonStartAction.toLaunch(launchAgentInstalled: launchAgentInstalled)
        runDaemonAction(action)
    }

    func restartDaemon() {
        runDaemonAction(.restart)
    }

    private func runDaemonAction(_ action: DaemonStartAction) {
        guard let cliPath, !isStartingDaemon else { return }
        isStartingDaemon = true
        startFailure = nil
        Task {
            do {
                let outcome = try await processRunner.run(executable: cliPath, arguments: action.arguments, timeout: 30)
                if !outcome.succeeded {
                    let message = outcome.errorOutput.isEmpty ? outcome.output : outcome.errorOutput
                    startFailure = message.trimmingCharacters(in: .whitespacesAndNewlines)
                }
            } catch {
                startFailure = error.localizedDescription
            }
            isStartingDaemon = false
            retryNow()
        }
    }

    @discardableResult
    func perform(_ command: DaemonCommand) async -> Status? {
        guard let connection else { return nil }
        do {
            let updated = try await connection.status(after: command)
            lastCommandError = nil
            apply(updated)
            return updated
        } catch {
            lastCommandError = error.localizedDescription
            return nil
        }
    }

    func send(_ command: DaemonCommand) {
        Task { await perform(command) }
    }

    func changeConfig(_ command: DaemonCommand) async -> Config? {
        guard let connection else { return nil }
        do {
            let config = try await connection.config(after: command)
            lastCommandError = nil
            return config
        } catch {
            lastCommandError = error.localizedDescription
            return nil
        }
    }

    func library(query: String?) async -> [LibraryImage]? {
        guard let connection else { return nil }
        do {
            return try await connection.library(query: query)
        } catch {
            lastCommandError = error.localizedDescription
            return nil
        }
    }

    func history(limit: Int) async -> [HistoryEntry]? {
        guard let connection else { return nil }
        do {
            return try await connection.history(limit: limit)
        } catch {
            lastCommandError = error.localizedDescription
            return nil
        }
    }

    func dismissCommandError() {
        lastCommandError = nil
    }
}
