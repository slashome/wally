import Foundation
import Testing
@testable import WallyCore

struct LocalizationTests {
    private static let resources = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("Resources")

    private func table(_ language: String) throws -> [String: String] {
        let url = Self.resources.appendingPathComponent("\(language).lproj/Localizable.strings")
        let table = NSDictionary(contentsOf: url) as? [String: String]
        return try #require(table)
    }

    @Test(arguments: ["en", "fr"])
    func everyKeyIsTranslated(language: String) throws {
        let table = try table(language)
        let missing = LocalizedKey.allCases.map(\.rawValue).filter { table[$0] == nil }
        #expect(missing == [])
        let unknown = Set(table.keys).subtracting(LocalizedKey.allCases.map(\.rawValue))
        #expect(unknown == [])
    }

    @Test(arguments: ["en", "fr"])
    func translationsKeepTheirPlaceholders(language: String) throws {
        let table = try table(language)
        for key in LocalizedKey.allCases {
            let translation = try #require(table[key.rawValue])
            #expect(placeholders(in: translation) == placeholders(in: key.rawValue), "\(language): \(key.rawValue)")
        }
    }

    @Test func englishIsTheKeyItself() {
        #expect(LocalizedKey.next.text == "Next")
        #expect(LocalizedKey.screenNumber.format(2) == "Screen 2")
    }

    private func placeholders(in text: String) -> [String] {
        let pattern = /%(?:\d+\$)?([@d])/
        return text.matches(of: pattern).map { String($0.output.1) }.sorted()
    }
}
