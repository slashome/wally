import Foundation
import Testing
@testable import WallyCore

struct DaemonConnectionTests {
    private func echoingDaemon() throws -> FakeDaemon {
        try FakeDaemon { request in
            let id = request["id"]
            switch request["cmd"] as? String {
            case "hello":
                return [Fixture.response(id: id, result: #"{"version": "0.0.2", "protocol": 1}"#)]
            case "status", "pause":
                return [Fixture.response(id: id, result: Fixture.statusJSON(paused: request["cmd"] as? String == "pause"))]
            case "subscribe":
                return [
                    Fixture.response(id: id, result: Fixture.statusJSON()),
                    #"{"event": "something-new"}"#,
                    Fixture.statusEvent(paused: true),
                    Fixture.statusEvent(paused: false),
                    FakeDaemon.hangUp,
                ]
            case "config_get":
                return [FakeDaemon.hangUp]
            default:
                return [Fixture.failure(id: id, message: "unknown command")]
            }
        }
    }

    @Test func helloRoundTrips() async throws {
        let daemon = try echoingDaemon()
        defer { daemon.stop() }
        let connection = try await DaemonConnection.open(socketPath: daemon.socketPath)
        #expect(try await connection.hello() == Hello(version: "0.0.2", protocolVersion: 1))
        await connection.close()
    }

    @Test func pipelinedRequestsGetTheirOwnAnswers() async throws {
        let daemon = try echoingDaemon()
        defer { daemon.stop() }
        let connection = try await DaemonConnection.open(socketPath: daemon.socketPath)
        async let paused = connection.status(after: .pause)
        async let current = connection.status(after: .status)
        async let hello = connection.hello()
        let results = try await (paused, current, hello)
        #expect(results.0.paused)
        #expect(!results.1.paused)
        #expect(results.2.version == "0.0.2")
        #expect(Set(daemon.requests.compactMap { Self.id(of: $0) }) == [1, 2, 3])
        await connection.close()
    }

    @Test func aRefusalSurfacesTheMessage() async throws {
        let daemon = try echoingDaemon()
        defer { daemon.stop() }
        let connection = try await DaemonConnection.open(socketPath: daemon.socketPath)
        await #expect(throws: DaemonError.rejected("unknown command")) {
            try await connection.history()
        }
        await connection.close()
    }

    @Test func aHangUpFailsWaitingAndLaterRequests() async throws {
        let daemon = try echoingDaemon()
        defer { daemon.stop() }
        let connection = try await DaemonConnection.open(socketPath: daemon.socketPath)
        await #expect(throws: DaemonError.connectionClosed) {
            try await connection.send(.configGet, expecting: Config.self)
        }
        await #expect(throws: DaemonError.connectionClosed) {
            try await connection.hello()
        }
    }

    @Test func noDaemonMeansConnectionRefused() async throws {
        let path = FileManager.default.temporaryDirectory.appendingPathComponent("w-none.sock").path
        #expect(throws: DaemonError.self) {
            try DaemonConnection(socketPath: path)
        }
    }

    @Test func aSubscriptionYieldsTheStatusThenEveryEvent() async throws {
        let daemon = try echoingDaemon()
        defer { daemon.stop() }
        var received: [Bool] = []
        do {
            for try await status in try StatusSubscription.open(socketPath: daemon.socketPath) {
                received.append(status.paused)
            }
            Issue.record("a subscription ends with an error")
        } catch {
            #expect(error as? DaemonError == .connectionClosed)
        }
        #expect(received == [false, true, false])
        #expect(daemon.requests.first == #"{"cmd":"subscribe","id":1}"#)
    }

    private static func id(of line: String) -> Int? {
        let object = try? JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any]
        return object?["id"] as? Int
    }
}
