import RunItText
import SwiftUI

struct ForYou: View {
    @Environment(Store.self) private var store
    @State private var selected: String?

    var body: some View {
        if let s = store.shortlist {
            HStack(spacing: 0) {
                ScrollView { list(s).padding(.horizontal, 40).padding(.top, 50).padding(.bottom, 40) }.scrollIndicators(.never)
                Rectangle().fill(Color.rule).frame(width: 1).ignoresSafeArea()
                if let c = choice(s) {
                    DetailPane(id: c.id, seed: c, code: pickFor(s, c)?.role == "code").frame(width: 400)
                }
            }
            .onAppear { if selected == nil { selected = s.picks.first?.choice.id ?? s.ranked.first?.id ?? s.unrated.first?.id } }
        } else if let e = store.shortlistError {
            VStack(alignment: .leading, spacing: 16) {
                Text("The catalog is out of reach.").font(.display(40))
                Text(e).font(.sans(15)).foregroundStyle(Color.faint)
                Button("Try again") { Task { await store.recommend() } }.buttonStyle(Solid())
            }
            .frame(maxWidth: 520, alignment: .leading)
        } else {
            VStack(alignment: .leading, spacing: 18) {
                Typed(text: "Reading the catalog.", font: .display(40), perSecond: 30)
                GeometryReader { g in
                    Rectangle().fill(Color.rule)
                    Rectangle().fill(Color.ink).frame(width: g.size.width * (store.catalogProgress ?? 0)).animation(.smooth, value: store.catalogProgress)
                }
                .frame(width: 320, height: 2)
                Text("Sizing up every model hosted on Hugging Bay.").font(.sans(14)).foregroundStyle(Color.faint)
            }
        }
    }

    private func choice(_ s: Shortlist) -> Choice? {
        (s.picks.map(\.choice) + s.ranked + s.unrated).first { $0.id == selected }
    }
    private func pickFor(_ s: Shortlist, _ c: Choice) -> Pick? { s.picks.first { $0.choice.id == c.id } }

    private func list(_ s: Shortlist) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if let best = s.picks.first {
                Verdict(pick: best)
            } else {
                Text("Nothing rated fits this computer comfortably.").font(.display(40))
                Text("Smaller models below may still be worth a try.").font(.sans(15)).foregroundStyle(Color.faint).padding(.top, 10)
            }
            VStack(spacing: 0) {
                ForEach(Array(s.picks.enumerated()), id: \.element.id) { n, p in
                    PickRow(pick: p, on: selected == p.choice.id) { withAnimation(.settle) { selected = p.choice.id } }
                        .modifier(Rise(order: n))
                    Hairline()
                }
            }
            .overlay(alignment: .top) { Hairline() }
            .padding(.top, 36)

            if s.ranked.count > 0 {
                heading("Rated, and runs here", "Rating: LMArena, \(s.ratingsDate). Higher is better.")
                table(s.ranked.prefix(14).map { $0 })
            }
            if !s.unrated.isEmpty {
                heading("Popular, not rated yet", "From the model's own publisher, by downloads.")
                table(s.unrated)
            }
        }
        .frame(maxWidth: 720, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func heading(_ t: String, _ sub: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(t).font(.display(26))
            Text(sub).font(.sans(12.5)).foregroundStyle(Color.faint)
        }
        .padding(.top, 48).padding(.bottom, 14)
    }

    private func table(_ rows: [Choice]) -> some View {
        VStack(spacing: 0) {
            Hairline()
            ForEach(rows) { c in
                ModelRow(name: c.name, sub: c.variant.label + ", " + Show.size(c.variant.size), rating: c.rating?.rating, prediction: c.prediction, on: selected == c.id) {
                    withAnimation(.settle) { selected = c.id }
                }
                Hairline()
            }
        }
    }
}

/// Fade and lift, one after another.
struct Rise: ViewModifier {
    let order: Int
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var up = false
    func body(content: Content) -> some View {
        content.opacity(up || still ? 1 : 0).offset(y: up || still ? 0 : 10)
            .onAppear { withAnimation(.settle.delay(0.55 + Double(order) * 0.09)) { up = true } }
    }
}

