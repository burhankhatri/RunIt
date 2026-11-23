import RunItText
import SwiftUI

struct Browse: View {
    @Environment(Store.self) private var store
    @FocusState private var typing: Bool

    var body: some View {
        @Bindable var store = store
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Image(systemName: "magnifyingglass").font(.system(size: 15, weight: .bold)).foregroundStyle(Color.orange)
                TextField("Search every model on Hugging Bay", text: $store.query).textFieldStyle(.plain).font(.sans(17)).focused($typing)
                    .onChange(of: store.query) { store.search() }
            }
            .padding(.horizontal, 18).frame(height: 54)
            .background(Color.card, in: .rect(cornerRadius: 12))
            Text(status).font(.sans(12.5)).foregroundStyle(Color.soft).padding(.top, 12).padding(.bottom, 8)
            if let e = store.searchError {
                VStack(alignment: .leading, spacing: 12) {
                    Text(e).font(.sans(14)).foregroundStyle(Color.alarm)
                    Button("Try again") { store.search() }.buttonStyle(Block())
                }
                .padding(.top, 12)
                Spacer()
            } else {
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(store.hits) { h in
                            Hairline()
                            ModelRow(name: h.name, sub: sub(h), rating: h.rating?.rating, prediction: h.prediction) {
                                withAnimation(.settle) { store.opened = Opened(id: h.id, name: h.name) }
                            }
                        }
                        if store.next != nil, !store.hits.isEmpty {
                            Text("Loading more").font(.sans(12)).foregroundStyle(Color.soft).padding(18).onAppear { store.search(reset: false) }
                        }
                    }
                    .padding(.bottom, 30)
                }
                .scrollIndicators(.never)
            }
        }
        .frame(maxWidth: 1180).frame(maxWidth: .infinity)
        .padding(.horizontal, 36).padding(.top, 44)
        .onAppear {
            typing = true
            if store.hits.isEmpty { store.search() }
        }
    }

    private var status: String {
        if store.searching && store.hits.isEmpty { return "Searching" }
        if store.hits.isEmpty { return store.query.isEmpty ? " " : "Nothing found. Try the publisher's name or fewer words." }
        return "\(store.total.formatted()) models"
    }
    private func sub(_ h: Hit) -> String {
        [h.repo.split(separator: "/").first.map(String.init), h.kind, h.label ?? h.formats.first, h.size.map(Show.size), "\(Show.count(h.downloads)) downloads"]
            .compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: ", ")
    }
}

struct Library: View {
    @Environment(Store.self) private var store
    @State private var doomed: Item?
    var body: some View {
        if store.items.isEmpty {
            VStack(spacing: 18) {
                Creature(name: "empty", side: 120, color: .off)
                Text("Nothing here yet").font(.bold(22))
                Text("Models you download live here, with their predicted speed beside the real one.").font(.sans(14)).foregroundStyle(Color.soft).multilineTextAlignment(.center).frame(maxWidth: 340)
                Button("See what this Mac runs") { withAnimation(.settle) { store.go(.forYou) } }.buttonStyle(Block())
            }
        } else {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Library").font(.bold(27)).tracking(-0.6).padding(.bottom, 8)
                    ForEach(store.items) { row($0) }
                }
                .frame(maxWidth: 1080, alignment: .leading).frame(maxWidth: .infinity)
                .padding(.horizontal, 36).padding(.top, 44).padding(.bottom, 36)
            }
            .scrollIndicators(.never)
            .confirmationDialog("Delete \(doomed?.name ?? "this model")?", isPresented: .init(get: { doomed != nil }, set: { if !$0 { doomed = nil } }), presenting: doomed) { i in
                Button("Delete", role: .destructive) { store.act("remove", i.key) }
            } message: { i in Text("This frees \(Show.size(i.total)). You can download it again.") }
        }
    }

    private func row(_ i: Item) -> some View {
        HStack(alignment: .top, spacing: 20) {
            Tile(name: i.name, side: 92)
            VStack(alignment: .leading, spacing: 14) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(i.name).font(.bold(19)).lineLimit(1)
                        Text("\(i.repo), \(i.label), \(Show.size(i.total))").font(.sans(12.5)).foregroundStyle(Color.soft).lineLimit(1)
                    }
                    Spacer()
                    if i.status == .ready {
                        HStack(spacing: 8) {
                            if i.runnable {
                                Button(store.measuring.contains(i.key) ? "Measuring" : i.measured == nil ? "Measure" : "Measure again") { store.measure(i.key) }
                                    .buttonStyle(Plain()).disabled(store.measuring.contains(i.key))
                            }
                            Button("Show in Finder") { store.reveal(i.key) }.buttonStyle(Plain())
                            Button("Delete") { doomed = i }.buttonStyle(Plain())
                            if i.runnable { Button("Try it") { store.openChat(i.key) }.buttonStyle(Block(fill: store.tint(i.name))) }
                        }
                    }
                }
                if i.status != .ready { ItemProgress(item: i) } else { Versus(item: i, busy: store.measuring.contains(i.key)) }
            }
        }
        .padding(18).background(Color.card, in: .rect(cornerRadius: 14))
    }
}

