import RunItText
import SwiftUI

/// One model: how it will feel, whether it fits, and what to get.
struct ModelPage: View {
    let opened: Opened
    @Environment(Store.self) private var store
    @State private var detail: Detail?
    @State private var error: String?
    @State private var label: String?
    @State private var showOthers = false

    private var seed: Choice? { opened.seed }
    private var name: String { seed?.name ?? detail?.name ?? opened.name }
    private var tint: Color { store.tint(name) }
    private var offer: Offer? {
        if let d = detail, let o = d.variants.first(where: { $0.variant.label == label }) ?? d.variants.first { return o }
        return seed.map { Offer(variant: $0.variant, prediction: $0.prediction) }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Button { withAnimation(.settle) { store.opened = nil } } label: {
                    HStack(spacing: 8) { Image(systemName: "arrow.left").font(.system(size: 12, weight: .bold)); Text("Back") }
                }
                .buttonStyle(Plain()).keyboardShortcut(.cancelAction)
                hero
                HStack(alignment: .top, spacing: 16) {
                    facts.frame(maxWidth: .infinity)
                    sizes.frame(maxWidth: .infinity)
                }
            }
            .frame(maxWidth: 1080).frame(maxWidth: .infinity)
            .padding(.horizontal, 36).padding(.top, 40).padding(.bottom, 36)
        }
        .scrollIndicators(.never)
        .background(Color.ground)
        .task(id: opened.id) { await load() }
    }

    private func load() async {
        error = nil
        label = seed?.variant.label
        do {
            let d = try await store.detail(opened.id)
            guard !Task.isCancelled else { return }
            if label == nil { label = d.variants.first { $0.variant.label == "Q4_K_M" }?.variant.label ?? d.variants.first?.variant.label }
            withAnimation(.settle) { detail = d }
        } catch {
            guard !Task.isCancelled else { return }
            self.error = error.localizedDescription
        }
    }

    private var hero: some View {
        let p = offer?.prediction
        return HStack(alignment: .top, spacing: 30) {
            VStack(alignment: .leading, spacing: 0) {
                Text(name).font(.bold(34)).tracking(-0.9).lineLimit(2).minimumScaleFactor(0.7)
                Text([detail?.repo ?? seed?.repo, (detail?.license ?? seed?.license).map(Show.license)].compactMap { $0 }.joined(separator: ", "))
                    .font(.sans(13)).opacity(0.72).padding(.top, 5).lineLimit(1).textSelection(.enabled)
                Spacer(minLength: 0)
                if let p, p.tier != .tooBig {
                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        Figure(value: p.tokensPerS, size: 128)
                        VStack(alignment: .leading, spacing: 2) {
                            Text("tokens a second").font(.bold(14))
                            Text("\(p.rough ? "rough, " : "")\(Show.speed(p.tokensPerSLow)) to \(Show.speed(p.tokensPerSHigh))").font(.sans(13)).opacity(0.72)
                        }
                    }
                    .padding(.bottom, -12)
                } else if p != nil {
                    Text("Too big for this Mac").font(.pixel(52))
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            VStack(alignment: .trailing, spacing: 0) {
                HStack(alignment: .top, spacing: 24) {
                    if let p, p.tier != .tooBig { SpeedPreview(tokensPerSecond: p.tokensPerS, code: opened.code, size: 13, limit: 46).frame(width: 250, height: 204) }
                    Creature(name: name, side: 160)
                }
                Spacer(minLength: 0)
                if let o = offer { Get(id: opened.id, repo: detail?.repo ?? seed?.repo ?? "", variant: o.variant, fill: .ink, label: tint, tall: true, enabled: canGet(o)) }
            }
        }
        .foregroundStyle(Color.ink)
        .padding(28).frame(height: 316)
        .background(tint, in: .rect(cornerRadius: 18))
    }

    private func canGet(_ o: Offer) -> Bool { detail?.variants.contains { $0.variant.label == o.variant.label } ?? false }

    @ViewBuilder private var facts: some View {
        VStack(alignment: .leading, spacing: 14) {
            if let o = offer, let i = store.item(repo: detail?.repo ?? seed?.repo ?? "", paths: o.variant.files) {
                if i.status == .ready {
                    HStack(spacing: 8) {
                        Button("Show in Finder") { store.reveal(i.key) }.buttonStyle(Plain())
                        Button("Open in Library") { withAnimation(.settle) { store.go(.library) } }.buttonStyle(Plain())
                    }
                } else { ItemProgress(item: i) }
            }
            if let p = offer?.prediction {
                Text(word(p)).font(.bold(15))
                Blocks(share: p.needBytes / max(p.capacityBytes, 1), count: 30, width: 10, height: 18, color: p.tier == .comfortable ? tint : .alarm)
                row("Memory", "\(Show.memory(p.needBytes)) of \(Show.memory(p.capacityBytes))")
                if p.tier != .tooBig {
                    row("Reads your prompt at", "\(Show.speed(p.promptTokensPerS)) tokens/s")
                    row("In a long conversation", "\(Show.speed(p.tokensPerSDeep)) tokens/s")
                    if p.tokensPerSSustained < p.tokensPerS * 0.95 { row("After 20 s of steady work", "\(Show.speed(p.tokensPerSSustained)) tokens/s") }
                } else {
                    Text("You can still download it for another computer.").font(.sans(13)).foregroundStyle(Color.soft)
                }
            } else if let o = offer, let cap = detail?.capacity {
                Text(o.variant.size < cap * 0.8 ? "Fits in this Mac's model memory" : "Larger than the model memory here").font(.bold(15))
                Text("RunIt predicts speed for language models only.").font(.sans(13)).foregroundStyle(Color.soft)
            }
            if let r = detail?.rating ?? seed?.rating {
                row("LMArena rating", "\(r.rating)")
                Text("From \(Show.count(r.votes)) blind votes, for the full model. Smaller files lose a little.").font(.sans(12.5)).lineSpacing(3).foregroundStyle(Color.soft)
            }
            if let error {
                Text(error).font(.sans(13.5)).foregroundStyle(Color.alarm)
                Button("Try again") { Task { await load() } }.buttonStyle(Plain())
            }
        }
        .padding(20).frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.card, in: .rect(cornerRadius: 14))
    }

    private func word(_ p: Prediction) -> String {
        switch p.tier {
        case .comfortable: "Fits with room to spare"
        case .tight: "Fits, but close heavy apps first"
        case .partial: "\(Int(p.gpuShare * 100))% runs on graphics, the rest on the processor"
        case .tooBig: "Needs more memory than this Mac has"
        }
    }

    private func row(_ k: String, _ v: String) -> some View {
        HStack { Text(k).font(.sans(13.5)).foregroundStyle(Color.soft); Spacer(); Text(v).font(.bold(13.5)) }
    }

    @ViewBuilder private var sizes: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let d = detail {
                if d.variants.isEmpty && d.others.isEmpty {
                    Text(d.note ?? "Hugging Bay does not host files for this one yet.").font(.sans(13.5)).foregroundStyle(Color.soft)
                }
                if !d.variants.isEmpty {
                    Text("Sizes").font(.bold(15))
                    Text("Larger is closer to the original, and slower.").font(.sans(12.5)).foregroundStyle(Color.soft).padding(.top, 3).padding(.bottom, 10)
                    ForEach(d.variants, id: \.variant.files) { o in
                        let on = o.variant.label == offer?.variant.label
                        Button { withAnimation(.settle) { label = o.variant.label } } label: {
                            HStack(spacing: 10) {
                                Rectangle().fill(on ? tint : Color.off).frame(width: 10, height: 10)
                                Text(o.variant.label).font(on ? .bold(13.5) : .sans(13.5))
                                Spacer()
                                Text(Show.size(o.variant.size)).font(.sans(13)).foregroundStyle(Color.soft)
                                SpeedLabel(prediction: o.prediction).frame(width: 106, alignment: .trailing)
                            }
                            .padding(.horizontal, 10).frame(height: 38)
                            .background(on ? Color.raised : .clear, in: .rect(cornerRadius: 8)).contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                    }
                }
                if !d.others.isEmpty { others(d).padding(.top, d.variants.isEmpty ? 0 : 18) }
            } else if error == nil {
                HStack(spacing: 12) {
                    Creature(name: name, side: 20, color: .soft, pace: 0.2)
                    Text("Checking what can be downloaded").font(.sans(13.5)).foregroundStyle(Color.soft)
                }
            }
        }
        .padding(20).frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.card, in: .rect(cornerRadius: 14))
    }

    @ViewBuilder private func others(_ d: Detail) -> some View {
        let open = showOthers || d.variants.isEmpty
        HStack {
            Text(d.variants.isEmpty ? "Files" : "Other files").font(.bold(15))
            Spacer()
            if d.variants.isEmpty {
                Button("Get all, \(Show.size(d.others.reduce(0) { $0 + $1.size }))") { store.download(d.id, d.others) }.buttonStyle(Block(fill: tint))
            } else {
                Button(open ? "Hide" : "Show \(d.others.count)") { withAnimation(.settle) { showOthers.toggle() } }.buttonStyle(Plain())
            }
        }
        if open {
            VStack(spacing: 0) {
                ForEach(d.others, id: \.path) { f in
                    HStack(spacing: 10) {
                        Text(f.path).font(.mono(12)).lineLimit(1).truncationMode(.middle)
                        Spacer()
                        Text(Show.size(f.size)).font(.sans(12)).foregroundStyle(Color.soft)
                        if let i = store.item(repo: d.repo, paths: [f]) {
                            Text(i.status == .ready ? "saved" : "\(Int(i.done / max(i.total, 1) * 100))%").font(.bold(12)).foregroundStyle(tint)
                        } else {
                            Button("Get") { store.download(d.id, [f]) }.buttonStyle(Plain())
                        }
                    }
                    .padding(.vertical, 4)
                }
            }
            .padding(.top, 10)
        }
    }
}

