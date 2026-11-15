import Foundation

public struct LineSplitter: Sendable {
    private var pending = Data()

    public init() {}

    public mutating func append(_ chunk: Data) -> [Data] {
        pending.append(chunk)
        var lines: [Data] = []
        while let newline = pending.firstIndex(of: UInt8(ascii: "\n")) {
            let line = pending[pending.startIndex..<newline]
            pending = Data(pending[pending.index(after: newline)...])
            let trimmed = Data(line.last == UInt8(ascii: "\r") ? line.dropLast() : line)
            if !trimmed.isEmpty {
                lines.append(trimmed)
            }
        }
        return lines
    }
}
