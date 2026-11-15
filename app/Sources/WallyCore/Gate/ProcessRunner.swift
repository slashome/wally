import Foundation

public struct ProcessOutcome: Equatable, Sendable {
    public var exitCode: Int32
    public var output: String
    public var errorOutput: String

    public init(exitCode: Int32, output: String, errorOutput: String) {
        self.exitCode = exitCode
        self.output = output
        self.errorOutput = errorOutput
    }

    public var succeeded: Bool { exitCode == 0 }
}

public protocol ProcessRunning: Sendable {
    func run(executable: String, arguments: [String], timeout: Double) async throws -> ProcessOutcome
}

public struct ProcessRunner: ProcessRunning {
    public init() {}

    public func run(executable: String, arguments: [String], timeout: Double) async throws -> ProcessOutcome {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments
        process.standardInput = FileHandle.nullDevice
        let outputPipe = Pipe()
        let errorPipe = Pipe()
        process.standardOutput = outputPipe
        process.standardError = errorPipe
        let termination = AsyncStream<Int32> { continuation in
            process.terminationHandler = { finished in
                continuation.yield(finished.terminationStatus)
                continuation.finish()
            }
        }

        try process.run()

        let outputHandle = outputPipe.fileHandleForReading
        let errorHandle = errorPipe.fileHandleForReading
        let outputReader = Task.detached { outputHandle.readDataToEndOfFile() }
        let errorReader = Task.detached { errorHandle.readDataToEndOfFile() }

        let exitCode: Int32
        do {
            exitCode = try await withTimeout(seconds: timeout) {
                for await code in termination { return code }
                return -1
            }
        } catch {
            process.terminate()
            throw error
        }

        return ProcessOutcome(
            exitCode: exitCode,
            output: String(decoding: await outputReader.value, as: UTF8.self),
            errorOutput: String(decoding: await errorReader.value, as: UTF8.self)
        )
    }
}
