// swift-tools-version: 6.0
import Foundation
import PackageDescription

let commandLineToolsDeveloperDirectory = "/Library/Developer/CommandLineTools/Library/Developer"
let commandLineToolsFrameworks = "\(commandLineToolsDeveloperDirectory)/Frameworks"
let commandLineToolsLibraries = "\(commandLineToolsDeveloperDirectory)/usr/lib"
let testingNeedsCommandLineToolsPaths = FileManager.default.fileExists(
    atPath: "\(commandLineToolsFrameworks)/Testing.framework"
)

let testSwiftSettings: [SwiftSetting] = testingNeedsCommandLineToolsPaths
    ? [.unsafeFlags(["-F", commandLineToolsFrameworks])]
    : []

let testLinkerSettings: [LinkerSetting] = testingNeedsCommandLineToolsPaths
    ? [.unsafeFlags([
        "-F", commandLineToolsFrameworks,
        "-Xlinker", "-rpath", "-Xlinker", commandLineToolsFrameworks,
        "-Xlinker", "-rpath", "-Xlinker", commandLineToolsLibraries,
    ])]
    : []

let package = Package(
    name: "Wally",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "Wally", targets: ["WallyApp"]),
    ],
    targets: [
        .target(name: "WallyCore"),
        .executableTarget(
            name: "WallyApp",
            dependencies: ["WallyCore"]
        ),
        .testTarget(
            name: "WallyCoreTests",
            dependencies: ["WallyCore"],
            swiftSettings: testSwiftSettings,
            linkerSettings: testLinkerSettings
        ),
    ]
)
