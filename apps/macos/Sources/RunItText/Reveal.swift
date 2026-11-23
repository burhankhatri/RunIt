/// Which characters of a text are written, part written, or still to come.
public struct Reveal: Equatable {
    public static let trail = 5.0
    public let solid: Int
    public let fading: Range<Int>
    /// The next character to write, until there is none.
    public let caret: Int?
    let shown: Double

    public init(shown: Double, count: Int) {
        self.shown = shown
        let n = Int(shown)
        solid = min(count, max(0, n - Int(Self.trail)))
        fading = solid..<max(solid, min(n, count))
        caret = n < count ? n : nil
    }
    public func opacity(_ i: Int) -> Double { min(1, (shown - Double(i)) / Self.trail) }
}
