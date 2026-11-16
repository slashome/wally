import Foundation

public func withTimeout<Value: Sendable>(
    seconds: Double,
    operation: @escaping @Sendable () async throws -> Value
) async throws -> Value {
    try await withThrowingTaskGroup(of: Value.self) { group in
        group.addTask { try await operation() }
        group.addTask {
            try await Task.sleep(for: .seconds(seconds))
            throw DaemonError.timedOut
        }
        guard let first = try await group.next() else { throw DaemonError.timedOut }
        group.cancelAll()
        return first
    }
}
