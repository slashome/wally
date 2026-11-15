import Foundation
import Testing
@testable import WallyCore

struct SupportTests {
    @Test(arguments: [
        (0.0, "0s"), (0.2, "1s"), (-5.0, "0s"), (45.0, "45s"), (60.0, "1m 00s"), (725.0, "12m 05s"),
        (3600.0, "1h"), (3720.0, "1h 02m"), (5400.0, "1h 30m"), (90000.0, "25h"),
    ])
    func countdownIsCompact(seconds: Double, expected: String) {
        #expect(CountdownFormatter.format(secondsRemaining: seconds) == expected)
    }

    @Test func countdownMeasuresUntilADate() {
        let now = Date(timeIntervalSince1970: 1000)
        #expect(CountdownFormatter.format(until: now.addingTimeInterval(90), now: now) == "1m 30s")
    }

    @Test func socketLivesInTheStateDirectory() {
        #expect(WallyLocations.socketPath(environment: [:], homeDirectory: "/Users/me") == "/Users/me/.local/state/wally/wally.sock")
        #expect(WallyLocations.socketPath(environment: ["XDG_STATE_HOME": "/tmp/state/"], homeDirectory: "/Users/me") == "/tmp/state/wally/wally.sock")
        #expect(WallyLocations.socketPath(environment: ["XDG_STATE_HOME": ""], homeDirectory: "/Users/me") == "/Users/me/.local/state/wally/wally.sock")
    }

    @Test func launchAgentPlistIsInTheUserLibrary() {
        #expect(WallyLocations.launchAgentPlistPath(homeDirectory: "/Users/me") == "/Users/me/Library/LaunchAgents/me.slashome.wally.plist")
    }

    @Test func pathsAreShownWithATilde() {
        #expect(PathDisplay.tilde("/Users/me/Pictures", homeDirectory: "/Users/me") == "~/Pictures")
        #expect(PathDisplay.tilde("/Users/me", homeDirectory: "/Users/me") == "~")
        #expect(PathDisplay.tilde("/Users/meow/x", homeDirectory: "/Users/me") == "/Users/meow/x")
        #expect(PathDisplay.expand("~/Pictures", homeDirectory: "/Users/me") == "/Users/me/Pictures")
        #expect(PathDisplay.expand("/tmp", homeDirectory: "/Users/me") == "/tmp")
    }

    @Test func aSlowOperationTimesOut() async {
        await #expect(throws: DaemonError.timedOut) {
            try await withTimeout(seconds: 0.05) {
                try await Task.sleep(for: .seconds(5))
                return 1
            }
        }
    }

    @Test func aProcessReportsItsOutputAndExitCode() async throws {
        let outcome = try await ProcessRunner().run(executable: "/bin/sh", arguments: ["-c", "echo out; echo err >&2; exit 3"], timeout: 5)
        #expect(outcome == ProcessOutcome(exitCode: 3, output: "out\n", errorOutput: "err\n"))
        #expect(!outcome.succeeded)
    }
}
