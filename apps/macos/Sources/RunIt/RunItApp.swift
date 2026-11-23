import SwiftUI

@main
struct RunItApp: App {
    @State private var store = Store.shared
    init() { Fonts.register() }
    var body: some Scene {
        Window("RunIt", id: "main") {
            Root().environment(store).frame(minWidth: 1060, minHeight: 700).background(Color.paper)
                .onAppear {
                    store.start()
                    Rehearsal.begin(store)
                }
                .onReceive(NotificationCenter.default.publisher(for: NSApplication.willTerminateNotification)) { _ in store.core?.close() }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1220, height: 800)
        .commands {
            CommandGroup(replacing: .newItem) {}
            CommandMenu("Go") {
                Button("For you") { store.place = .forYou }.keyboardShortcut("1")
                Button("Browse") { store.place = .browse }.keyboardShortcut("2")
                Button("Library") { store.place = .library }.keyboardShortcut("3")
                Button("This Mac") { store.place = .machine }.keyboardShortcut("4")
            }
        }
    }
}

struct Root: View {
    @Environment(Store.self) private var store
    var body: some View {
        ZStack {
            Color.paper.ignoresSafeArea()
            if let f = store.fatal {
                VStack(alignment: .leading, spacing: 14) {
                    Text("Something is missing.").font(.display(44))
                    Text(f).font(.sans(15)).foregroundStyle(Color.faint)
                }.frame(maxWidth: 560, alignment: .leading)
            } else if !store.ready {
                EmptyView()
            } else if !store.entered {
                TestScreen().transition(.opacity)
            } else {
                MainScreen().transition(.opacity)
            }
        }
        .foregroundStyle(Color.ink)
        .animation(.easeInOut(duration: 0.4), value: store.entered)
        .overlay(alignment: .bottom) { Notice() }
    }
}

struct Notice: View {
    @Environment(Store.self) private var store
    var body: some View {
        if let n = store.notice {
            HStack(spacing: 14) {
                Text(n).font(.sans(13)).foregroundStyle(Color.paper).lineLimit(3)
                Button("Dismiss") { store.notice = nil }.buttonStyle(.plain).font(.sansBold(13)).foregroundStyle(Color.marker)
            }
            .padding(.horizontal, 16).padding(.vertical, 11)
            .background(Color.ink, in: .rect(cornerRadius: 8)).frame(maxWidth: 620).padding(.bottom, 22)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .task(id: n) {
                try? await Task.sleep(for: .seconds(8))
                if !Task.isCancelled { withAnimation(.settle) { store.notice = nil } }
            }
        }
    }
}

struct MainScreen: View {
    @Environment(Store.self) private var store
    var body: some View {
        HStack(spacing: 0) {
            Sidebar()
            Rectangle().fill(Color.rule).frame(width: 1).ignoresSafeArea()
            Group {
                if store.chatKey != nil {
                    ChatScreen()
                } else {
                    switch store.place {
                    case .forYou: ForYou()
                    case .browse: Browse()
                    case .library: Library()
                    case .machine: ThisMac()
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

struct Sidebar: View {
    @Environment(Store.self) private var store
    @Namespace private var mark
    private var busy: Int { store.items.filter { $0.status == .active || $0.status == .queued }.count }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text("RunIt").font(.displayItalic(26)).padding(.bottom, 26).padding(.leading, 10)
            row("For you", .forYou)
            row("Browse", .browse)
            row("Library", .library, badge: busy)
            row("This Mac", .machine)
            Spacer()
            if let m = store.report?.machine {
                Text(m.cpu).font(.sans(12))
                Text("\(Int((m.ramBytes / 1_073_741_824).rounded())) GB memory").font(.sans(12)).foregroundStyle(Color.faint)
            }
        }
        .padding(.top, 46).padding(.bottom, 22).padding(.horizontal, 14)
        .frame(width: 172, alignment: .leading)
        .ignoresSafeArea(edges: .top)
    }

    private func row(_ title: String, _ p: Place, badge: Int = 0) -> some View {
        let on = store.place == p && store.chatKey == nil
        return Button {
            if store.chatKey != nil { store.closeChat() }
            withAnimation(.settle) { store.place = p }
        } label: {
            HStack {
                Text(title).font(on ? .sansBold(14) : .sans(14)).foregroundStyle(on ? Color.onMarker : Color.ink)
                Spacer()
                if badge > 0 { Text("\(badge)").font(.monoBold(11)).foregroundStyle(on ? Color.onMarker : Color.faint) }
            }
            .padding(.horizontal, 10).frame(height: 32)
            .background { if on { RoundedRectangle(cornerRadius: 4).fill(Color.marker).matchedGeometryEffect(id: "mark", in: mark) } }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}
