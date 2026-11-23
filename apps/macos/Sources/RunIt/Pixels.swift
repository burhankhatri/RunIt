import RunItText
import SwiftUI

/// Square cells with no soft edges.
private func cells(_ rows: [UInt16], _ side: CGFloat, keep: (Int, Int) -> Bool = { _, _ in true }) -> Path {
    var p = Path()
    let c = side / 10
    for (y, row) in rows.enumerated() {
        for x in 0..<10 where row >> x & 1 == 1 && keep(x, y) {
            p.addRect(CGRect(x: CGFloat(x) * c, y: CGFloat(y) * c, width: c, height: c))
        }
    }
    return p
}

/// A model's creature, walking on the spot.
struct Creature: View {
    let name: String
    let side: CGFloat
    var color: Color = .ink
    var pace = 0.46
    @Environment(Store.self) private var store
    @Environment(\.accessibilityReduceMotion) private var still

    var body: some View {
        let frames = store.creature(name)
        TimelineView(.animation(minimumInterval: pace, paused: still)) { t in
            let n = still ? 0 : Int(t.date.timeIntervalSinceReferenceDate / pace) % 2
            Canvas(rendersAsynchronously: false) { g, _ in
                if frames.count == 2 { g.fill(cells(frames[n], side), with: .color(color), style: FillStyle(antialiased: false)) }
            }
        }
        .frame(width: side, height: side)
        .accessibilityHidden(true)
    }
}

/// A creature on its colour.
struct Tile: View {
    let name: String
    var side: CGFloat = 44
    @Environment(Store.self) private var store
    var body: some View {
        RoundedRectangle(cornerRadius: side * 0.2).fill(store.tint(name))
            .overlay(Creature(name: name, side: (side * 0.066).rounded() * 10))
            .frame(width: side, height: side)
    }
}

/// This computer's creature, put together one block at a time as the test runs.
struct Building: View {
    let name: String
    let progress: Double
    let side: CGFloat
    @Environment(Store.self) private var store
    @Environment(\.accessibilityReduceMotion) private var still

    private var order: [(Int, Int)] {
        let rows = store.creature(name).first ?? []
        guard rows.count == 10 else { return [] }
        var all: [(Int, Int)] = []
        for y in 0..<10 { for x in 0..<10 where rows[y] >> x & 1 == 1 { all.append((x, y)) } }
        // Scattered, but the same every time.
        func rank(_ c: (Int, Int)) -> Int { (c.0 * 37 + c.1 * 59) % 101 }
        return all.sorted { rank($0) < rank($1) }
    }

    var body: some View {
        let order = order
        TimelineView(.animation(minimumInterval: 1 / 30, paused: still)) { t in
            let blink = Int(t.date.timeIntervalSinceReferenceDate * 5) % 2 == 0
            Canvas { g, _ in draw(&g, order, blink) }
        }
        .frame(width: side, height: side)
        .accessibilityLabel("Test \(Int(progress * 100)) percent done")
    }

    private func draw(_ g: inout GraphicsContext, _ order: [(Int, Int)], _ blink: Bool) {
        let c: CGFloat = side / 10
        for y in 0..<10 { for x in 0..<10 {
            let r = CGRect(x: CGFloat(x) * c + 1, y: CGFloat(y) * c + 1, width: c - 2, height: c - 2)
            g.fill(Path(r), with: .color(.card))
        } }
        guard progress > 0 else { return }
        let n = Int(progress * Double(order.count))
        for (i, at) in order.prefix(n + 1).enumerated() {
            // The block being laid blinks; the rest are solid.
            let dim = i == n && progress < 1 && !blink
            let r = CGRect(x: CGFloat(at.0) * c, y: CGFloat(at.1) * c, width: c, height: c)
            g.fill(Path(r), with: .color(.orange.opacity(dim ? 0.25 : 1)), style: FillStyle(antialiased: false))
        }
    }
}

/// A row of blocks that light up one after another.
struct Blocks: View {
    let share: Double
    var count = 24
    var width: CGFloat = 9
    var height: CGFloat = 14
    var color: Color = .orange
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var lit = 0

    private var want: Int { share <= 0 ? 0 : max(1, min(count, Int((share * Double(count)).rounded()))) }

    var body: some View {
        HStack(spacing: 3) {
            ForEach(0..<count, id: \.self) { i in
                Rectangle().fill(i < lit ? color : .off).frame(width: width, height: height)
                    .animation(still ? nil : .linear(duration: 0.06).delay(Double(i) * 0.016), value: lit)
            }
        }
        .onAppear { lit = want }
        .onChange(of: want) { lit = want }
        .accessibilityHidden(true)
    }
}

/// A figure that counts up.
struct Figure: View {
    let value: Double
    let size: CGFloat
    var color: Color = .ink
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var shown = 0.0
    var body: some View {
        Counting(value: shown, size: size, color: color)
            .onAppear { set() }.onChange(of: value) { set() }
            .accessibilityLabel(Show.speed(value))
    }
    private func set() { if still { shown = value } else { withAnimation(.easeOut(duration: 0.55)) { shown = value } } }

    private struct Counting: View, Animatable {
        var value: Double
        let size: CGFloat
        let color: Color
        var animatableData: Double { get { value } set { value = newValue } }
        var body: some View { Text(Show.speed(value)).font(.pixel(size)).foregroundStyle(color).lineLimit(1).fixedSize() }
    }
}
