import Foundation
import Testing
@testable import WallyCore

struct ResponseDecodingTests {
    private func data(_ text: String) -> Data { Data(text.utf8) }

    @Test func helloCarriesVersionAndProtocol() throws {
        let line = data(#"{"id": 1, "ok": true, "result": {"version": "0.0.2", "protocol": 1}}"#)
        #expect(try ResponseDecoder.result(Hello.self, from: line) == Hello(version: "0.0.2", protocolVersion: 1))
    }

    @Test func statusDecodesEveryDocumentedField() throws {
        let status = try ResponseDecoder.result(Status.self, from: data(Fixture.response(id: 1, result: Fixture.statusJSON())))
        #expect(status.version == "0.0.2")
        #expect(status.protocolVersion == 1)
        #expect(status.pid == 1234)
        #expect(status.paused == false)
        #expect(status.interval == "30m")
        #expect(status.intervalSeconds == 1800)
        #expect(status.order == "random")
        #expect(status.screensMode == "distinct")
        #expect(status.recursive == false)
        #expect(status.sources == ["~/Pictures/Wallpapers"])
        #expect(status.images == 1722)
        #expect(status.favorites == 12)
        #expect(status.banned == 3)
        #expect(status.lastChange == Fixture.date("2026-11-15T10:00:00Z"))
        #expect(status.nextChange == Fixture.date("2026-11-15T10:30:00Z"))
        #expect(status.error == nil)
        #expect(status.screens == [
            Screen(index: 1, id: 69733382, name: "DELL U2723QE", image: "/Users/me/Pictures/Wallpapers/forest.jpg", favorite: false),
        ])
        #expect(status.screens.first?.imageFileName == "forest.jpg")
    }

    @Test func aPausedStatusHasNoNextChangeAndMayCarryAnError() throws {
        let line = data(Fixture.response(id: 1, result: Fixture.statusJSON(paused: true, error: "no image in ~/Empty")))
        let status = try ResponseDecoder.result(Status.self, from: line)
        #expect(status.paused)
        #expect(status.nextChange == nil)
        #expect(status.error == "no image in ~/Empty")
    }

    @Test func aScreenWithoutImageDecodesAsNil() throws {
        let line = data(#"{"index": 2, "id": 1, "name": "Built-in", "image": null, "favorite": false}"#)
        let screen = try JSONDecoder().decode(Screen.self, from: line)
        #expect(screen.image == nil)
        #expect(screen.imageFileName == nil)
    }

    @Test func historyIsAListOfEntries() throws {
        let line = data(#"{"id": 3, "ok": true, "result": [{"at": "2026-11-15T10:00:00Z", "screen_id": 69733382, "screen_name": "DELL U2723QE", "image": "/path/forest.jpg"}]}"#)
        let entries = try ResponseDecoder.result([HistoryEntry].self, from: line)
        #expect(entries == [
            HistoryEntry(at: Fixture.date("2026-11-15T10:00:00Z"), screenID: 69733382, screenName: "DELL U2723QE", image: "/path/forest.jpg"),
        ])
        #expect(entries.first?.imageFileName == "forest.jpg")
    }

    @Test func libraryIsAListOfItems() throws {
        let line = data(#"{"id": 4, "ok": true, "result": [{"path": "/path/forest.jpg", "name": "forest.jpg", "favorite": true}]}"#)
        #expect(try ResponseDecoder.result([LibraryItem].self, from: line) == [
            LibraryItem(path: "/path/forest.jpg", name: "forest.jpg", favorite: true),
        ])
    }

    @Test func configIsTheYAMLFileAsJSON() throws {
        let line = data(#"{"id": 5, "ok": true, "result": {"sources": ["~/Pictures/Wallpapers"], "recursive": false, "interval": "30m", "order": "random", "screens": "distinct"}}"#)
        #expect(try ResponseDecoder.result(Config.self, from: line) == Config(
            sources: ["~/Pictures/Wallpapers"], recursive: false, interval: "30m", order: "random", screens: "distinct"
        ))
    }

    @Test func aRefusalThrowsTheDaemonMessage() throws {
        let line = data(#"{"id": 1, "ok": false, "error": "no screen with id 9"}"#)
        #expect(throws: DaemonError.rejected("no screen with id 9")) {
            try ResponseDecoder.result(Status.self, from: line)
        }
    }

    @Test func unknownFieldsAreIgnored() throws {
        let line = data(#"{"id": 1, "ok": true, "extra": 1, "result": {"version": "0.1.0", "protocol": 1, "features": ["x"]}}"#)
        #expect(try ResponseDecoder.result(Hello.self, from: line).version == "0.1.0")
    }

    @Test func responsesAreClassifiedByID() throws {
        let line = data(#"{"id": 7, "ok": true, "result": {}}"#)
        guard case .response(let id, _) = try ResponseDecoder.classify(line) else {
            Issue.record("expected a response")
            return
        }
        #expect(id == 7)
    }

    @Test func statusEventsCarryTheStatus() throws {
        guard case .statusEvent(let status) = try ResponseDecoder.classify(data(Fixture.statusEvent(paused: true))) else {
            Issue.record("expected a status event")
            return
        }
        #expect(status.paused)
    }

    @Test func otherEventsAreNamed() throws {
        guard case .otherEvent(let name) = try ResponseDecoder.classify(data(#"{"event": "ping"}"#)) else {
            Issue.record("expected another event")
            return
        }
        #expect(name == "ping")
    }

    @Test func aLineThatIsNotJSONIsMalformed() {
        #expect(throws: DaemonError.malformedLine) {
            try ResponseDecoder.classify(Data("not json".utf8))
        }
    }

    @Test func timestampsAcceptFractionalSeconds() {
        #expect(Timestamp.parse("2026-11-15T10:00:00.250Z") != nil)
        #expect(Timestamp.parse("2026-11-15T10:00:00Z") == Date(timeIntervalSince1970: 1_794_736_800))
        #expect(Timestamp.parse("yesterday") == nil)
    }
}
