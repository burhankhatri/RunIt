import AppKit
import Observation
import SwiftUI

enum Place: Hashable { case forYou, browse, library, machine }

struct Line: Identifiable, Equatable { let id = UUID(); let text: String }
struct Turn: Identifiable, Equatable { let id = UUID(); let mine: Bool; var text: String; var thought = "" }

@MainActor @Observable
final class Store {
    private(set) var core: Core?
    var fatal: String?
    var ready = false
    var notice: String?

    var report: Report?
    var items: [Item] = []
    var modelsDir = ""
    var version = ""
    var ratingsDate = ""

    // The test
    var testing = false
    var lines: [Line] = []
    var stage = ""
    var progress = 0.0
    var testError: String?
    /// The verdict has been read and the main screens are open.
    var entered = false

    var place = Place.forYou
    var shortlist: Shortlist?
    var catalogProgress: Double?
    var shortlistError: String?

    var query = ""
    var hits: [Hit] = []
    var browsing: Hit?
    var total = 0
    var next: String?
    var searching = false
    var searchError: String?
    private var searchTask: Task<Void, Never>?

    var chatKey: String?
    var chatLoading = false
    var chatBusy = false
    var chatError: String?
    /// Why replies are slower than they could be, if they are.
    var chatNote: String?
    var turns: [Turn] = []
    var liveSpeed = 0.0
    var measuring: Set<String> = []

    func start() {
        guard core == nil else { return }
        guard let c = Core(onEvent: { e in Task { @MainActor in Store.shared.handle(e) } }) else {
            fatal = "RunIt could not start. Check that this account can write to its Library folder."
            return
        }
        core = c
        Task {
            await refresh()
            entered = report != nil
            ready = true
            if report != nil { await recommend() }
        }
    }

    static let shared = Store()

    func refresh() async {
        guard let s = await attempt({ try await $0.call("state", as: AppState.self) }) else { return }
        report = s.report
        items = s.items
        modelsDir = s.modelsDir
        version = s.version
        ratingsDate = s.ratingsDate
        if !s.engine { fatal = "The llama.cpp engine is missing from this copy of RunIt. Run the install command again." }
    }

    private func attempt<T>(_ f: (Core) async throws -> T) async -> T? {
        guard let core else { return nil }
        do { return try await f(core) } catch {
            notice = error.localizedDescription
            return nil
        }
    }

    // MARK: test

    func test() {
        lines = []
        progress = 0
        testError = nil
        testing = true
        entered = false
        Task {
            do { try await core?.run("bench.start") } catch {
                testing = false
                testError = error.localizedDescription
            }
        }
    }

    func stopTest() {
        Task { await attempt { try await $0.run("bench.stop") } }
    }

    func recommend() async {
        shortlistError = nil
        guard let core else { return }
        do { shortlist = try await core.call("recommend", as: Shortlist.self) } catch { shortlistError = error.localizedDescription }
        catalogProgress = nil
    }

    // MARK: browse

    func search(reset: Bool = true) {
        searchTask?.cancel()
        let q = query, cursor = reset ? nil : next
        searchTask = Task {
            if reset { try? await Task.sleep(for: .milliseconds(q.isEmpty ? 0 : 260)) }
            guard !Task.isCancelled, let core else { return }
            searching = true
            searchError = nil
            do {
                var p: [String: Any] = ["q": q]
                if let cursor { p["cursor"] = cursor }
                let page = try await core.call("search", p, as: Page.self)
                guard !Task.isCancelled else { return }
                if reset { hits = page.hits } else {
                    let known = Set(hits.map(\.id))
                    hits += page.hits.filter { !known.contains($0.id) }
                }
                total = page.total
                next = page.next
            } catch {
                guard !Task.isCancelled else { return }
                searchError = error.localizedDescription
            }
            searching = false
        }
    }

    func detail(_ id: String) async throws -> Detail {
        guard let core else { throw CoreError(message: "RunIt is not ready") }
        return try await core.call("model", ["id": id], as: Detail.self)
    }

