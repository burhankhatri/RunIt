import RunItText
import SwiftUI

/// Text that writes itself behind a block caret. Room for all of it is held from the start, so nothing shifts.
struct Typed: View {
    let text: String
    var font: Font = .mono(17)
    var color: Color = .text
    var perSecond = 46.0
    var delay = 0.0
    var done: () -> Void = {}
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var start: Date?
    @State private var finished = false

    private var chars: [Character] { Array(text) }

    var body: some View {
        Group {
            if finished || still {
                Text(text).foregroundStyle(color)
            } else {
                TimelineView(.animation) { t in Text(frame(at: t.date)) }
            }
        }
        .font(font)
        .accessibilityLabel(text)
        .task(id: text) {
            if still { return done() }
            finished = false
            start = .now.addingTimeInterval(delay)
            try? await Task.sleep(for: .seconds(delay + Double(chars.count) / perSecond + 0.05))
            guard !Task.isCancelled else { return }
            finished = true
            done()
        }
    }

    private func frame(at now: Date) -> AttributedString {
        let shown = start.map { max(0, now.timeIntervalSince($0) * perSecond) } ?? 0
        return written(chars, Reveal(shown: shown, count: chars.count).fading.upperBound, color: color, caret: shown > 0 ? .orange : .clear)
    }
}

/// The first `n` characters, then a caret, then clear space for the rest.
private func written(_ chars: [Character], _ n: Int, color: Color, caret: Color) -> AttributedString {
    var out = AttributedString(String(chars[0..<n]))
    out.foregroundColor = color
    guard n < chars.count else { return out }
    var next = AttributedString(String(chars[n]))
    next.foregroundColor = .clear
    next.backgroundColor = caret
    var rest = AttributedString(String(chars[(n + 1)...]))
    rest.foregroundColor = .clear
    return out + next + rest
}

/// A sample reply written at exactly the speed a model is predicted to write.
struct SpeedPreview: View {
    let tokensPerSecond: Double
    var code = false
    var size: CGFloat = 13.5
    var color: Color = .ink
    /// How much of the sample fits where it is shown.
    var limit = 48
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var start = Date.now

    var body: some View {
        let tokens = Array((code ? Samples.code : Samples.prose).prefix(limit))
        let chars = Array(tokens.joined())
        let length = Double(tokens.count) / max(tokensPerSecond, 0.1)
        TimelineView(.animation(paused: still)) { t in
            let at = still ? length : t.date.timeIntervalSince(start).truncatingRemainder(dividingBy: length + 2.4)
            let n = min(tokens.count, Int(at * tokensPerSecond))
            Text(written(chars, tokens.prefix(n).reduce(0) { $0 + $1.count }, color: color, caret: color))
                .font(.mono(size)).lineSpacing(size * 0.34)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .onChange(of: tokensPerSecond) { start = .now }
        .accessibilityLabel("Sample text written at \(Show.speed(tokensPerSecond)) tokens a second")
    }
}
