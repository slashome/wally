import Darwin
import Foundation

public final class UnixSocket: @unchecked Sendable {
    private let descriptor: Int32
    private let lock = NSLock()
    private var isClosed = false

    init(descriptor: Int32) {
        self.descriptor = descriptor
        var enabled: Int32 = 1
        setsockopt(descriptor, SOL_SOCKET, SO_NOSIGPIPE, &enabled, socklen_t(MemoryLayout<Int32>.size))
    }

    deinit {
        Darwin.close(descriptor)
    }

    public static func connect(to path: String) throws -> UnixSocket {
        var address = try SocketAddress.make(path: path)
        let descriptor = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw DaemonError.system(operation: "socket", code: errno) }
        let result = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { socketAddress in
                Darwin.connect(descriptor, socketAddress, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0 else {
            let code = errno
            Darwin.close(descriptor)
            throw DaemonError.system(operation: "connect", code: code)
        }
        return UnixSocket(descriptor: descriptor)
    }

    public func write(_ data: Data) throws {
        try data.withUnsafeBytes { (buffer: UnsafeRawBufferPointer) in
            guard let base = buffer.baseAddress else { return }
            var offset = 0
            while offset < buffer.count {
                let written = Darwin.write(descriptor, base + offset, buffer.count - offset)
                if written < 0 {
                    if errno == EINTR { continue }
                    throw DaemonError.system(operation: "write", code: errno)
                }
                offset += written
            }
        }
    }

    public func read() throws -> Data {
        var buffer = [UInt8](repeating: 0, count: 65_536)
        while true {
            let count = buffer.withUnsafeMutableBytes { Darwin.read(descriptor, $0.baseAddress, $0.count) }
            if count < 0 {
                if errno == EINTR { continue }
                throw DaemonError.system(operation: "read", code: errno)
            }
            return Data(buffer[0..<count])
        }
    }

    public func lines() -> AsyncThrowingStream<Data, Error> {
        AsyncThrowingStream { continuation in
            let thread = Thread { [self] in
                var splitter = LineSplitter()
                do {
                    while true {
                        let chunk = try read()
                        if chunk.isEmpty { break }
                        for line in splitter.append(chunk) {
                            continuation.yield(line)
                        }
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            thread.name = "wally.socket.reader"
            continuation.onTermination = { [weak self] _ in self?.close() }
            thread.start()
        }
    }

    public func close() {
        lock.lock()
        defer { lock.unlock() }
        guard !isClosed else { return }
        isClosed = true
        Darwin.shutdown(descriptor, SHUT_RDWR)
    }
}

enum SocketAddress {
    static func make(path: String) throws -> sockaddr_un {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        guard bytes.count < capacity else { throw DaemonError.socketPathTooLong }
        withUnsafeMutableBytes(of: &address.sun_path) { buffer in
            buffer.copyBytes(from: bytes)
            buffer[bytes.count] = 0
        }
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        return address
    }
}
