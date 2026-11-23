import RunItText
import SwiftUI

/// One model: how it will feel, whether it fits, and what to download.
struct DetailPane: View {
    let id: String
    var seed: Choice?
    var code = false
    @Environment(Store.self) private var store
    @State private var detail: Detail?
    @State private var error: String?
    @State private var label: String?

    private var offer: Offer? {
        if let d = detail, let o = d.variants.first(where: { $0.variant.label == label }) ?? d.variants.first { return o }
        return seed.map { Offer(variant: $0.variant, prediction: $0.prediction) }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(seed?.name ?? detail?.name ?? " ").font(.display(32)).fixedSize(horizontal: false, vertical: true)
                Text([detail?.repo ?? seed?.repo, (detail?.license ?? seed?.license).map(Show.license)].compactMap { $0 }.joined(separator: "  ·  "))
                    .font(.sans(12.5)).foregroundStyle(Color.faint).padding(.top, 6).textSelection(.enabled)

                if let o = offer {
                    if let p = o.prediction {
                        if p.tier != .tooBig { feel(p) }
                        Fit(p: p).padding(.top, 26)
                    } else if let cap = detail?.capacity {
                        Text(o.variant.size < cap * 0.8 ? "The file fits in this computer's model memory. RunIt predicts speed for language models only." : "This file is larger than the model memory here.")
                            .font(.sans(13.5)).foregroundStyle(Color.faint).padding(.top, 22)
                    }
                    action(o).padding(.top, 24)
                }
                if let r = detail?.rating ?? seed?.rating {
                    Text("Rated \(String(r.rating)) from \(Show.count(r.votes)) blind votes on LMArena. The rating is for the full model; smaller files lose a little.")
                        .font(.sans(12.5)).lineSpacing(3).foregroundStyle(Color.faint).padding(.top, 18)
                }
                if let error {
                    VStack(alignment: .leading, spacing: 10) {
                        Text(error).font(.sans(13.5)).foregroundStyle(Color.alarm)
                        Button("Try again") { Task { await load() } }.buttonStyle(Outline())
                    }
                    .padding(.top, 22)
                }
                if let d = detail { files(d) } else if error == nil {
                    Text("Checking what can be downloaded").font(.sans(12.5)).foregroundStyle(Color.faint).padding(.top, 28)
                }
            }
            .padding(.horizontal, 28).padding(.top, 50).padding(.bottom, 36)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .scrollIndicators(.never)
        .background(Color.sheet.ignoresSafeArea())
        .task(id: id) { await load() }
    }

    private func load() async {
        detail = nil
        error = nil
        label = seed?.variant.label
        do {
            let d = try await store.detail(id)
            guard !Task.isCancelled else { return }
            if label == nil { label = d.variants.first { $0.variant.label == "Q4_K_M" }?.variant.label ?? d.variants.first?.variant.label }
            withAnimation(.settle) { detail = d }
        } catch {
            guard !Task.isCancelled else { return }
            self.error = error.localizedDescription
        }
    }

    private func feel(_ p: Prediction) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Rolling(value: p.tokensPerS, font: .display(72))
                VStack(alignment: .leading, spacing: 1) {
                    Text("tokens a second").font(.sans(13))
                    Text("\(p.rough ? "rough estimate, " : "")\(Show.speed(p.tokensPerSLow)) to \(Show.speed(p.tokensPerSHigh))").font(.sans(12)).foregroundStyle(Color.faint)
                }
            }
            Text("This is that speed:").font(.sans(12)).foregroundStyle(Color.faint).padding(.top, 8)
            SpeedPreview(tokensPerSecond: p.tokensPerS, code: code)
                .padding(16).frame(height: 214, alignment: .top).clipped()
                .background(Color.paper, in: .rect(cornerRadius: 6)).padding(.top, 8)
        }
        .padding(.top, 16)
    }

    @ViewBuilder private func action(_ o: Offer) -> some View {
        let repo = detail?.repo ?? seed?.repo ?? ""
        if let i = store.item(repo: repo, paths: o.variant.files) {
            if i.status == .ready {
                HStack(spacing: 10) {
                    Button("Try it") { store.openChat(i.key) }.buttonStyle(Solid())
                    Button("Show in Finder") { store.reveal(i.key) }.buttonStyle(Outline())
                }
            } else {
                ItemProgress(item: i)
            }
        } else {
            let gone = detail != nil && detail?.variants.contains { $0.variant.files == o.variant.files || $0.variant.label == o.variant.label } == false
            Button("Download, \(Show.size(o.variant.size))") { store.download(id, o.variant.files) }
                .buttonStyle(Solid()).disabled(gone || detail == nil)
            if o.prediction?.tier == .tooBig {
                Text("This will not fit in memory here. You can still download it for another computer.").font(.sans(12.5)).foregroundStyle(Color.alarm).padding(.top, 8)
            }
        }
    }

    @ViewBuilder private func files(_ d: Detail) -> some View {
        if d.variants.isEmpty && d.others.isEmpty {
            Text(d.note ?? "Hugging Bay does not host files for this one yet.").font(.sans(13.5)).foregroundStyle(Color.faint).padding(.top, 24)
        }
        if !d.variants.isEmpty {
            Text("Sizes").font(.display(20)).padding(.top, 34).padding(.bottom, 8)
            Text("Larger files are closer to the original and slower.").font(.sans(12.5)).foregroundStyle(Color.faint).padding(.bottom, 10)
            Hairline()
            ForEach(d.variants, id: \.variant.files) { o in
                let on = o.variant.label == offer?.variant.label
                Button { withAnimation(.settle) { label = o.variant.label } } label: {
                    HStack {
                        Text(o.variant.label).font(on ? .monoBold(13) : .mono(13))
                        Spacer()
                        Text(Show.size(o.variant.size)).font(.mono(12)).foregroundStyle(Color.faint)
                        SpeedLabel(prediction: o.prediction).frame(width: 108, alignment: .trailing)
                    }
                    .padding(.vertical, 8).padding(.horizontal, 8)
                    .background(on ? Color.paper : .clear).overlay(alignment: .leading) { if on { Rectangle().fill(Color.marker).frame(width: 4) } }
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                Hairline()
            }
        }
        if !d.others.isEmpty {
            DisclosureGroup {
                VStack(spacing: 0) {
                    ForEach(d.others, id: \.path) { f in
                        HStack {
                            Text(f.path).font(.mono(11.5)).lineLimit(1).truncationMode(.middle)
                            Spacer()
                            Text(Show.size(f.size)).font(.mono(11.5)).foregroundStyle(Color.faint)
                            if let i = store.item(repo: d.repo, paths: [f]) {
                                Text(i.status == .ready ? "saved" : "\(Int(i.done / max(i.total, 1) * 100))%").font(.sans(12)).foregroundStyle(Color.faint)
                            } else {
                                Button("Get") { store.download(d.id, [f]) }.buttonStyle(Outline()).controlSize(.small)
                            }
                        }
                        .padding(.vertical, 5)
                    }
                }
                .padding(.top, 8)
            } label: {
                HStack {
                    Text(d.variants.isEmpty ? "Files" : "Other files").font(.display(20))
                    Spacer()
                    if d.variants.isEmpty { Button("Get all, \(Show.size(d.others.reduce(0) { $0 + $1.size }))") { store.download(d.id, d.others) }.buttonStyle(Outline()) }
                }
            }
            .padding(.top, 30)
        }
    }
}

