import Foundation
import Testing
@testable import WallyCore

struct RequestEncodingTests {
    private func encoded(_ command: DaemonCommand, id: Int = 1) throws -> String {
        let data = try RequestEncoder.line(for: command, id: id)
        #expect(data.last == UInt8(ascii: "\n"))
        return String(decoding: data.dropLast(), as: UTF8.self)
    }

    @Test(arguments: [
        (DaemonCommand.hello, #"{"cmd":"hello","id":1}"#),
        (.status, #"{"cmd":"status","id":1}"#),
        (.next(screen: nil), #"{"cmd":"next","id":1}"#),
        (.next(screen: 2), #"{"cmd":"next","id":1,"screen":2}"#),
        (.set(screen: 69733382, image: "/p/forest.jpg"), #"{"cmd":"set","id":1,"image":"/p/forest.jpg","screen":69733382}"#),
        (.pause, #"{"cmd":"pause","id":1}"#),
        (.resume, #"{"cmd":"resume","id":1}"#),
        (.favorite(.image("/p/a.jpg"), on: true), #"{"cmd":"favorite","id":1,"image":"/p/a.jpg","on":true}"#),
        (.favorite(.screen(7), on: false), #"{"cmd":"favorite","id":1,"on":false,"screen":7}"#),
        (.ban(.screen(7), on: true), #"{"cmd":"ban","id":1,"on":true,"screen":7}"#),
        (.ban(.image("/p/a.jpg"), on: true), #"{"cmd":"ban","id":1,"image":"/p/a.jpg","on":true}"#),
        (.history(limit: nil), #"{"cmd":"history","id":1}"#),
        (.history(limit: 20), #"{"cmd":"history","id":1,"limit":20}"#),
        (.library(query: nil), #"{"cmd":"library","id":1}"#),
        (.library(query: "  "), #"{"cmd":"library","id":1}"#),
        (.library(query: "dune night"), #"{"cmd":"library","id":1,"query":"dune night"}"#),
        (.configGet, #"{"cmd":"config_get","id":1}"#),
        (.configSet(key: .interval, value: "15m"), #"{"cmd":"config_set","id":1,"key":"interval","value":"15m"}"#),
        (.configSet(key: .recursive, value: "true"), #"{"cmd":"config_set","id":1,"key":"recursive","value":"true"}"#),
        (.sourceAdd(folder: "~/Pictures/Space"), #"{"cmd":"source_add","folder":"~/Pictures/Space","id":1}"#),
        (.sourceRemove(folder: "/Volumes/Art"), #"{"cmd":"source_remove","folder":"/Volumes/Art","id":1}"#),
        (.subscribe, #"{"cmd":"subscribe","id":1}"#),
    ])
    func everyCommandIsOneJSONLine(command: DaemonCommand, expected: String) throws {
        #expect(try encoded(command) == expected)
    }

    @Test func theRequestIDIsEchoedFromTheCaller() throws {
        #expect(try encoded(.status, id: 42) == #"{"cmd":"status","id":42}"#)
    }
}
