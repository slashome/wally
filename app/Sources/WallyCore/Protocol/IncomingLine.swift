import Foundation

public enum DaemonError: Error, Equatable, Sendable, LocalizedError {
    case rejected(String)
    case malformedLine
    case connectionClosed
    case socketPathTooLong
    case system(operation: String, code: Int32)
    case timedOut

    public var errorDescription: String? {
        switch self {
        case .rejected(let message): message
        case .malformedLine: "The daemon sent a line that is not valid JSON."
        case .connectionClosed: "The connection to the daemon closed."
        case .socketPathTooLong: "The socket path is too long."
        case .system(let operation, let code): "\(operation) failed: \(String(cString: strerror(code)))"
        case .timedOut: "The daemon did not answer in time."
        }
    }
}

public enum IncomingLine: Sendable {
    case response(id: Int, line: Data)
    case statusEvent(Status)
    case otherEvent(String)
}

public enum ResponseDecoder {
    private struct Envelope: Decodable {
        var id: Int?
        var ok: Bool?
        var error: String?
        var event: String?
    }

    private struct ResultEnvelope<Result: Decodable>: Decodable {
        var result: Result
    }

    private struct StatusEvent: Decodable {
        var status: Status
    }

    public static func classify(_ line: Data) throws -> IncomingLine {
        let decoder = makeJSONDecoder()
        guard let envelope = try? decoder.decode(Envelope.self, from: line) else {
            throw DaemonError.malformedLine
        }
        if let event = envelope.event {
            guard event == "status" else { return .otherEvent(event) }
            return .statusEvent(try decoder.decode(StatusEvent.self, from: line).status)
        }
        guard let id = envelope.id else { throw DaemonError.malformedLine }
        return .response(id: id, line: line)
    }

    public static func result<Result: Decodable>(_ type: Result.Type, from line: Data) throws -> Result {
        let decoder = makeJSONDecoder()
        guard let envelope = try? decoder.decode(Envelope.self, from: line) else {
            throw DaemonError.malformedLine
        }
        guard envelope.ok == true else {
            throw DaemonError.rejected(envelope.error ?? "The daemon refused the request.")
        }
        return try decoder.decode(ResultEnvelope<Result>.self, from: line).result
    }

    public static func makeJSONDecoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let text = try container.decode(String.self)
            if let date = Timestamp.parse(text) { return date }
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Not an RFC 3339 timestamp: \(text)")
        }
        return decoder
    }
}

public enum Timestamp {
    public static func parse(_ text: String) -> Date? {
        if let date = try? Date(text, strategy: .iso8601) { return date }
        return try? Date(text, strategy: Date.ISO8601FormatStyle(includingFractionalSeconds: true))
    }
}
