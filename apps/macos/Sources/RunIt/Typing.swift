import RunItText
import SwiftUI

/// Text that writes itself. Space for the whole text is held from the start, so nothing shifts.
struct Typed: View {
    let text: String
    var font: Font
    var color: Color = .ink
    var perSecond = 46.0
    var delay = 0.0
    var caret = true
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
                TimelineView(.animation) { t in
                    Text(frame(at: t.date))
                }
            }
        }
        .font(font)
        .accessibilityLabel(text)
        .task(id: text) {
            if still { return done() }
            finished = false
            start = .now.addingTimeInterval(delay)
            try? await Task.sleep(for: .seconds(delay + (Double(chars.count) + Reveal.trail) / perSecond + 0.05))
            guard !Task.isCancelled else { return }
            finished = true
            done()
        }
    }

    private func frame(at now: Date) -> AttributedString {
        let shown = start.map { max(0, now.timeIntervalSince($0) * perSecond) } ?? 0
        let r = Reveal(shown: shown, count: chars.count)
        var out = AttributedString(String(chars[0..<r.solid]))
        out.foregroundColor = color
        for i in r.fading {
            var c = AttributedString(String(chars[i]))
            c.foregroundColor = color.opacity(r.opacity(i))
            out += c
        }
        if let at = r.caret {
            var next = AttributedString(String(chars[at]))
            next.foregroundColor = .clear
            if caret, shown > 0 { next.backgroundColor = .marker }
            var rest = AttributedString(String(chars[(at + 1)...]))
            rest.foregroundColor = .clear
            out += next + rest
        }
        return out
    }
}

/// A sample reply written at exactly the speed a model is predicted to write.
struct SpeedPreview: View {
    let tokensPerSecond: Double
    var code = false
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var start = Date.now

    var body: some View {
        let tokens = code ? Samples.code : Samples.prose
        let length = Double(tokens.count) / max(tokensPerSecond, 0.1)
        TimelineView(.animation(paused: still)) { t in
            let at = still ? length : t.date.timeIntervalSince(start).truncatingRemainder(dividingBy: length + 2.4)
            Text(frame(tokens, Int(at * tokensPerSecond)))
                .font(code ? .mono(12.5) : .reading(16)).lineSpacing(code ? 4 : 5)
                .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .onChange(of: tokensPerSecond) { start = .now }
        .accessibilityLabel("Sample text written at \(Show.speed(tokensPerSecond)) tokens a second")
    }

    private func frame(_ tokens: [String], _ n: Int) -> AttributedString {
        var a = AttributedString(tokens.prefix(n).joined())
        a.foregroundColor = .ink
        if n < tokens.count {
            var next = AttributedString(tokens[n])
            next.foregroundColor = .clear
            var head = AttributedString(String(tokens[n].prefix(1)))
            head.foregroundColor = .clear
            head.backgroundColor = .marker
            next = head + { var r = AttributedString(String(tokens[n].dropFirst())); r.foregroundColor = .clear; return r }()
            var rest = AttributedString(tokens.suffix(from: n + 1).joined())
            rest.foregroundColor = .clear
            a += next + rest
        }
        return a
    }
}

/// A figure that rolls to its value.
struct Rolling: View {
    let value: Double
    var font: Font
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var shown = 0.0
    var body: some View {
        Text(Show.speed(shown)).font(font).monospacedDigit().contentTransition(.numericText(value: shown))
            .onAppear { set(value) }.onChange(of: value) { set(value) }
    }
    private func set(_ v: Double) {
        if still { shown = v } else { withAnimation(.smooth(duration: 0.6)) { shown = v } }
    }
}
