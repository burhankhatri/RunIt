import AppKit
import CoreText
import SwiftUI

extension Color {
    init(light: UInt32, dark: UInt32) {
        self.init(nsColor: NSColor(name: nil) { a in
            let h = a.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light
            return NSColor(srgbRed: CGFloat(h >> 16 & 255) / 255, green: CGFloat(h >> 8 & 255) / 255, blue: CGFloat(h & 255) / 255, alpha: 1)
        })
    }
    static let paper = Color(light: 0xE8ECE1, dark: 0x0C1511)
    static let sheet = Color(light: 0xF3F6EE, dark: 0x131F19)
    static let ink = Color(light: 0x0F2519, dark: 0xE7EEE4)
    static let faint = Color(light: 0x56685C, dark: 0x93A599)
    static let rule = Color(light: 0xCBD3C5, dark: 0x25362C)
    /// The marker. Text on it is always dark.
    static let marker = Color(light: 0xF1FB5F, dark: 0xD9EE4E)
    static let onMarker = Color(light: 0x0F2519, dark: 0x0F2519)
    static let alarm = Color(light: 0xA8321A, dark: 0xF08A6B)
}

extension Font {
    static func display(_ s: CGFloat) -> Font { .custom("RunItDisplay-Regular", fixedSize: s) }
    static func displayItalic(_ s: CGFloat) -> Font { .custom("RunItDisplay-Italic", fixedSize: s) }
    static func reading(_ s: CGFloat) -> Font { .custom("RunItText-Regular", fixedSize: s) }
    static func sans(_ s: CGFloat) -> Font { .custom("RunItSans-Regular", fixedSize: s) }
    static func sansBold(_ s: CGFloat) -> Font { .custom("RunItSans-Bold", fixedSize: s) }
    static func mono(_ s: CGFloat) -> Font { .custom("RunItMono-Regular", fixedSize: s) }
    static func monoBold(_ s: CGFloat) -> Font { .custom("RunItMono-Bold", fixedSize: s) }
}

enum Fonts {
    static func register() {
        let dir = Bundle.main.resourceURL?.appending(path: "fonts")
        let files = dir.flatMap { try? FileManager.default.contentsOfDirectory(at: $0, includingPropertiesForKeys: nil) } ?? []
        CTFontManagerRegisterFontURLs(files.filter { $0.pathExtension == "ttf" } as CFArray, .process, true, nil)
    }
}

extension Animation {
    static let settle = Animation.spring(response: 0.42, dampingFraction: 0.86)
}

struct Solid: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration c: Configuration) -> some View {
        c.label.font(.sansBold(14)).foregroundStyle(Color.paper).padding(.horizontal, 18).frame(height: 40)
            .background(Color.ink.opacity(c.isPressed ? 0.82 : 1), in: .rect(cornerRadius: 6))
            .opacity(enabled ? 1 : 0.4).contentShape(.rect)
            .scaleEffect(c.isPressed ? 0.985 : 1).animation(.easeOut(duration: 0.12), value: c.isPressed)
    }
}

struct Outline: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    @State private var over = false
    func makeBody(configuration c: Configuration) -> some View {
        c.label.font(.sans(13)).foregroundStyle(Color.ink).padding(.horizontal, 12).frame(height: 30)
            .background(Color.ink.opacity(c.isPressed ? 0.1 : over ? 0.05 : 0), in: .rect(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Color.rule))
            .opacity(enabled ? 1 : 0.4).contentShape(.rect).onHover { over = $0 }
    }
}

struct Hairline: View {
    var body: some View { Rectangle().fill(Color.rule).frame(height: 1) }
}

/// Text a highlighter has been drawn across.
struct Marked: View {
    let text: String
    let size: CGFloat
    var delay = 0.0
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var drawn = false
    var body: some View {
        Text(text).font(.displayItalic(size)).foregroundStyle(drawn ? Color.onMarker : Color.ink)
            .padding(.horizontal, size * 0.12)
            .background(alignment: .leading) {
                GeometryReader { g in
                    Path { p in
                        // A marker is never quite level.
                        p.move(to: .init(x: 0, y: g.size.height * 0.16))
                        p.addLine(to: .init(x: g.size.width, y: g.size.height * 0.10))
                        p.addLine(to: .init(x: g.size.width - 3, y: g.size.height * 0.93))
                        p.addLine(to: .init(x: 2, y: g.size.height * 0.97))
                        p.closeSubpath()
                    }
                    .fill(Color.marker)
                    .mask(alignment: .leading) { Rectangle().frame(width: drawn ? g.size.width : 0) }
                }
            }
            .padding(.leading, -size * 0.12)
            .onAppear {
                if still { drawn = true } else { withAnimation(.easeInOut(duration: 0.55).delay(delay)) { drawn = true } }
            }
    }
}
