import RunItText
import SwiftUI

struct TestScreen: View {
    @Environment(Store.self) private var store
    @State private var asked = false

    var body: some View {
        Group {
            if store.testing || !store.lines.isEmpty { Running() } else { opening }
        }
        .frame(maxWidth: 820, maxHeight: .infinity, alignment: .leading)
        .padding(.horizontal, 60)
    }

    private var opening: some View {
        VStack(alignment: .leading, spacing: 0) {
            Spacer()
            Typed(text: "What can this Mac run?", font: .display(68), perSecond: 22, delay: 0.35) { withAnimation(.easeOut(duration: 0.5)) { asked = true } }
            VStack(alignment: .leading, spacing: 26) {
                Text("RunIt builds test models on your disk and times the real engine on them, so the speeds it gives you belong to this computer. It takes a minute or two. Nothing is uploaded, and the test models are deleted.")
                    .font(.sans(16)).lineSpacing(5).foregroundStyle(Color.faint).frame(maxWidth: 560, alignment: .leading)
                if let e = store.testError {
                    Text(e).font(.sans(14)).foregroundStyle(Color.alarm).frame(maxWidth: 560, alignment: .leading)
                }
                HStack(spacing: 14) {
                    Button("Test this Mac") { withAnimation(.easeInOut(duration: 0.35)) { store.test() } }.buttonStyle(Solid()).keyboardShortcut(.defaultAction)
                    if store.report != nil { Button("Keep the last result") { store.entered = true }.buttonStyle(Outline()) }
                }
            }
            .padding(.top, 26).opacity(asked ? 1 : 0).offset(y: asked ? 0 : 8)
            Spacer()
            Spacer()
        }
    }
}

/// The lab report, writing itself.
private struct Running: View {
    @Environment(Store.self) private var store
    @State private var verdict = false
    private var over: Bool { !store.testing && store.testError == nil && store.report != nil }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Spacer(minLength: 60)
            VStack(alignment: .leading, spacing: 13) {
                ForEach(store.lines) { l in
                    let last = l.id == store.lines.last?.id
                    Typed(text: l.text, font: .display(29), color: last || over ? .ink : .faint, perSecond: 62)
                        .fixedSize(horizontal: false, vertical: true)
                        .transition(.opacity)
                }
            }
            .animation(.settle, value: store.lines)
            Spacer(minLength: 30)
            footer.frame(height: 92, alignment: .top)
        }
        .padding(.bottom, 34)
    }

    @ViewBuilder private var footer: some View {
        if let e = store.testError {
            VStack(alignment: .leading, spacing: 14) {
                Text(e).font(.sans(14)).foregroundStyle(Color.alarm)
                Button("Try again") { store.test() }.buttonStyle(Outline())
            }
        } else if over {
            HStack(spacing: 18) {
                Button(store.shortlist == nil ? waiting : "See what it runs") { store.entered = true }
                    .buttonStyle(Solid()).disabled(store.shortlist == nil && store.shortlistError == nil)
                    .keyboardShortcut(.defaultAction)
                if let e = store.shortlistError {
                    Text(e).font(.sans(13)).foregroundStyle(Color.alarm)
                    Button("Try again") { Task { await store.recommend() } }.buttonStyle(Outline())
                }
            }
        } else {
            VStack(alignment: .leading, spacing: 12) {
                GeometryReader { g in
                    Rectangle().fill(Color.rule)
                    Rectangle().fill(Color.ink).frame(width: g.size.width * store.progress).animation(.smooth(duration: 1.2), value: store.progress)
                }
                .frame(height: 2)
                HStack {
                    Text(store.stage).font(.mono(12)).foregroundStyle(Color.faint).contentTransition(.opacity).animation(.easeInOut, value: store.stage)
                    Spacer()
                    Text("\(Int(store.progress * 100))%").font(.mono(12)).foregroundStyle(Color.faint).contentTransition(.numericText())
                    Button("Stop") { store.stopTest() }.buttonStyle(Outline())
                }
            }
        }
    }

    private var waiting: String {
        if let p = store.catalogProgress, p < 1 { return "Reading the catalog, \(Int(p * 100))%" }
        return "Matching models"
    }
}
