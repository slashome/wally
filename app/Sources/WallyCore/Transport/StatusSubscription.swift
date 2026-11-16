import Foundation

public enum StatusSubscription {
    public static func open(socketPath: String) throws -> AsyncThrowingStream<Status, Error> {
        let socket = try UnixSocket.connect(to: socketPath)
        try socket.write(RequestEncoder.line(for: .subscribe, id: 1))
        let lines = socket.lines()
        return AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    for try await line in lines {
                        switch try ResponseDecoder.classify(line) {
                        case .response(_, let data):
                            continuation.yield(try ResponseDecoder.result(Status.self, from: data))
                        case .statusEvent(let status):
                            continuation.yield(status)
                        case .otherEvent:
                            continue
                        }
                    }
                    continuation.finish(throwing: DaemonError.connectionClosed)
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in
                task.cancel()
                socket.close()
            }
        }
    }
}