struct ItemProgress: View {
    let item: Item
    @Environment(Store.self) private var store
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Blocks(share: item.done / max(item.total, 1), count: 30, width: 10, height: 18, color: item.status == .failed ? .alarm : store.tint(item.name))
            HStack(spacing: 8) {
                Text(line).font(.sans(12.5)).foregroundStyle(item.status == .failed ? Color.alarm : .soft).lineLimit(2).monospacedDigit()
                Spacer()
                switch item.status {
                case .active, .queued: Button("Pause") { store.act("pause", item.key) }.buttonStyle(Plain())
                case .paused: Button("Resume") { store.act("resume", item.key) }.buttonStyle(Plain())
                case .failed: Button("Try again") { store.act("resume", item.key) }.buttonStyle(Plain())
                case .ready: EmptyView()
                }
                Button(item.status == .failed ? "Remove" : "Cancel") { store.act("remove", item.key) }.buttonStyle(Plain())
            }
        }
    }
    private var line: String {
        switch item.status {
        case .queued: "Waiting"
        case .failed: item.error ?? "Failed"
        case .paused: "Paused at \(Show.size(item.done)) of \(Show.size(item.total))"
        default: item.done >= item.total ? "Checking" : "\(Show.size(item.done)) of \(Show.size(item.total)), \(Show.size(item.bytesPerS))/s  \(Show.left(item.total - item.done, at: item.bytesPerS))"
        }
    }
}
