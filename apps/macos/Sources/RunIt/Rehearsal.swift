import AppKit
import SwiftUI

/// With RUNIT_SHOTS set, RunIt walks through its own screens and saves pictures of its own window.
/// This is how the release build is checked, here and in CI.
@MainActor
enum Rehearsal {
    private static let env = ProcessInfo.processInfo.environment
    static let folder = env["RUNIT_SHOTS"].map { URL(fileURLWithPath: $0) }

    static func begin(_ store: Store) {
        guard let folder else { return }
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        Task {
            do { try await walk(store) } catch { say("FAILED \(error.localizedDescription)"); exit(1) }
            say("REHEARSAL OK")
            exit(0)
        }
    }

    private static func walk(_ store: Store) async throws {
        try await until("the app to open", 30) { store.ready || store.fatal != nil }
        if let f = store.fatal { throw CoreError(message: f) }
        if store.report == nil || env["RUNIT_RETEST"] != nil {
            store.entered = false
            try await film("open", seconds: 2.2)
            store.test()
            for n in 1...4 {
                let want = n * 2
                try await until("the test to move on", 400) { store.lines.count >= want || !store.testing }
                try await pause(1.2)
                shot("test-\(n)")
            }
            try await until("the test to finish", 600) { !store.testing }
            if let e = store.testError { throw CoreError(message: e) }
            try await pause(2)
            shot("test-done")
        }
        try await until("suggestions", 240) { store.shortlist != nil || store.shortlistError != nil }
        if let e = store.shortlistError { throw CoreError(message: e) }
        if (store.shortlist?.ranked.count ?? 0) + (store.shortlist?.unrated.count ?? 0) == 0 { throw CoreError(message: "no suggestions") }
        store.go(.machine)
        store.entered = true
        try await pause(1.2)
        shot("this-computer")
        store.go(.forYou)
        try await film("for-you", seconds: 3)
        shot("for-you")
        if let c = store.shortlist?.picks.first?.choice ?? store.shortlist?.ranked.first {
            withAnimation(.settle) { store.opened = Opened(id: c.id, name: c.name, seed: c) }
            try await film("model", seconds: 2)
            try await pause(6)
            shot("model")
        }
        withAnimation(.settle) { store.go(.browse) }
        try await until("search", 60) { !store.hits.isEmpty || store.searchError != nil }
        store.query = "gemma 3"
        store.search()
        try await pause(5)
        if let e = store.searchError { throw CoreError(message: e) }
        shot("browse")
        withAnimation(.settle) { store.go(.library) }
        try await pause(1)
        shot("library")
        if let i = store.items.first(where: \.runnable) {
            store.measure(i.key)
            try await until("a measurement", 240) { store.measuring.isEmpty }
            try await pause(1)
            shot("library-measured")
            store.openChat(i.key)
            try await until("the model to load", 120) { !store.chatLoading }
            store.send("In two sentences, why is the sky blue?")
            try await until("a reply", 120) { !store.chatBusy }
            if let e = store.chatError { throw CoreError(message: e) }
            if store.turns.last?.text.isEmpty ?? true, store.turns.last?.thought.isEmpty ?? true { throw CoreError(message: "empty reply") }
            try await pause(1)
            shot("chat")
            store.closeChat()
        }
    }

    private static func pause(_ s: Double) async throws { try await Task.sleep(for: .seconds(s)) }

    private static func until(_ what: String, _ limit: Double, _ done: @MainActor () -> Bool) async throws {
        let start = Date.now
        while !done() {
            if Date.now.timeIntervalSince(start) > limit { throw CoreError(message: "gave up waiting for \(what)") }
            try await Task.sleep(for: .milliseconds(100))
        }
    }

    private static func picture() -> NSBitmapImageRep? {
        guard let v = NSApp.windows.first(where: { $0.contentView != nil && $0.frame.width > 500 })?.contentView,
              let rep = v.bitmapImageRepForCachingDisplay(in: v.bounds) else { return nil }
        v.cacheDisplay(in: v.bounds, to: rep)
        return rep
    }

    private static func shot(_ name: String) {
        guard let folder, let png = picture()?.representation(using: .png, properties: [:]) else { return say("no picture of \(name)") }
        try? png.write(to: folder.appending(path: "\(name).png"))
        say(name)
    }

    /// Frames for checking that motion is smooth.
    private static func film(_ name: String, seconds: Double) async throws {
        guard let folder, env["RUNIT_FILM"] != nil else { return try await pause(seconds) }
        let start = Date.now
        var frames: [NSBitmapImageRep] = []
        while Date.now.timeIntervalSince(start) < seconds {
            if let p = picture() { frames.append(p) }
            try await Task.sleep(for: .milliseconds(12))
        }
        let dir = folder.appending(path: "film-\(name)")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        for (n, f) in frames.enumerated() {
            try? f.representation(using: .png, properties: [:])?.write(to: dir.appending(path: String(format: "%04d.png", n)))
        }
        say("film \(name): \(frames.count) frames in \(seconds) s")
    }

    private static func say(_ s: String) { FileHandle.standardError.write(Data("rehearsal: \(s)\n".utf8)) }
}
