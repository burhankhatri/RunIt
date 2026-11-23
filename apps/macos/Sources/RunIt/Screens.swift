import RunItText
import SwiftUI

struct Browse: View {
    @Environment(Store.self) private var store
    @FocusState private var typing: Bool

    var body: some View {
        @Bindable var store = store
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 0) {
                TextField("Search every model on Hugging Bay", text: $store.query)
                    .textFieldStyle(.plain).font(.display(30)).focused($typing)
                    .onChange(of: store.query) { store.search() }
                HStack {
                    Text(status).font(.sans(12.5)).foregroundStyle(Color.faint).contentTransition(.numericText())
                    Spacer()
                }
                .padding(.top, 8).padding(.bottom, 14)
                Hairline()
                if let e = store.searchError {
                    VStack(alignment: .leading, spacing: 12) {
                        Text(e).font(.sans(14)).foregroundStyle(Color.alarm)
                        Button("Try again") { store.search() }.buttonStyle(Outline())
                    }
                    .padding(.top, 24)
                    Spacer()
                } else {
                    ScrollView {
                        LazyVStack(spacing: 0) {
                            ForEach(store.hits) { h in
                                ModelRow(name: h.name, sub: sub(h), rating: h.rating?.rating, prediction: h.prediction, on: store.browsing?.id == h.id) {
                                    withAnimation(.settle) { store.browsing = h }
                                }
                                Hairline()
                            }
                            if store.next != nil, !store.hits.isEmpty {
                                Text("Loading more").font(.sans(12)).foregroundStyle(Color.faint).padding(18).onAppear { store.search(reset: false) }
                            }
                        }
                        .padding(.bottom, 30)
                    }
                    .scrollIndicators(.never)
                }
            }
            .padding(.horizontal, 40).padding(.top, 50)
            .frame(maxWidth: .infinity, alignment: .leading)
            if let h = store.browsing {
                Rectangle().fill(Color.rule).frame(width: 1).ignoresSafeArea()
                DetailPane(id: h.id).frame(width: 400).transition(.move(edge: .trailing).combined(with: .opacity))
            }
        }
        .onAppear {
            typing = true
            if store.hits.isEmpty { store.search() }
        }
    }

    private var status: String {
        if store.searching && store.hits.isEmpty { return "Searching" }
        if store.hits.isEmpty { return store.query.isEmpty ? "" : "Nothing found. Try the publisher's name or fewer words." }
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
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text("Library").font(.display(46)).padding(.bottom, 24)
                if store.items.isEmpty {
                    Text("Models you download are kept here, with their predicted speed beside the real one.").font(.sans(15)).foregroundStyle(Color.faint).frame(maxWidth: 440, alignment: .leading)
                    Button("See what this Mac runs") { withAnimation(.settle) { store.place = .forYou } }.buttonStyle(Solid()).padding(.top, 20)
                } else {
                    Hairline()
                    ForEach(store.items) { i in
                        row(i).padding(.vertical, 20)
                        Hairline()
                    }
                }
            }
            .frame(maxWidth: 820, alignment: .leading).frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 40).padding(.top, 50).padding(.bottom, 40)
        }
        .scrollIndicators(.never)
        .confirmationDialog("Delete \(doomed?.name ?? "this model")?", isPresented: .init(get: { doomed != nil }, set: { if !$0 { doomed = nil } }), presenting: doomed) { i in
            Button("Delete", role: .destructive) { store.act("remove", i.key) }
        } message: { i in Text("This frees \(Show.size(i.total)). You can download it again.") }
    }

    private func row(_ i: Item) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 3) {
                    Text(i.name).font(.display(26))
                    Text("\(i.repo), \(i.label), \(Show.size(i.total))").font(.sans(12.5)).foregroundStyle(Color.faint).lineLimit(1)
                }
                Spacer()
                if i.status == .ready {
                    HStack(spacing: 8) {
                        if i.runnable {
                            Button("Try it") { store.openChat(i.key) }.buttonStyle(Outline())
                            Button(store.measuring.contains(i.key) ? "Measuring" : i.measured == nil ? "Measure real speed" : "Measure again") { store.measure(i.key) }
                                .buttonStyle(Outline()).disabled(store.measuring.contains(i.key))
                        }
                        Button("Show in Finder") { store.reveal(i.key) }.buttonStyle(Outline())
                        Button("Delete") { doomed = i }.buttonStyle(Outline())
                    }
                }
            }
            if i.status != .ready { ItemProgress(item: i) }
            if i.status == .ready, i.prediction != nil || i.measured != nil { Versus(predicted: i.prediction?.tokensPerS, measured: i.measured, busy: store.measuring.contains(i.key)) }
        }
    }
}

