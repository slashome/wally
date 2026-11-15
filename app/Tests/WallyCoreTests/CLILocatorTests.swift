import Testing
@testable import WallyCore

struct FakeFileProbe: FileProbing {
    var executables: Set<String>
    var files: Set<String> = []

    func isExecutableFile(atPath path: String) -> Bool { executables.contains(path) }
    func fileExists(atPath path: String) -> Bool { files.contains(path) || executables.contains(path) }
}

struct CLILocatorTests {
    private func locator(executables: Set<String>, shellAnswer: String?) -> CLILocator {
        CLILocator(fileProbe: FakeFileProbe(executables: executables), homeDirectory: "/Users/me", shellLookup: { shellAnswer })
    }

    @Test func homebrewOnAppleSiliconComesFirst() async {
        let found = await locator(executables: ["/opt/homebrew/bin/wally", "/usr/local/bin/wally"], shellAnswer: nil).locate()
        #expect(found == "/opt/homebrew/bin/wally")
    }

    @Test func thenHomebrewOnIntel() async {
        let found = await locator(executables: ["/usr/local/bin/wally", "/Users/me/.local/bin/wally"], shellAnswer: nil).locate()
        #expect(found == "/usr/local/bin/wally")
    }

    @Test func thenTheInstallScriptFolder() async {
        let found = await locator(executables: ["/Users/me/.local/bin/wally"], shellAnswer: "/elsewhere/wally").locate()
        #expect(found == "/Users/me/.local/bin/wally")
    }

    @Test func thenThePathOfALoginShell() async {
        let found = await locator(executables: ["/Users/me/.cargo/bin/wally"], shellAnswer: "\n/Users/me/.cargo/bin/wally\n").locate()
        #expect(found == "/Users/me/.cargo/bin/wally")
    }

    @Test func aShellAnswerThatIsNotAnExecutableFileIsIgnored() async {
        #expect(await locator(executables: [], shellAnswer: "wally: aliased to echo").locate() == nil)
        #expect(await locator(executables: [], shellAnswer: "/gone/wally").locate() == nil)
    }

    @Test func nothingFoundMeansMissing() async {
        #expect(await locator(executables: [], shellAnswer: nil).locate() == nil)
    }
}