/// Predicted against real.
struct Versus: View {
    let item: Item
    var busy = false
    @Environment(Store.self) private var store
    var body: some View {
        let top = max(item.prediction?.tokensPerS ?? 0, item.measured?.tokensPerS ?? 0, 1)
        VStack(alignment: .leading, spacing: 8) {
            if let p = item.prediction { bar("Predicted", p.tokensPerS, top, .soft) }
            if let m = item.measured { bar("Measured", m.tokensPerS, top, store.tint(item.name)) }
            if busy { Text("Timing it on this Mac").font(.sans(12.5)).foregroundStyle(Color.soft) } else if let note { Text(note).font(.sans(12.5)).foregroundStyle(Color.soft) }
        }
    }
    private var note: String? {
        guard let c = item.measured?.conditions else { return nil }
        if c.hot { return "Measured while this Mac was hot and holding back. Let it cool and measure again." }
        return c.onBattery ? "Measured on battery." : nil
    }
    private func bar(_ k: String, _ v: Double, _ top: Double, _ c: Color) -> some View {
        HStack(spacing: 14) {
            Text(k).font(.sans(13)).foregroundStyle(Color.soft).frame(width: 72, alignment: .leading)
            Blocks(share: v / top, count: 30, color: c)
            Text(Show.speed(v)).font(.pixel(25))
        }
    }
}

