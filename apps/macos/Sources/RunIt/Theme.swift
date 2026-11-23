import AppKit
import CoreText
import SwiftUI

extension Color {
    init(_ h: UInt32) { self.init(.sRGB, red: Double(h >> 16 & 255) / 255, green: Double(h >> 8 & 255) / 255, blue: Double(h & 255) / 255) }
    static let ground = Color(0x1B1B1D)
    static let card = Color(0x262629)
    static let raised = Color(0x2E2E32)
    static let line = Color(0x303034)
    static let off = Color(0x37373B)
    static let text = Color(0xF4F4F2)
    static let soft = Color(0xA6A6AA)
    static let ink = Color(0x111111)
    static let orange = Color(0xFF7A00)
    static let alarm = Color(0xFF6B52)
    /// Flat blocks. Ink reads on all of them.
    static let tints: [Color] = [.orange, Color(0xFFD400), Color(0xF4F4F2), Color(0xFF5A3C), Color(0x62C8FF)]
}

extension Font {
    static func sans(_ s: CGFloat) -> Font { .custom("RunItSans-Regular", fixedSize: s) }
    static func bold(_ s: CGFloat) -> Font { .custom("RunItSans-Bold", fixedSize: s) }
    static func mono(_ s: CGFloat) -> Font { .custom("RunItMono-Regular", fixedSize: s) }
    static func pixel(_ s: CGFloat) -> Font { .custom("RunItPixel-Regular", fixedSize: s) }
}

enum Fonts {
    static func register() {
        let dir = Bundle.main.resourceURL?.appending(path: "fonts")
        let files = dir.flatMap { try? FileManager.default.contentsOfDirectory(at: $0, includingPropertiesForKeys: nil) } ?? []
        CTFontManagerRegisterFontURLs(files.filter { $0.pathExtension == "ttf" } as CFArray, .process, true, nil)
    }
}

extension Animation {
    static let settle = Animation.spring(response: 0.38, dampingFraction: 0.84)
    static let hop = Animation.spring(response: 0.28, dampingFraction: 0.5)
}

/// A flat block. Pressing it sinks it.
struct Block: ButtonStyle {
    var fill = Color.orange
    var label = Color.ink
    var tall = false
    @Environment(\.isEnabled) private var enabled
    @State private var over = false
    func makeBody(configuration c: Configuration) -> some View {
        c.label.font(.bold(tall ? 15 : 13)).foregroundStyle(label).lineLimit(1).fixedSize()
            .padding(.horizontal, tall ? 20 : 14).frame(height: tall ? 44 : 34)
            .background(fill, in: .rect(cornerRadius: 8))
            .brightness(over && !c.isPressed ? 0.06 : 0)
            .offset(y: c.isPressed ? 2 : 0)
            .opacity(enabled ? 1 : 0.45).contentShape(.rect).onHover { over = $0 }
            .animation(.easeOut(duration: 0.1), value: c.isPressed)
    }
}

struct Plain: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    @State private var over = false
    func makeBody(configuration c: Configuration) -> some View {
        c.label.font(.sans(13)).foregroundStyle(Color.text).lineLimit(1).fixedSize().padding(.horizontal, 12).frame(height: 32)
            .background(c.isPressed ? Color.off : over ? Color.raised : Color.card, in: .rect(cornerRadius: 8))
            .opacity(enabled ? 1 : 0.45).contentShape(.rect).onHover { over = $0 }
    }
}

struct Hairline: View {
    var body: some View { Rectangle().fill(Color.line).frame(height: 1) }
}