/// Memory: what the model takes, against what there is.
struct Fit: View {
    let p: Prediction
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var grown = false
    private var share: Double { p.needBytes / max(p.capacityBytes, 1) }
    private var word: String {
        switch p.tier {
        case .comfortable: "Fits with room to spare"
        case .tight: "Fits, but close heavy apps first"
        case .partial: "Too big for graphics memory, so \(Int(p.gpuShare * 100))% runs there and the rest on the processor"
        case .tooBig: "Too big for this computer"
        }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            GeometryReader { g in
                RoundedRectangle(cornerRadius: 3).fill(Color.rule)
                RoundedRectangle(cornerRadius: 3).fill(p.tier == .comfortable ? Color.ink : Color.alarm)
                    .frame(width: g.size.width * min(1, grown ? share : 0))
            }
            .frame(height: 8)
            HStack(alignment: .firstTextBaseline) {
                Text(word).font(.sans(13)).fixedSize(horizontal: false, vertical: true)
                Spacer()
                Text("\(Show.memory(p.needBytes)) of \(Show.memory(p.capacityBytes))").font(.mono(12)).foregroundStyle(Color.faint)
            }
            if p.tier != .tooBig {
                VStack(spacing: 5) {
                    fact("Reads your prompt at", "\(Show.speed(p.promptTokensPerS)) tokens/s")
                    fact("In a long conversation", "\(Show.speed(p.tokensPerSDeep)) tokens/s")
                    if p.tokensPerSSustained < p.tokensPerS * 0.95 { fact("After 20 s of steady work", "\(Show.speed(p.tokensPerSSustained)) tokens/s") }
                }
                .padding(.top, 8)
            }
        }
        .onAppear { if still { grown = true } else { withAnimation(.smooth(duration: 0.7)) { grown = true } } }
        .animation(.settle, value: share)
    }
    private func fact(_ k: String, _ v: String) -> some View {
        HStack { Text(k).font(.sans(12.5)).foregroundStyle(Color.faint); Spacer(); Text(v).font(.mono(12.5)) }
    }
}

struct ItemProgress: View {
    let item: Item
    @Environment(Store.self) private var store
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            GeometryReader { g in
                RoundedRectangle(cornerRadius: 3).fill(Color.rule)
                RoundedRectangle(cornerRadius: 3).fill(item.status == .failed ? Color.alarm : Color.ink)
                    .frame(width: g.size.width * item.done / max(item.total, 1)).animation(.linear(duration: 0.15), value: item.done)
            }
            .frame(height: 8)
            HStack(spacing: 8) {
                Text(line).font(.mono(12)).foregroundStyle(item.status == .failed ? Color.alarm : Color.faint).lineLimit(2).contentTransition(.numericText())
                Spacer()
                switch item.status {
                case .active, .queued: Button("Pause") { store.act("pause", item.key) }.buttonStyle(Outline())
                case .paused: Button("Resume") { store.act("resume", item.key) }.buttonStyle(Outline())
                case .failed: Button("Try again") { store.act("resume", item.key) }.buttonStyle(Outline())
                case .ready: EmptyView()
                }
                Button(item.status == .failed ? "Remove" : "Cancel") { store.act("remove", item.key) }.buttonStyle(Outline())
            }
        }
    }
    private var line: String {
        switch item.status {
        case .queued: "Waiting"
        case .failed: item.error ?? "Failed"
        case .paused: "Paused at \(Show.size(item.done)) of \(Show.size(item.total))"
        default: item.done >= item.total ? "Checking" : "\(Show.size(item.done)) of \(Show.size(item.total))  \(Show.size(item.bytesPerS))/s  \(Show.left(item.total - item.done, at: item.bytesPerS))"
        }
    }
}