private struct Verdict: View {
    let pick: Pick
    @Environment(Store.self) private var store
    private var chip: String { store.report?.machine.cpu ?? "computer" }
    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Typed(text: "Your \(chip) is at its best with", font: .display(46), perSecond: 50, caret: false)
            Marked(text: pick.choice.name, size: 52, delay: 0.7)
            Text(sentence).font(.sans(16)).lineSpacing(4).foregroundStyle(Color.faint).padding(.top, 14).frame(maxWidth: 520, alignment: .leading)
        }
    }
    private var sentence: String {
        let p = pick.choice.prediction
        var s = "It should write \(Show.speed(p.tokensPerSLow)) to \(Show.speed(p.tokensPerSHigh)) tokens a second and leave \(Show.memory(max(0, p.capacityBytes - p.needBytes))) of model memory free."
        let names = ["quick": "the quickest", "code": "best for code", "smart": "the smartest it can hold"]
        let also = pick.also.compactMap { names[$0] }
        if !also.isEmpty { s += " Also \(also.joined(separator: " and "))." }
        return s
    }
}

private struct PickRow: View {
    let pick: Pick
    let on: Bool
    let act: () -> Void
    @State private var over = false
    var body: some View {
        Button(action: act) {
            HStack(alignment: .firstTextBaseline, spacing: 18) {
                VStack(alignment: .leading, spacing: 5) {
                    Text(pick.title).font(.sans(12.5)).foregroundStyle(Color.faint)
                    Text(pick.choice.name).font(.display(27))
                    Text(pick.why).font(.sans(13.5)).foregroundStyle(Color.faint).lineLimit(2).multilineTextAlignment(.leading)
                }
                Spacer(minLength: 10)
                VStack(alignment: .trailing, spacing: 0) {
                    Rolling(value: pick.choice.prediction.tokensPerS, font: .display(38))
                    Text("tokens/s").font(.sans(11.5)).foregroundStyle(Color.faint)
                }
            }
            .padding(.vertical, 17).padding(.horizontal, 14)
            .background(on ? Color.sheet : over ? Color.sheet.opacity(0.5) : .clear)
            .overlay(alignment: .leading) { if on { Rectangle().fill(Color.marker).frame(width: 4) } }
            .contentShape(.rect)
        }
        .buttonStyle(.plain).onHover { over = $0 }
    }
}

struct ModelRow: View {
    let name: String
    let sub: String
    var rating: Int?
    let prediction: Prediction?
    let on: Bool
    let act: () -> Void
    @State private var over = false
    var body: some View {
        Button(action: act) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(name).font(.sansBold(14)).lineLimit(1)
                    Text(sub).font(.sans(12)).foregroundStyle(Color.faint).lineLimit(1)
                }
                Spacer(minLength: 8)
                if let rating { Text(String(rating)).font(.mono(13)).foregroundStyle(Color.faint).frame(width: 48, alignment: .trailing) }
                SpeedLabel(prediction: prediction).frame(width: 112, alignment: .trailing)
            }
            .padding(.vertical, 10).padding(.horizontal, 14)
            .background(on ? Color.sheet : over ? Color.sheet.opacity(0.5) : .clear)
            .overlay(alignment: .leading) { if on { Rectangle().fill(Color.marker).frame(width: 4) } }
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
                Text("too big").font(.sans(13)).foregroundStyle(Color.faint)
            } else {
                HStack(alignment: .firstTextBaseline, spacing: 4) {
                    if p.tier != .comfortable { Text(p.tier == .tight ? "tight" : "split").font(.sans(11)).foregroundStyle(Color.alarm) }
                    Text((p.rough ? "~" : "") + Show.speed(p.tokensPerS)).font(.monoBold(15))
                    Text("tok/s").font(.sans(11)).foregroundStyle(Color.faint)
                }
            }
        } else {
            EmptyView()
        }
    }
}