struct ThisMac: View {
    @Environment(Store.self) private var store
    var body: some View {
        ScrollView {
            if let r = store.report {
                VStack(alignment: .leading, spacing: 0) {
                    Text("This Mac").font(.bold(27)).tracking(-0.6)
                    Text("\(r.machine.cpu), \(r.machine.os)").font(.sans(14)).foregroundStyle(Color.soft).padding(.top, 6)
                    HStack(spacing: 14) {
                        VStack(alignment: .leading, spacing: 0) {
                            Creature(name: r.machine.cpu, side: 110)
                        }
                        .frame(maxWidth: .infinity, minHeight: 190).background(Color.orange, in: .rect(cornerRadius: 14))
                        stat("\(Int(r.bytesPerS / 1e9))", "GB read a second", Color.tints[1])
                        stat(capacity(r), "GB free for a model", Color.tints[2])
                        stat("\(Int(r.calibration.sustained * 100))%", "speed kept over 20 s", Color.tints[4])
                    }
                    .padding(.top, 22)
                    Text("Writing a token means reading the model's weights once, so the first figure sets every speed in RunIt.")
                        .font(.sans(13.5)).foregroundStyle(Color.soft).padding(.top, 14)

                    Text("What was timed").font(.bold(16)).padding(.top, 34).padding(.bottom, 10)
                    ForEach(r.runs, id: \.self) { x in
                        Hairline()
                        HStack(spacing: 14) {
                            Text("\(Show.size(x.modelBytes)) test model").font(.bold(13.5)).frame(width: 170, alignment: .leading)
                            Text(x.place == "gpu" ? "graphics" : "processor").font(.sans(13.5)).foregroundStyle(Color.soft).frame(width: 90, alignment: .leading)
                            Blocks(share: x.decode / (r.runs.map(\.decode).max() ?? 1), color: x.place == "gpu" ? .orange : Color.tints[1])
                            Spacer()
                            Text("reads \(Show.speed(x.prompt))").font(.sans(13)).foregroundStyle(Color.soft)
                            HStack(alignment: .firstTextBaseline, spacing: 5) { Text(Show.speed(x.decode)).font(.pixel(25)); Text("tok/s").font(.sans(11)).foregroundStyle(Color.soft) }.frame(width: 100, alignment: .trailing)
                        }
                        .frame(height: 46)
                    }
                    HStack(spacing: 10) {
                        Button("Test again") { withAnimation(.settle) { store.test() } }.buttonStyle(Block())
                        Button("Choose models folder") { choose() }.buttonStyle(Plain())
                        Text(store.modelsDir).font(.mono(12)).foregroundStyle(Color.soft).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                    }
                    .padding(.top, 24)
                    Text("Tested \(Date(timeIntervalSince1970: r.takenAt).formatted(date: .abbreviated, time: .shortened)), in \(Int(r.seconds)) s. Models and files: Hugging Bay. Ratings: LMArena leaderboard dataset, CC BY 4.0, \(store.ratingsDate). Engine: llama.cpp. RunIt \(store.version) sends nothing about this computer anywhere.")
                        .font(.sans(12.5)).lineSpacing(3).foregroundStyle(Color.soft).padding(.top, 34).frame(maxWidth: 620, alignment: .leading)
                }
                .frame(maxWidth: 1080, alignment: .leading).frame(maxWidth: .infinity)
                .padding(.horizontal, 36).padding(.top, 44).padding(.bottom, 36)
            }
        }
        .scrollIndicators(.never)
    }
    private func capacity(_ r: Report) -> String {
        let ram = r.machine.ramBytes, room = ram - max(ram / 4, 4 * 1_073_741_824)
        let c = r.machine.gpu.map { $0.integrated ? min($0.totalMib * 1_048_576, room) : $0.freeMib * 1_048_576 } ?? room
        return String(format: "%.1f", c / 1_073_741_824)
    }
    private func stat(_ v: String, _ k: String, _ c: Color) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Spacer()
            Text(v).font(.pixel(64)).lineLimit(1).minimumScaleFactor(0.6)
            Text(k).font(.bold(13)).lineLimit(2)
        }
        .foregroundStyle(Color.ink).padding(18)
        .frame(maxWidth: .infinity, minHeight: 190, alignment: .leading).background(c, in: .rect(cornerRadius: 14))
    }
    private func choose() {
        let p = NSOpenPanel()
        p.canChooseDirectories = true
        p.canChooseFiles = false
        p.canCreateDirectories = true
        p.prompt = "Use this folder"
        if p.runModal() == .OK, let u = p.url { store.moveModels(to: u) }
    }
}

