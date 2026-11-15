import Foundation

public protocol DaemonCommanding: Sendable {
    func send<Result: Decodable & Sendable>(_ command: DaemonCommand, expecting type: Result.Type) async throws -> Result
    func close() async
}

public actor DaemonConnection: DaemonCommanding {
    private let socket: UnixSocket
    private var nextRequestID = 1
    private var pendingRequests: [Int: CheckedContinuation<Data, Error>] = [:]
    private var closedError: DaemonError?
    private var readerTask: Task<Void, Never>?

    public init(socketPath: String) throws {
        socket = try UnixSocket.connect(to: socketPath)
    }

    public static func open(socketPath: String) async throws -> DaemonConnection {
        let connection = try DaemonConnection(socketPath: socketPath)
        await connection.startReading()
        return connection
    }

    private func startReading() {
        let lines = socket.lines()
        readerTask = Task { [weak self] in
            do {
                for try await line in lines {
                    await self?.receive(line)
                }
                await self?.finish(with: .connectionClosed)
            } catch let error as DaemonError {
                await self?.finish(with: error)
            } catch {
                await self?.finish(with: .connectionClosed)
            }
        }
    }

    private func receive(_ line: Data) {
        guard case .response(let id, let data)? = try? ResponseDecoder.classify(line) else { return }
        pendingRequests.removeValue(forKey: id)?.resume(returning: data)
    }

    private func finish(with error: DaemonError) {
        if closedError == nil {
            closedError = error
        }
        let waiting = pendingRequests
        pendingRequests.removeAll()
        for continuation in waiting.values {
            continuation.resume(throwing: error)
        }
    }

    public func send<Result: Decodable & Sendable>(_ command: DaemonCommand, expecting type: Result.Type) async throws -> Result {
        let line = try await sendRaw(command)
        return try ResponseDecoder.result(type, from: line)
    }

    private func sendRaw(_ command: DaemonCommand) async throws -> Data {
        if let closedError { throw closedError }
        let id = nextRequestID
        nextRequestID += 1
        let request = try RequestEncoder.line(for: command, id: id)
        return try await withCheckedThrowingContinuation { continuation in
            pendingRequests[id] = continuation
            do {
                try socket.write(request)
            } catch {
                pendingRequests.removeValue(forKey: id)?.resume(throwing: error)
            }
        }
    }

    public func close() {
        socket.close()
        finish(with: .connectionClosed)
        readerTask?.cancel()
    }
}

public extension DaemonCommanding {
    func hello() async throws -> Hello {
        try await send(.hello, expecting: Hello.self)
    }

    func status(after command: DaemonCommand) async throws -> Status {
        try await send(command, expecting: Status.self)
    }

    func history(limit: Int? = nil) async throws -> [HistoryEntry] {
        try await send(.history(limit: limit), expecting: [HistoryEntry].self)
    }

    func library(query: String?) async throws -> [LibraryItem] {
        try await send(.library(query: query), expecting: [LibraryItem].self)
    }

    func config(after command: DaemonCommand) async throws -> Config {
        try await send(command, expecting: Config.self)
    }
}
