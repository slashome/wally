import Foundation
import Testing
@testable import WallyCore

struct LineSplitterTests {
    @Test func linesAreSplitAcrossChunks() {
        var splitter = LineSplitter()
        #expect(splitter.append(Data("{\"a\":1}\n{\"b\"".utf8)) == [Data("{\"a\":1}".utf8)])
        #expect(splitter.append(Data(":2}\n\n".utf8)) == [Data("{\"b\":2}".utf8)])
        #expect(splitter.append(Data("x\r\n".utf8)) == [Data("x".utf8)])
    }
}