    // MARK: library

    func item(repo: String, paths: [RemoteFile]) -> Item? {
        guard let first = paths.first else { return nil }
        return items.first { $0.key == "\(repo)/\(first.path)" }
    }

    func download(_ id: String, _ files: [RemoteFile]) {
        Task { await attempt { try await $0.run("download", ["id": id, "paths": files.map(\.path)]) } }
    }
    func act(_ method: String, _ key: String) {
        Task { await attempt { try await $0.run(method, ["key": key]) } }
    }
    func reveal(_ key: String) {
        Task {
            guard let p = await attempt({ try await $0.call("path", ["key": key], as: String.self) }) else { return }
            NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: p)])
        }
    }
    func measure(_ key: String) {
        measuring.insert(key)
        if chatKey != nil { closeChat() }
        Task {
            do { try await core?.run("measure", ["key": key]) } catch {
                measuring.remove(key)
                notice = error.localizedDescription
            }
        }
    }
    func moveModels(to url: URL) {
        Task {
            if await attempt({ try await $0.run("settings.set", ["models_dir": url.path]) }) != nil {
                notice = "New downloads will go to \(url.path) after you reopen RunIt."
            }
        }
    }

    // MARK: chat

    func openChat(_ key: String) {
        chatKey = key
        turns = []
        liveSpeed = 0
        chatError = nil
        chatNote = nil
        chatLoading = true
        Task {
            do { try await core?.run("chat.open", ["key": key]) } catch { if chatKey == key { chatError = error.localizedDescription } }
            if chatKey == key { chatLoading = false }
        }
    }
    func closeChat() {
        chatKey = nil
        chatBusy = false
        Task { try? await core?.run("chat.close") }
    }
    func send(_ text: String) {
        guard !chatBusy else { return }
        chatBusy = true
        chatError = nil
        turns.append(Turn(mine: true, text: text))
        let messages = turns.map { ["role": $0.mine ? "user" : "assistant", "content": $0.text] }
        turns.append(Turn(mine: false, text: ""))
        Task {
            do { try await core?.run("chat.send", ["messages": messages]) } catch {
                chatBusy = false
                chatError = error.localizedDescription
            }
        }
    }
    func stopReply() { Task { try? await core?.run("chat.stop") } }

    // MARK: events

    private func handle(_ e: Event) {
        switch (e.type, e.kind) {
        case ("bench", "stage"):
            stage = e.text ?? ""
            progress = e.progress ?? progress
        case ("bench", "finding"):
            if let t = e.text { lines.append(Line(text: t)) }
            progress = e.progress ?? progress
        case ("bench", "done"):
            report = e.report
            progress = 1
            testing = false
            shortlist = nil
            Task { await recommend() }
        case ("bench", "stopped"):
            testing = false
            if report != nil { entered = true }
        case ("bench", "failed"):
            testing = false
            testError = e.error
        case ("catalog", _):
            catalogProgress = e.progress
        case ("item", _):
            if let gone = e.removed { items.removeAll { $0.key == gone } }
            if let i = e.item {
                if let n = items.firstIndex(where: { $0.key == i.key }) { items[n] = i } else { items.insert(i, at: 0) }
            }
        case ("measure", "done"):
            measuring.remove(e.key ?? "")
        case ("measure", "failed"):
            measuring.remove(e.key ?? "")
            notice = e.error
        case ("chat", let kind) where e.key == chatKey:
            switch kind {
            case "text": if !turns.isEmpty { turns[turns.count - 1].text += e.text ?? "" }
            case "thought": if !turns.isEmpty { turns[turns.count - 1].thought += e.text ?? "" }
            case "speed": liveSpeed = e.tokensPerS ?? liveSpeed
            case "end":
                chatBusy = false
                chatNote = e.text
            case "failed":
                chatBusy = false
                chatError = e.error
            default: break
            }
        default: break
        }
    }
}