/// Predicted against real.
struct Versus: View {
    let predicted: Double?
    let measured: Measured?
    var busy = false
    var body: some View {
        let top = max(predicted ?? 0, measured?.tokensPerS ?? 0, 1)
        VStack(alignment: .leading, spacing: 7) {
            if let predicted { bar("Predicted", predicted, top, .rule) }
            if let measured { bar("Measured", measured.tokensPerS, top, .marker) }
            if let note, !busy { Text(note).font(.sans(12.5)).foregroundStyle(Color.faint) }
            if busy { Text("Timing it on this Mac").font(.sans(12.5)).foregroundStyle(Color.faint) }
        }
        .frame(maxWidth: 520, alignment: .leading)
        .animation(.settle, value: measured)
    }
    private var note: String? {
        guard let c = measured?.conditions else { return nil }
        if c.hot { return "Measured while this Mac was hot and holding back. Let it cool and measure again." }
        return c.onBattery ? "Measured on battery." : nil
    }
    private func bar(_ k: String, _ v: Double, _ top: Double, _ c: Color) -> some View {
        HStack(spacing: 12) {
            Text(k).font(.sans(12.5)).foregroundStyle(Color.faint).frame(width: 72, alignment: .leading)
            GeometryReader { g in RoundedRectangle(cornerRadius: 2).fill(c).frame(width: max(2, g.size.width * v / top)) }.frame(height: 14)
            Text("\(Show.speed(v)) tok/s").font(.mono(12.5)).frame(width: 92, alignment: .trailing)
        }
    }
}

