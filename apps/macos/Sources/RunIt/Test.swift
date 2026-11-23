import RunItText
import SwiftUI

struct TestScreen: View {
    @Environment(Store.self) private var store
    var body: some View {
        Group {
            if store.testing || !store.lines.isEmpty { Running() } else { Opening() }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.horizontal, 70).padding(.vertical, 40)
    }
}

private struct Opening: View {
    @Environment(Store.self) private var store
    @State private var asked = false
    var body: some View {
        HStack(spacing: 70) {
            RoundedRectangle(cornerRadius: 28).fill(Color.orange)
                .overlay(Creature(name: "RunIt", side: 240))
                .frame(width: 340, height: 340)
            VStack(alignment: .leading, spacing: 24) {
                Typed(text: "What can\nthis Mac run?", font: .bold(52), perSecond: 20, delay: 0.3) { withAnimation(.settle) { asked = true } }
                    .tracking(-1.6)
                VStack(alignment: .leading, spacing: 24) {
                    Text("RunIt builds small test models on your disk and times the real engine on them, so the speeds belong to this computer. It takes a minute or two. Nothing is uploaded, and the test models are deleted.")
                        .font(.sans(15)).lineSpacing(5).foregroundStyle(Color.soft).frame(maxWidth: 440, alignment: .leading)
                    if let e = store.testError { Text(e).font(.sans(14)).foregroundStyle(Color.alarm).frame(maxWidth: 440, alignment: .leading) }
                    HStack(spacing: 12) {
                        Button("Test this Mac") { withAnimation(.settle) { store.test() } }.buttonStyle(Block(tall: true)).keyboardShortcut(.defaultAction)
                        if store.report != nil { Button("Keep the last result") { store.entered = true }.buttonStyle(Plain()) }
                    }
                }
                .opacity(asked ? 1 : 0).offset(y: asked ? 0 : 10)
            }
            .frame(width: 470, alignment: .leading)
        }
    }
}

/// The machine's creature goes up block by block while the findings type out.
private struct Running: View {
    @Environment(Store.self) private var store
    private var over: Bool { !store.testing && store.testError == nil && store.report != nil }

    var body: some View {
        HStack(spacing: 64) {
            VStack(spacing: 26) {
                if over {
                    RoundedRectangle(cornerRadius: 28).fill(Color.orange)
                        .overlay(Creature(name: name, side: 240, pace: 0.3)).frame(width: 340, height: 340)
                        .transition(.scale(scale: 0.92).combined(with: .opacity))
                } else {
                    Building(name: name, progress: store.progress, side: 340)
                }
                HStack(alignment: .firstTextBaseline) {
                    Text("\(Int(store.progress * 100))%").font(.pixel(44)).foregroundStyle(Color.orange).contentTransition(.numericText())
                    Spacer()
                    Text(over ? "Done" : store.stage).font(.sans(13)).foregroundStyle(Color.soft).multilineTextAlignment(.trailing).frame(maxWidth: 190, alignment: .trailing)
                }
                .frame(width: 340)
            }
            .animation(.settle, value: over)
            VStack(alignment: .leading, spacing: 0) {
                VStack(alignment: .leading, spacing: 15) {
                    ForEach(store.lines) { l in
                        let last = l.id == store.lines.last?.id
                        HStack(alignment: .firstTextBaseline, spacing: 12) {
                            Rectangle().fill(last && !over ? Color.orange : Color.off).frame(width: 9, height: 9).offset(y: -2)
                            Typed(text: l.text, font: .mono(15.5), color: last || over ? .text : .soft, perSecond: 70)
                                .lineSpacing(4).fixedSize(horizontal: false, vertical: true)
                        }
                        .transition(.opacity)
                    }
                }
                .animation(.settle, value: store.lines)
                footer.padding(.top, 34)
            }
            .frame(width: 480, alignment: .leading)
        }
    }

    private var name: String { store.report?.machine.cpu ?? "this Mac" }

    @ViewBuilder private var footer: some View {
        if let e = store.testError {
            VStack(alignment: .leading, spacing: 14) {
                Text(e).font(.sans(14)).foregroundStyle(Color.alarm)
                Button("Try again") { store.test() }.buttonStyle(Block())
            }
        } else if over {
            VStack(alignment: .leading, spacing: 14) {
                Button(store.shortlist == nil && store.shortlistError == nil ? waiting : "See what it runs") { store.entered = true }
                    .buttonStyle(Block(tall: true)).disabled(store.shortlist == nil && store.shortlistError == nil)
                    .keyboardShortcut(.defaultAction)
                if let e = store.shortlistError {
                    Text(e).font(.sans(13)).foregroundStyle(Color.alarm)
                    Button("Try again") { Task { await store.recommend() } }.buttonStyle(Plain())
                }
            }
        } else {
            Button("Stop") { store.stopTest() }.buttonStyle(Plain())
        }
    }

    private var waiting: String {
        if let p = store.catalogProgress, p < 1 { return "Reading the catalog, \(Int(p * 100))%" }
        return "Matching models"
    }
}
