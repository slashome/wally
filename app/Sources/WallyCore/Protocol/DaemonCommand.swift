import Foundation

public enum MarkTarget: Equatable, Sendable {
    case image(String)
    case screen(Int)
}

public enum DaemonCommand: Equatable, Sendable {
    case hello
    case status
    case next(screen: Int?)
    case set(screen: Int, image: String)
    case pause
    case resume
    case favorite(MarkTarget, on: Bool)
    case ban(MarkTarget, on: Bool)
    case history(limit: Int?)
    case library(query: String?)
    case configGet
    case configSet(key: ConfigKey, value: String)
    case sourceAdd(folder: String)
    case sourceRemove(folder: String)
    case subscribe

    public var name: String {
        switch self {
        case .hello: "hello"
        case .status: "status"
        case .next: "next"
        case .set: "set"
        case .pause: "pause"
        case .resume: "resume"
        case .favorite: "favorite"
        case .ban: "ban"
        case .history: "history"
        case .library: "library"
        case .configGet: "config_get"
        case .configSet: "config_set"
        case .sourceAdd: "source_add"
        case .sourceRemove: "source_remove"
        case .subscribe: "subscribe"
        }
    }

    var arguments: [String: Any] {
        switch self {
        case .hello, .status, .pause, .resume, .configGet, .subscribe:
            return [:]
        case .next(let screen):
            return screen.map { ["screen": $0] } ?? [:]
        case .set(let screen, let image):
            return ["screen": screen, "image": image]
        case .favorite(let target, let on), .ban(let target, let on):
            var arguments: [String: Any] = ["on": on]
            switch target {
            case .image(let path): arguments["image"] = path
            case .screen(let id): arguments["screen"] = id
            }
            return arguments
        case .history(let limit):
            return limit.map { ["limit": $0] } ?? [:]
        case .library(let query):
            guard let query, !query.trimmingCharacters(in: .whitespaces).isEmpty else { return [:] }
            return ["query": query]
        case .configSet(let key, let value):
            return ["key": key.rawValue, "value": value]
        case .sourceAdd(let folder), .sourceRemove(let folder):
            return ["folder": folder]
        }
    }
}

public enum RequestEncoder {
    public static func line(for command: DaemonCommand, id: Int) throws -> Data {
        var object = command.arguments
        object["id"] = id
        object["cmd"] = command.name
        var data = try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys, .withoutEscapingSlashes])
        data.append(UInt8(ascii: "\n"))
        return data
    }
}
