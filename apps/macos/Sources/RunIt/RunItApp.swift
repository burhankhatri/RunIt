import RunItText
import SwiftUI

@main
struct RunItApp: App {
    @State private var store = Store.shared
    init() { Fonts.register() }
    var body: some Scene {
        Window("RunIt", id: "main") {
            Root().environment(store).frame(minWidth: 1060, minHeight: 700).background(Color.ground)
                .preferredColorScheme(.dark)
                .onAppear {
                    NSApp.appearance = NSAppearance(named: .darkAqua)
                    store.start()
                    Rehearsal.begin(store)
                }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1200, height: 780)
        .commands {
            CommandGroup(replacing: .newItem) {}
            CommandMenu("Go") {
                Button("For you") { go(.forYou) }.keyboardShortcut("1")
                Button("Browse") { go(.browse) }.keyboardShortcut("2")
                Button("Library") { go(.library) }.keyboardShortcut("3")
                Button("This Mac") { go(.machine) }.keyboardShortcut("4")
            }
        }
    }
    private func go(_ p: Place) { if store.entered { withAnimation(.settle) { store.go(p) } } }
}

struct Root: View {
    @Environment(Store.self) private var store
    @Environment(\.controlActiveState) private var active
    var body: some View {
        ZStack {
            Color.ground.ignoresSafeArea()
            if let f = store.fatal {
                VStack(alignment: .leading, spacing: 16) {
                    Creature(name: "lost", side: 100, color: .alarm)
                    Text("Something is missing").font(.bold(28))
                    Text(f).font(.sans(15)).foregroundStyle(Color.soft)
                }
                .frame(maxWidth: 520, alignment: .leading)
            } else if store.ready {
                if store.entered { MainScreen().transition(.opacity) } else { TestScreen().transition(.opacity) }
            }
        }
        .foregroundStyle(Color.text)
        .animation(.easeInOut(duration: 0.3), value: store.entered)
        .overlay(alignment: .bottom) { Notice() }
        .environment(\.resting, active == .inactive && Rehearsal.folder == nil)
    }
}

struct Notice: View {
    @Environment(Store.self) private var store
    var body: some View {
        ZStack {
            if let n = store.notice {
                HStack(spacing: 16) {
                    Text(n).font(.sans(13)).foregroundStyle(Color.ink).lineLimit(3)
                    Button("Dismiss") { store.notice = nil }.buttonStyle(.plain).font(.bold(13)).foregroundStyle(Color.ink)
                }
                .padding(.horizontal, 16).padding(.vertical, 12)
                .background(Color.text, in: .rect(cornerRadius: 10)).frame(maxWidth: 620).padding(.bottom, 22)
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .task(id: n) {
                    try? await Task.sleep(for: .seconds(8))
                    if !Task.isCancelled { store.notice = nil }
                }
            }
        }
        .animation(.settle, value: store.notice)
    }
}

struct MainScreen: View {
    @Environment(Store.self) private var store
    @Environment(\.resting) private var resting
    private var covered: Bool { store.opened != nil || store.chatKey != nil }
    var body: some View {
        HStack(spacing: 0) {
            Sidebar()
            Rectangle().fill(Color.line).frame(width: 1).ignoresSafeArea()
            ZStack {
                // The list stays put underneath, so coming back costs nothing and keeps its place.
                Group {
                    switch store.place {
                    case .forYou: ForYou()
                    case .browse: Browse()
                    case .library: Library()
                    case .machine: ThisMac()
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .transition(.opacity)
                .environment(\.resting, resting || covered)
                .allowsHitTesting(!covered)
                .accessibilityHidden(covered)
                if let o = store.opened {
                    ModelPage(opened: o).id(o.id).transition(.move(edge: .bottom).combined(with: .opacity))
                }
                if store.chatKey != nil {
                    ChatScreen().background(Color.ground).transition(.opacity)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .clipped()
        }
    }
}

struct Sidebar: View {
    @Environment(Store.self) private var store
    private var busy: Int { store.items.filter { $0.status == .active || $0.status == .queued }.count }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 10) {
                RoundedRectangle(cornerRadius: 7).fill(Color.orange).overlay(Creature(name: "RunIt", side: 20)).frame(width: 30, height: 30)
                Text("RunIt").font(.pixel(27))
            }
            .padding(.leading, 8).padding(.bottom, 20)
            row("For you", .forYou)
            row("Browse", .browse)
            row("Library", .library, badge: busy)
            row("This Mac", .machine)
            Spacer()
            if let m = store.report?.machine {
                HStack(spacing: 10) {
                    Creature(name: m.cpu, side: 30, color: .orange)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(m.cpu).font(.bold(12)).lineLimit(1)
                        Text("\(Int((m.ramBytes / 1_073_741_824).rounded())) GB memory").font(.sans(12)).foregroundStyle(Color.soft)
                    }
                }
                .padding(.leading, 8)
            }
        }
        .padding(.top, 52).padding(.bottom, 20).padding(.horizontal, 12)
        .frame(width: 198, alignment: .leading)
        .ignoresSafeArea(edges: .top)
    }

    private func row(_ title: String, _ p: Place, badge: Int = 0) -> some View {
        let on = store.place == p && store.chatKey == nil
        return Button { withAnimation(.settle) { store.go(p) } } label: {
            HStack {
                Text(title).font(on ? .bold(14) : .sans(14)).foregroundStyle(on ? Color.text : .soft)
                Spacer()
                if badge > 0 { Text("\(badge)").font(.pixel(16)).foregroundStyle(Color.orange) }
            }
            .padding(.horizontal, 10).frame(height: 35)
            .background(on ? Color.raised : .clear, in: .rect(cornerRadius: 8))
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}
