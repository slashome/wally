import Testing
@testable import WallyCore

struct GateTests {
    @Test func noCLIAndNoDaemonMeansInstallWally() {
        #expect(GateState.resolve(cliPath: nil, handshake: .unreachable) == .cliMissing)
    }

    @Test func aCLIWithoutDaemonOffersToStartIt() {
        #expect(GateState.resolve(cliPath: "/opt/homebrew/bin/wally", handshake: .unreachable)
            == .daemonUnreachable(cliPath: "/opt/homebrew/bin/wally"))
    }

    @Test func aDaemonOfAnotherProtocolAsksForAnUpgrade() {
        let hello = Hello(version: "0.9.0", protocolVersion: 2)
        #expect(GateState.resolve(cliPath: "/usr/local/bin/wally", handshake: .answered(hello))
            == .protocolMismatch(daemonVersion: "0.9.0", daemonProtocol: 2))
    }

    @Test func aDaemonOfTheSameProtocolConnects() {
        let state = GateState.resolve(cliPath: "/usr/local/bin/wally", handshake: .answered(Hello(version: "0.0.2", protocolVersion: 1)))
        #expect(state == .connected(cliPath: "/usr/local/bin/wally", daemonVersion: "0.0.2"))
        #expect(state.isConnected)
        #expect(state.cliPath == "/usr/local/bin/wally")
    }

    @Test func aRunningDaemonIsEnoughEvenWithoutCLI() {
        let state = GateState.resolve(cliPath: nil, handshake: .answered(Hello(version: "0.0.2", protocolVersion: 1)))
        #expect(state.isConnected)
        #expect(state.cliPath == nil)
    }

    @Test func startingInstallsTheLaunchAgentOnlyWhenMissing() {
        #expect(DaemonStartAction.toLaunch(launchAgentInstalled: false).arguments == ["daemon", "install"])
        #expect(DaemonStartAction.toLaunch(launchAgentInstalled: true).arguments == ["daemon", "start"])
        #expect(DaemonStartAction.restart.arguments == ["daemon", "restart"])
    }

    @Test func backoffDoublesUpToItsCeilingAndResets() {
        var backoff = Backoff(initialDelay: 1, maximumDelay: 5)
        #expect([backoff.nextDelay(), backoff.nextDelay(), backoff.nextDelay(), backoff.nextDelay()] == [1, 2, 4, 5])
        backoff.reset()
        #expect(backoff.nextDelay() == 1)
    }
}