struct ThisMac: View {
    @Environment(Store.self) private var store
    var body: some View {
        ScrollView {
            if let r = store.report {
                VStack(alignment: .leading, spacing: 0) {
                    Typed(text: "\(r.machine.cpu) reads \(Int(r.bytesPerS / 1e9)) GB of model a second.", font: .display(46), perSecond: 60, caret: false)
                        .fixedSize(horizontal: false, vertical: true)
                    Text("Writing a token means reading the model's weights once, so that figure sets every speed on these screens.")
                        .font(.sans(15)).foregroundStyle(Color.faint).padding(.top, 12).frame(maxWidth: 520, alignment: .leading)

                    facts([
                        ("Processor", "\(r.machine.cpu), \(r.machine.cores) cores"),
                        ("Graphics", r.machine.gpu?.name ?? "none the engine can use"),
                        ("Memory", Show.memory(r.machine.ramBytes)),
                        ("Speed kept after 20 s of work", "\(Int(r.calibration.sustained * 100))%"),
                        ("System", r.machine.os),
                        ("Tested", Date(timeIntervalSince1970: r.takenAt).formatted(date: .abbreviated, time: .shortened) + ", \(Int(r.seconds)) s"),
                    ]).padding(.top, 34)

                    Text("What was timed").font(.display(26)).padding(.top, 44).padding(.bottom, 12)
                    Hairline()
                    ForEach(r.runs, id: \.self) { x in
                        HStack {
                            Text("\(Show.size(x.modelBytes)) test model").font(.sans(13.5))
                            Text(x.place == "gpu" ? "graphics" : "processor").font(.sans(13.5)).foregroundStyle(Color.faint)
                            Spacer()
                            Text("\(Show.speed(x.decode)) tok/s").font(.mono(13)).frame(width: 110, alignment: .trailing)
                            Text("reads \(Show.speed(x.prompt))").font(.mono(13)).foregroundStyle(Color.faint).frame(width: 120, alignment: .trailing)
                        }
                        .padding(.vertical, 9)
                        Hairline()
                    }

                    HStack(spacing: 10) {
                        Button("Test again") { store.lines = []; store.test() }.buttonStyle(Solid())
                        Button("Choose models folder") { choose() }.buttonStyle(Outline())
                    }
                    .padding(.top, 30)
                    Text(store.modelsDir).font(.mono(12)).foregroundStyle(Color.faint).padding(.top, 10).textSelection(.enabled)

                    Text("Models and files: Hugging Bay. Ratings: LMArena leaderboard dataset, CC BY 4.0, \(store.ratingsDate). Engine: llama.cpp. RunIt \(store.version) sends nothing about this computer anywhere.")
                        .font(.sans(12.5)).lineSpacing(3).foregroundStyle(Color.faint).padding(.top, 40).frame(maxWidth: 560, alignment: .leading)
                }
                .frame(maxWidth: 760, alignment: .leading).frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 40).padding(.top, 50).padding(.bottom, 40)
            }
        }
        .scrollIndicators(.never)
    }
    private func facts(_ f: [(String, String)]) -> some View {
        VStack(spacing: 0) {
            Hairline()
            ForEach(f, id: \.0) { k, v in
                HStack { Text(k).font(.sans(13.5)).foregroundStyle(Color.faint); Spacer(); Text(v).font(.sans(13.5)) }.padding(.vertical, 9)
                Hairline()
            }
        }
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

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 3) {
                    Text(item?.name ?? "").font(.display(26))
                    Text("Running on this Mac. Nothing leaves it.").font(.sans(12.5)).foregroundStyle(Color.faint)
                }
                Spacer()
                if store.liveSpeed > 0 {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Text(Show.speed(store.liveSpeed)).font(.display(36)).contentTransition(.numericText(value: store.liveSpeed)).animation(.smooth, value: store.liveSpeed)
                        VStack(alignment: .leading, spacing: 0) {
                            Text("tokens/s now").font(.sans(11.5))
                            if let p = item?.prediction { Text("\(Show.speed(p.tokensPerS)) predicted").font(.sans(11.5)).foregroundStyle(Color.faint) }
                        }
                    }
                }
                Button("Close") { store.closeChat() }.buttonStyle(Outline()).keyboardShortcut(.cancelAction).padding(.leading, 12)
            }
            .padding(.horizontal, 40).padding(.top, 50).padding(.bottom, 16)
            Hairline()
            ScrollView {
                VStack(alignment: .leading, spacing: 22) {
                    if store.chatLoading {
                        Typed(text: "Loading it into memory.", font: .display(26), color: .faint, perSecond: 30)
                    } else if store.turns.isEmpty && store.chatError == nil {
                        Text("Ask it anything.").font(.display(26)).foregroundStyle(Color.faint)
                    }
                    ForEach(store.turns) { t in
                        if t.mine {
                            Text(t.text).font(.sansBold(15)).textSelection(.enabled)
                        } else {
                            VStack(alignment: .leading, spacing: 10) {
                                if !t.thought.isEmpty {
                                    DisclosureGroup("Thinking") { Text(t.thought).font(.sans(13)).foregroundStyle(Color.faint).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }.font(.sans(12.5)).foregroundStyle(Color.faint)
                                }
                                Text(reply(t)).font(.reading(17)).lineSpacing(5).textSelection(.enabled)
                            }
                        }
                    }
                    if let n = store.chatNote, !store.chatBusy { Text(n).font(.sans(12.5)).foregroundStyle(Color.faint) }
                    if let e = store.chatError { Text(e).font(.sans(14)).foregroundStyle(Color.alarm) }
                }
                .frame(maxWidth: 660, alignment: .leading).frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 40).padding(.vertical, 26)
            }
            .defaultScrollAnchor(.bottom)
            HStack(spacing: 10) {
                TextField("Message", text: $draft, axis: .vertical).textFieldStyle(.plain).font(.sans(15)).lineLimit(1...6).focused($typing)
                    .onSubmit(send)
                if store.chatBusy { Button("Stop") { store.stopReply() }.buttonStyle(Outline()) } else {
                    Button("Send", action: send).buttonStyle(Solid()).disabled(store.chatLoading || draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
            .padding(.leading, 18).padding(.trailing, 8).padding(.vertical, 8).frame(minHeight: 56)
            .background(Color.sheet, in: .rect(cornerRadius: 8)).overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Color.rule))
            .padding(.horizontal, 40).padding(.bottom, 26)
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
            c.backgroundColor = .marker
            a += c
        }
        return a
    }
}