struct ChatScreen: View {
    @Environment(Store.self) private var store
    @State private var draft = ""
    @FocusState private var typing: Bool
    private var item: Item? { store.items.first { $0.key == store.chatKey } }
    private var name: String { item?.name ?? "" }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 16) {
                RoundedRectangle(cornerRadius: 12).fill(store.tint(name))
                    .overlay(Creature(name: name, side: 40, pace: store.chatBusy ? 0.14 : 0.46)).frame(width: 58, height: 58)
                VStack(alignment: .leading, spacing: 3) {
                    Text(name).font(.bold(19))
                    Text("Running on this Mac. Nothing leaves it.").font(.sans(12.5)).foregroundStyle(Color.soft)
                }
                Spacer()
                if store.liveSpeed > 0 {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text(Show.speed(store.liveSpeed)).font(.pixel(48)).foregroundStyle(store.tint(name))
                        VStack(alignment: .leading, spacing: 1) {
                            Text("tokens/s now").font(.bold(12))
                            if let p = item?.prediction { Text("\(Show.speed(p.tokensPerS)) predicted").font(.sans(12)).foregroundStyle(Color.soft) }
                        }
                    }
                }
                Button("Close") { withAnimation(.settle) { store.closeChat() } }.buttonStyle(Plain()).keyboardShortcut(.cancelAction).padding(.leading, 8)
            }
            .padding(.horizontal, 36).padding(.top, 40).padding(.bottom, 16)
            Hairline()
            ScrollView {
                VStack(alignment: .leading, spacing: 22) {
                    if store.chatLoading {
                        HStack(spacing: 14) { Creature(name: name, side: 30, color: store.tint(name), pace: 0.16); Typed(text: "Loading it into memory", font: .mono(15), color: .soft, perSecond: 30) }
                    } else if store.turns.isEmpty && store.chatError == nil {
                        Text("Ask it anything.").font(.bold(22)).foregroundStyle(Color.soft)
                    }
                    ForEach(store.turns) { t in
                        if t.mine {
                            Text(t.text).font(.bold(15)).textSelection(.enabled).padding(.horizontal, 14).padding(.vertical, 10)
                                .background(Color.card, in: .rect(cornerRadius: 10)).frame(maxWidth: .infinity, alignment: .trailing)
                        } else {
                            VStack(alignment: .leading, spacing: 10) {
                                if !t.thought.isEmpty {
                                    DisclosureGroup("Thinking") { Text(t.thought).font(.sans(13)).foregroundStyle(Color.soft).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }.font(.sans(12.5)).foregroundStyle(Color.soft)
                                }
                                Text(reply(t)).font(.sans(15.5)).lineSpacing(6).textSelection(.enabled)
                            }
                        }
                    }
                    if let n = store.chatNote, !store.chatBusy { Text(n).font(.sans(12.5)).foregroundStyle(Color.soft) }
                    if let e = store.chatError { Text(e).font(.sans(14)).foregroundStyle(Color.alarm) }
                }
                .frame(maxWidth: 700, alignment: .leading).frame(maxWidth: .infinity)
                .padding(.horizontal, 36).padding(.vertical, 26)
            }
            .defaultScrollAnchor(.bottom)
            HStack(spacing: 10) {
                TextField("Message", text: $draft, axis: .vertical).textFieldStyle(.plain).font(.sans(15)).lineLimit(1...6).focused($typing).onSubmit(send)
                if store.chatBusy { Button("Stop") { store.stopReply() }.buttonStyle(Plain()) } else {
                    Button("Send", action: send).buttonStyle(Block(fill: store.tint(name))).disabled(store.chatLoading || draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
            .padding(.leading, 18).padding(.trailing, 10).padding(.vertical, 9).frame(minHeight: 54)
            .background(Color.card, in: .rect(cornerRadius: 12))
            .frame(maxWidth: 736).padding(.horizontal, 36).padding(.bottom, 26)
        }
        .onAppear { typing = true }
    }

    private func send() {
        let t = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty, !store.chatBusy, !store.chatLoading else { return }
        draft = ""
        store.send(t)
    }

    private func reply(_ t: Turn) -> AttributedString {
        var a = (try? AttributedString(markdown: t.text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(t.text)
        if store.chatBusy, t.id == store.turns.last?.id {
            var c = AttributedString(" ")
            c.backgroundColor = store.tint(name)
            a += c
        }
        return a
    }
}
