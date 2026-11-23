import RunItText
import SwiftUI

struct ForYou: View {
    @Environment(Store.self) private var store
    var body: some View {
        if let s = store.shortlist {
            ScrollView { page(s).padding(.horizontal, 36).padding(.top, 44).padding(.bottom, 36) }.scrollIndicators(.never)
        } else if let e = store.shortlistError {
            VStack(alignment: .leading, spacing: 16) {
                Creature(name: "offline", side: 100, color: .alarm)
                Text("The catalog is out of reach").font(.bold(26))
                Text(e).font(.sans(15)).foregroundStyle(Color.soft)
                Button("Try again") { Task { await store.recommend() } }.buttonStyle(Block())
            }
            .frame(maxWidth: 480, alignment: .leading)
        } else {
            VStack(spacing: 22) {
                Creature(name: store.report?.machine.cpu ?? "RunIt", side: 120, color: .orange, pace: 0.2)
                Text("Sizing up every model on Hugging Bay").font(.bold(18))
                Blocks(share: store.catalogProgress ?? 0, count: 30)
            }
        }
    }

    /// Up to three cards: the picks, then the next best so there is always a choice.
    private func cards(_ s: Shortlist) -> [(Choice, String, Bool)] {
        var out: [(Choice, String, Bool)] = s.picks.prefix(3).map { p in
            let spare = Show.memory(max(0, p.choice.prediction.capacityBytes - p.choice.prediction.needBytes))
            let line = switch p.role {
            case "best": p.choice.prediction.tokensPerS >= 10 ? "Best for this Mac. \(spare) to spare." : "The best this Mac can run, slowly."
            case "quick": "Quickest worth using."
            case "code": "Best for code."
            default: p.choice.prediction.tier == .tight ? "Smartest it can hold. A tight fit." : "Smartest it can hold."
            }
            return (p.choice, line, p.role == "code")
        }
        for c in s.ranked + s.unrated where out.count < 3 && !out.contains(where: { $0.0.id == c.id }) {
            out.append((c, c.rating == nil ? "Popular, not rated yet." : "Rated \(c.rating!.rating). Runs well too.", false))
        }
        return out
    }

    private func page(_ s: Shortlist) -> some View {
        let cards = cards(s)
        let shown = Set(cards.map(\.0.id))
        let ranked = s.ranked.filter { !shown.contains($0.id) }.prefix(12)
        let unrated = s.unrated.filter { !shown.contains($0.id) }
        return VStack(alignment: .leading, spacing: 0) {
            Text(s.picks.isEmpty ? "Nothing rated fits comfortably" : "Your Mac runs these well").font(.bold(27)).tracking(-0.6)
            Text(line).font(.sans(14)).foregroundStyle(Color.soft).padding(.top, 6)
            Shelf {
                ForEach(Array(cards.enumerated()), id: \.element.0.id) { n, c in
                    Card(choice: c.0, line: c.1, code: c.2, big: n == 0).modifier(Arrive(order: n))
                }
            }
            .padding(.top, 22)
            if !ranked.isEmpty {
                heading("More that run here", "LMArena rating, \(s.ratingsDate)")
                table(Array(ranked))
            }
            if !unrated.isEmpty {
                heading("Popular, not rated yet", "From the model's own publisher")
                table(unrated)
            }
        }
        .frame(maxWidth: 1180, alignment: .leading).frame(maxWidth: .infinity)
    }

    private var line: String {
        guard let r = store.report else { return "" }
        return "\(r.machine.cpu), \(Int((r.machine.ramBytes / 1_073_741_824).rounded())) GB. Tested at \(Int(r.bytesPerS / 1e9)) GB of model a second."
    }

    private func heading(_ t: String, _ sub: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(t).font(.bold(16))
            Spacer()
            Text(sub).font(.sans(12.5)).foregroundStyle(Color.soft)
        }
        .padding(.top, 34).padding(.bottom, 10)
    }

    private func table(_ rows: [Choice]) -> some View {
        VStack(spacing: 0) {
            ForEach(rows) { c in
                Hairline()
                ModelRow(name: c.name, sub: Show.size(c.variant.size), rating: c.rating?.rating, prediction: c.prediction) {
                    withAnimation(.settle) { store.opened = Opened(id: c.id, name: c.name, seed: c) }
                }
            }
        }
    }
}

/// One wide card, then narrower ones.
private struct Shelf: Layout {
    private let gap: CGFloat = 14
    private func widths(_ total: CGFloat, _ n: Int) -> [CGFloat] {
        guard n > 0 else { return [] }
        let room = total - gap * CGFloat(n - 1), part = room / (1.55 + CGFloat(n - 1))
        return [n == 1 ? total : part * 1.55] + Array(repeating: part, count: n - 1)
    }
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let w = proposal.width ?? 900
        let h = zip(subviews, widths(w, subviews.count)).map { $0.sizeThatFits(.init(width: $1, height: nil)).height }.max() ?? 0
        return CGSize(width: w, height: h)
    }
    func placeSubviews(in b: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = b.minX
        for (v, w) in zip(subviews, widths(b.width, subviews.count)) {
            v.place(at: .init(x: x, y: b.minY), proposal: .init(width: w, height: b.height))
            x += w + gap
        }
    }
}

