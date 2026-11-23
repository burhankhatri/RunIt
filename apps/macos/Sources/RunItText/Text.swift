import Foundation

public enum Show {
    public static func size(_ b: Double) -> String { b >= 1e9 ? String(format: "%.1f GB", b / 1e9) : b >= 1e6 ? String(format: "%.0f MB", b / 1e6) : String(format: "%.0f KB", max(1, b / 1e3)) }
    public static func memory(_ b: Double) -> String { String(format: "%.1f GB", b / 1_073_741_824) }
    public static func speed(_ s: Double) -> String { s >= 10 ? String(format: "%.0f", s) : String(format: "%.1f", s) }
    public static func count(_ n: Int) -> String { n >= 1_000_000 ? String(format: "%.1fM", Double(n) / 1e6) : n >= 1000 ? "\(n / 1000)K" : "\(n)" }
    public static func license(_ l: String) -> String { l.isEmpty || l == "unknown" || l == "other" ? "licence not stated" : l }
    public static func left(_ bytes: Double, at bytesPerS: Double) -> String {
        guard bytesPerS > 1000 else { return "" }
        let s = bytes / bytesPerS
        return s < 90 ? "\(Int(s)) s left" : s < 5400 ? "\(Int(s / 60)) min left" : String(format: "%.1f h left", s / 3600)
    }
}

public enum Samples {
    /// Split the way tokenizers roughly do: words, with long ones in pieces.
    public static func split(_ s: String) -> [String] {
        var out: [String] = [], word = ""
        for c in s {
            if c == " " || c == "\n" || word.filter({ $0 != " " }).count >= 6 {
                if !word.isEmpty { out.append(word) }
                word = ""
            }
            word.append(c)
        }
        return out + (word.isEmpty ? [] : [word])
    }
    public static let prose = split("Tides are the ocean answering the moon. Its gravity pulls the near side of the Earth a little harder than the far side, and the water stretches into two bulges. The planet turns beneath them, so most coasts rise and fall twice a day. The sun joins in as well. When all three line up, the range is widest, and sailors call it a spring tide.")
    public static let code = split("def moving_average(values, window):\n    total = 0.0\n    out = []\n    for i, v in enumerate(values):\n        total += v\n        if i >= window:\n            total -= values[i - window]\n        if i >= window - 1:\n            out.append(total / window)\n    return out\n")
}
