import Foundation

public struct Backoff: Sendable {
    public let initialDelay: Double
    public let maximumDelay: Double
    public private(set) var attempts = 0

    public init(initialDelay: Double = 1, maximumDelay: Double = 5) {
        self.initialDelay = initialDelay
        self.maximumDelay = maximumDelay
    }

    public mutating func nextDelay() -> Double {
        let delay = min(maximumDelay, initialDelay * pow(2, Double(attempts)))
        attempts += 1
        return delay
    }

    public mutating func reset() {
        attempts = 0
    }
}
