import Foundation

public enum PathDisplay {
    public static func tilde(_ path: String, homeDirectory: String = NSHomeDirectory()) -> String {
        if path == homeDirectory { return "~" }
        let prefix = homeDirectory.hasSuffix("/") ? homeDirectory : homeDirectory + "/"
        guard path.hasPrefix(prefix) else { return path }
        return "~/" + path.dropFirst(prefix.count)
    }

    public static func expand(_ path: String, homeDirectory: String = NSHomeDirectory()) -> String {
        if path == "~" { return homeDirectory }
        guard path.hasPrefix("~/") else { return path }
        return (homeDirectory as NSString).appendingPathComponent(String(path.dropFirst(2)))
    }
}