/// Cards drop in one after another.
private struct Arrive: ViewModifier {
    let order: Int
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var here = false
    func body(content: Content) -> some View {
        content.opacity(here || still ? 1 : 0).offset(y: here || still ? 0 : 24)
            .onAppear { withAnimation(.spring(response: 0.45, dampingFraction: 0.72).delay(0.05 + Double(order) * 0.08)) { here = true } }
    }
}

private struct Card: View {
    let choice: Choice
    let line: String
    let code: Bool
    let big: Bool
    @Environment(Store.self) private var store
    @State private var over = false

    var body: some View {
        let tint = store.tint(choice.name)
        VStack(spacing: 0) {
            ZStack(alignment: .topLeading) {
                tint
                if big {
                    HStack(alignment: .top, spacing: 26) {
                        Creature(name: choice.name, side: 110).offset(y: over ? -8 : 0)
                        SpeedPreview(tokensPerSecond: choice.prediction.tokensPerS, code: code, size: 13, limit: 26).frame(height: 132)
                    }
                    .padding(.top, 24).padding(.leading, 26).padding(.trailing, 20)
                } else {
                    Creature(name: choice.name, side: 110).offset(y: over ? -8 : 0).frame(maxWidth: .infinity).padding(.top, 24)
                }
                HStack(alignment: .firstTextBaseline, spacing: 7) {
                    Figure(value: choice.prediction.tokensPerS, size: 64)
                    Text("tokens/s").font(.bold(12.5)).foregroundStyle(Color.ink)
                }
                .padding(.leading, 20).padding(.bottom, 4)
                .frame(maxHeight: .infinity, alignment: .bottomLeading)
            }
            .frame(height: 224)
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(choice.name).font(.bold(16)).lineLimit(1)
                    Text(line).font(.sans(13)).foregroundStyle(Color.soft).lineLimit(2).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
                if big { Get(id: choice.id, repo: choice.repo, variant: choice.variant, fill: tint) }
            }
            .padding(.horizontal, 18).padding(.vertical, 15).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .background(Color.card)
        }
        .clipShape(.rect(cornerRadius: 14))
        .offset(y: over ? -3 : 0)
        .animation(.hop, value: over)
        .contentShape(.rect)
        .onHover { over = $0 }
        .onTapGesture { open() }
        .accessibilityElement(children: .contain)
        .accessibilityAction(named: "Open") { open() }
    }
    private func open() { withAnimation(.settle) { store.opened = Opened(id: choice.id, name: choice.name, seed: choice, code: code) } }
}

/// Download, progress, or try it: whatever comes next for this file.
struct Get: View {
    let id: String
    let repo: String
    let variant: Variant
    var fill: Color = .orange
    var label: Color = .ink
    var tall = false
    var enabled = true
    @Environment(Store.self) private var store
    var body: some View {
        if let i = store.item(repo: repo, paths: variant.files) {
            switch i.status {
            case .ready: Button("Try it") { store.openChat(i.key) }.buttonStyle(Block(fill: fill, label: label, tall: tall)).disabled(!i.runnable)
            case .paused, .failed: Button(i.status == .paused ? "Resume" : "Try again") { store.act("resume", i.key) }.buttonStyle(Block(fill: fill, label: label, tall: tall))
            default: Button("\(Int(i.done / max(i.total, 1) * 100))%") { withAnimation(.settle) { store.go(.library) } }.buttonStyle(Block(fill: fill, label: label, tall: tall))
            }
        } else {
            Button("Get \(Show.size(variant.size))") { store.download(id, variant.files) }.buttonStyle(Block(fill: fill, label: label, tall: tall)).disabled(!enabled)
        }
    }
}

struct ModelRow: View {
    let name: String
    let sub: String
    var rating: Int?
    let prediction: Prediction?
    let act: () -> Void
    @Environment(Store.self) private var store
    @State private var over = false
    var body: some View {
        Button(action: act) {
            HStack(spacing: 14) {
                Tile(name: name, side: 34).offset(y: over ? -3 : 0).animation(.hop, value: over)
                VStack(alignment: .leading, spacing: 2) {
                    Text(name).font(.bold(14)).lineLimit(1)
                    Text(sub).font(.sans(12)).foregroundStyle(Color.soft).lineLimit(1)
                }
                Spacer(minLength: 10)
                if let p = prediction, p.tier != .tooBig { Blocks(share: p.tokensPerS / 60, color: store.tint(name)) }
                Text(rating.map(String.init) ?? "").font(.sans(13)).foregroundStyle(Color.soft).frame(width: 50, alignment: .trailing)
                SpeedLabel(prediction: prediction).frame(width: 106, alignment: .trailing)
            }
            .padding(.horizontal, 8).frame(height: 54)
            .background(over ? Color.card : .clear, in: .rect(cornerRadius: 8))
            .contentShape(.rect)
        }
        .buttonStyle(.plain).onHover { over = $0 }
    }
}

struct SpeedLabel: View {
    let prediction: Prediction?
    var body: some View {
        if let p = prediction {
            if p.tier == .tooBig {
                Text("too big").font(.sans(13)).foregroundStyle(Color.soft)
            } else {
                HStack(alignment: .firstTextBaseline, spacing: 5) {
                    if p.tier != .comfortable { Text(p.tier == .tight ? "tight" : "split").font(.bold(11)).foregroundStyle(Color.alarm) }
                    Text((p.rough ? "~" : "") + Show.speed(p.tokensPerS)).font(.pixel(25))
                    Text("tok/s").font(.sans(11)).foregroundStyle(Color.soft)
                }
            }
        }
    }
}
