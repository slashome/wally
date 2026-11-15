import Darwin
import Foundation
@testable import WallyCore

final class FakeDaemon: @unchecked Sendable {
    typealias Handler = @Sendable (_ request: [String: Any]) -> [String]

    let socketPath: String
    private let listener: Int32
    private let handler: Handler
    private let lock = NSLock()
    private var receivedLines: [String] = []

    init(handler: @escaping Handler) throws {
        socketPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("w-\(UUID().uuidString.prefix(8)).sock").path
        self.handler = handler
        var address = try SocketAddress.make(path: socketPath)
        listener = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        let bound = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(listener, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0, Darwin.listen(listener, 8) == 0 else {
            throw DaemonError.system(operation: "bind", code: errno)
        }
        let acceptor = Thread { [self] in acceptConnections() }
        acceptor.start()
    }

    var requests: [String] {
        lock.lock()
        defer { lock.unlock() }
        return receivedLines
    }

    func stop() {
        Darwin.shutdown(listener, SHUT_RDWR)
        Darwin.close(listener)
        unlink(socketPath)
    }

    private func acceptConnections() {
        while true {
            let client = Darwin.accept(listener, nil, nil)
            guard client >= 0 else { return }
            var enabled: Int32 = 1
            setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &enabled, socklen_t(MemoryLayout<Int32>.size))
            let worker = Thread { [self] in serve(client) }
            worker.start()
        }
    }

    private func serve(_ client: Int32) {
        defer { Darwin.close(client) }
        var splitter = LineSplitter()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while true {
            let count = buffer.withUnsafeMutableBytes { Darwin.read(client, $0.baseAddress, $0.count) }
            guard count > 0 else { return }
            for line in splitter.append(Data(buffer[0..<count])) {
                record(String(decoding: line, as: UTF8.self))
                let request = (try? JSONSerialization.jsonObject(with: line)) as? [String: Any] ?? [:]
                for reply in handler(request) {
                    if reply == FakeDaemon.hangUp { return }
                    let bytes = Array((reply + "\n").utf8)
                    _ = bytes.withUnsafeBytes { Darwin.write(client, $0.baseAddress, $0.count) }
                }
            }
        }
    }

    private func record(_ line: String) {
        lock.lock()
        receivedLines.append(line)
        lock.unlock()
    }

    static let hangUp = "\u{0}hang-up"
}

enum Fixture {
    static let screenJSON = """
    {"index": 1, "id": 69733382, "name": "DELL U2723QE", "image": "/Users/me/Pictures/Wallpapers/forest.jpg", "favorite": false}
    """

    static func statusJSON(paused: Bool = false, error: String? = nil) -> String {
        let errorValue = error.map { "\"\($0)\"" } ?? "null"
        let nextChange = paused ? "null" : "\"2026-11-15T10:30:00Z\""
        return """
        {"version": "0.0.2", "protocol": 1, "pid": 1234, "paused": \(paused), "interval": "30m", "interval_secs": 1800, "order": "random", "screens_mode": "distinct", "recursive": false, "sources": ["~/Pictures/Wallpapers"], "images": 1722, "favorites": 12, "banned": 3, "last_change": "2026-11-15T10:00:00Z", "next_change": \(nextChange), "screens": [\(screenJSON)], "error": \(errorValue)}
        """
    }

    static func response(id: Any?, result: String) -> String {
        "{\"id\": \(id ?? 0), \"ok\": true, \"result\": \(result)}"
    }

    static func failure(id: Any?, message: String) -> String {
        "{\"id\": \(id ?? 0), \"ok\": false, \"error\": \"\(message)\"}"
    }

    static func statusEvent(paused: Bool) -> String {
        "{\"event\": \"status\", \"status\": \(statusJSON(paused: paused))}"
    }

    static func date(_ text: String) -> Date {
        Timestamp.parse(text)!
    }
}
