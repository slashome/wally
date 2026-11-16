import Foundation

public enum CountdownFormatter {
    public static func format(secondsRemaining: TimeInterval) -> String {
        let total = max(0, Int(secondsRemaining.rounded(.up)))
        let hours = total / 3600
        let minutes = (total % 3600) / 60
        let seconds = total % 60
        if hours > 0 {
            return minutes > 0 ? "\(hours)h \(twoDigits(minutes))m" : "\(hours)h"
        }
        if minutes > 0 {
            return "\(minutes)m \(twoDigits(seconds))s"
        }
        return "\(seconds)s"
    }

    public static func format(until date: Date, now: Date) -> String {
        format(secondsRemaining: date.timeIntervalSince(now))
    }

    private static func twoDigits(_ value: Int) -> String {
        value < 10 ? "0\(value)" : "\(value)"
    }
}
