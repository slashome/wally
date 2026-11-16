import Foundation

public let supportedProtocolVersion = 1

public struct Hello: Decodable, Equatable, Sendable {
    public var version: String
    public var protocolVersion: Int

    public init(version: String, protocolVersion: Int) {
        self.version = version
        self.protocolVersion = protocolVersion
    }

    enum CodingKeys: String, CodingKey {
        case version
        case protocolVersion = "protocol"
    }
}

public struct Screen: Decodable, Equatable, Hashable, Identifiable, Sendable {
    public var index: Int
    public var id: Int
    public var name: String
    public var image: String?
    public var favorite: Bool

    public init(index: Int, id: Int, name: String, image: String?, favorite: Bool) {
        self.index = index
        self.id = id
        self.name = name
        self.image = image
        self.favorite = favorite
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        index = try container.decode(Int.self, forKey: .index)
        id = try container.decode(Int.self, forKey: .id)
        name = try container.decodeIfPresent(String.self, forKey: .name) ?? ""
        image = try container.decodeIfPresent(String.self, forKey: .image)
        favorite = try container.decodeIfPresent(Bool.self, forKey: .favorite) ?? false
    }

    public var imageFileName: String? {
        image.map { URL(fileURLWithPath: $0).lastPathComponent }
    }

    enum CodingKeys: String, CodingKey {
        case index, id, name, image, favorite
    }
}

public struct Status: Decodable, Equatable, Sendable {
    public var version: String
    public var protocolVersion: Int
    public var pid: Int
    public var paused: Bool
    public var interval: String
    public var intervalSeconds: Int
    public var order: String
    public var screensMode: String
    public var recursive: Bool
    public var sources: [String]
    public var images: Int
    public var favorites: Int
    public var banned: Int
    public var lastChange: Date?
    public var nextChange: Date?
    public var screens: [Screen]
    public var error: String?

    public init(
        version: String = "",
        protocolVersion: Int = supportedProtocolVersion,
        pid: Int = 0,
        paused: Bool = false,
        interval: String = "",
        intervalSeconds: Int = 0,
        order: String = RotationOrder.random.rawValue,
        screensMode: String = ScreensMode.distinct.rawValue,
        recursive: Bool = false,
        sources: [String] = [],
        images: Int = 0,
        favorites: Int = 0,
        banned: Int = 0,
        lastChange: Date? = nil,
        nextChange: Date? = nil,
        screens: [Screen] = [],
        error: String? = nil
    ) {
        self.version = version
        self.protocolVersion = protocolVersion
        self.pid = pid
        self.paused = paused
        self.interval = interval
        self.intervalSeconds = intervalSeconds
        self.order = order
        self.screensMode = screensMode
        self.recursive = recursive
        self.sources = sources
        self.images = images
        self.favorites = favorites
        self.banned = banned
        self.lastChange = lastChange
        self.nextChange = nextChange
        self.screens = screens
        self.error = error
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decodeIfPresent(String.self, forKey: .version) ?? ""
        protocolVersion = try container.decodeIfPresent(Int.self, forKey: .protocolVersion) ?? supportedProtocolVersion
        pid = try container.decodeIfPresent(Int.self, forKey: .pid) ?? 0
        paused = try container.decodeIfPresent(Bool.self, forKey: .paused) ?? false
        interval = try container.decodeIfPresent(String.self, forKey: .interval) ?? ""
        intervalSeconds = try container.decodeIfPresent(Int.self, forKey: .intervalSeconds) ?? 0
        order = try container.decodeIfPresent(String.self, forKey: .order) ?? RotationOrder.random.rawValue
        screensMode = try container.decodeIfPresent(String.self, forKey: .screensMode) ?? ScreensMode.distinct.rawValue
        recursive = try container.decodeIfPresent(Bool.self, forKey: .recursive) ?? false
        sources = try container.decodeIfPresent([String].self, forKey: .sources) ?? []
        images = try container.decodeIfPresent(Int.self, forKey: .images) ?? 0
        favorites = try container.decodeIfPresent(Int.self, forKey: .favorites) ?? 0
        banned = try container.decodeIfPresent(Int.self, forKey: .banned) ?? 0
        lastChange = try container.decodeIfPresent(Date.self, forKey: .lastChange)
        nextChange = try container.decodeIfPresent(Date.self, forKey: .nextChange)
        screens = try container.decodeIfPresent([Screen].self, forKey: .screens) ?? []
        error = try container.decodeIfPresent(String.self, forKey: .error)
    }

    enum CodingKeys: String, CodingKey {
        case version
        case protocolVersion = "protocol"
        case pid, paused, interval
        case intervalSeconds = "interval_secs"
        case order
        case screensMode = "screens_mode"
        case recursive, sources, images, favorites, banned
        case lastChange = "last_change"
        case nextChange = "next_change"
        case screens, error
    }
}

public struct HistoryEntry: Decodable, Equatable, Hashable, Identifiable, Sendable {
    public var at: Date
    public var screenID: Int
    public var screenName: String
    public var image: String

    public init(at: Date, screenID: Int, screenName: String, image: String) {
        self.at = at
        self.screenID = screenID
        self.screenName = screenName
        self.image = image
    }

    public var id: String { "\(at.timeIntervalSince1970)-\(screenID)-\(image)" }

    public var imageFileName: String { URL(fileURLWithPath: image).lastPathComponent }

    enum CodingKeys: String, CodingKey {
        case at
        case screenID = "screen_id"
        case screenName = "screen_name"
        case image
    }
}

public struct LibraryImage: Decodable, Equatable, Hashable, Identifiable, Sendable {
    public var path: String
    public var name: String
    public var favorite: Bool

    public init(path: String, name: String, favorite: Bool) {
        self.path = path
        self.name = name
        self.favorite = favorite
    }

    public var id: String { path }
}

public struct Config: Decodable, Equatable, Sendable {
    public var sources: [String]
    public var recursive: Bool
    public var interval: String
    public var order: String
    public var screens: String

    public init(sources: [String], recursive: Bool, interval: String, order: String, screens: String) {
        self.sources = sources
        self.recursive = recursive
        self.interval = interval
        self.order = order
        self.screens = screens
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        sources = try container.decodeIfPresent([String].self, forKey: .sources) ?? []
        recursive = try container.decodeIfPresent(Bool.self, forKey: .recursive) ?? false
        interval = try container.decodeIfPresent(String.self, forKey: .interval) ?? ""
        order = try container.decodeIfPresent(String.self, forKey: .order) ?? RotationOrder.random.rawValue
        screens = try container.decodeIfPresent(String.self, forKey: .screens) ?? ScreensMode.distinct.rawValue
    }

    enum CodingKeys: String, CodingKey {
        case sources, recursive, interval, order, screens
    }
}

public enum RotationOrder: String, CaseIterable, Identifiable, Sendable {
    case random
    case sequential

    public var id: String { rawValue }
}

public enum ScreensMode: String, CaseIterable, Identifiable, Sendable {
    case distinct
    case same

    public var id: String { rawValue }
}

public enum ConfigKey: String, CaseIterable, Sendable {
    case interval
    case order
    case screens
    case recursive
}
