import CRunIt
import Foundation

struct CoreError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

/// The Rust engine. Calls may block on the network, so they run off the main thread.
final class Core: @unchecked Sendable {
    private let app: OpaquePointer
    private let queue = DispatchQueue(label: "runit.calls", qos: .userInitiated, attributes: .concurrent)
    private let onEvent: @Sendable (Event) -> Void

    static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.keyDecodingStrategy = .convertFromSnakeCase
        return d
    }()

    private final class Box { weak var core: Core? }
    private let box: Box

    init?(onEvent: @escaping @Sendable (Event) -> Void) {
        let box = Box()
        let context = Unmanaged.passRetained(box).toOpaque()
        guard let app = runit_open(nil, { context, json in
            guard let context, let json else { return }
            let data = Data(bytes: json, count: strlen(json))
            guard let event = try? Core.decoder.decode(Event.self, from: data) else { return }
            Unmanaged<Box>.fromOpaque(context).takeUnretainedValue().core?.onEvent(event)
        }, context) else { return nil }
        self.app = app
        self.box = box
        self.onEvent = onEvent
        box.core = self
    }

    private struct Answer<T: Decodable>: Decodable {
        let ok: T?
        let error: String?
    }

    func call<T: Decodable & Sendable>(_ method: String, _ params: [String: Any] = [:], as: T.Type = T.self) async throws -> T {
        let body = try JSONSerialization.data(withJSONObject: params)
        return try await withCheckedThrowingContinuation { done in
            queue.async { [app] in
                let out = method.withCString { m in String(decoding: body, as: UTF8.self).withCString { runit_call(app, m, $0) } }
                guard let out else { return done.resume(throwing: CoreError(message: "RunIt did not answer")) }
                let data = Data(bytes: out, count: strlen(out))
                runit_free(out)
                done.resume(with: Result {
                    let a = try Core.decoder.decode(Answer<T>.self, from: data)
                    if let e = a.error { throw CoreError(message: e) }
                    guard let ok = a.ok else { throw CoreError(message: "RunIt gave an empty answer") }
                    return ok
                })
            }
        }
    }

    func run(_ method: String, _ params: [String: Any] = [:]) async throws {
        let body = try JSONSerialization.data(withJSONObject: params)
        struct Failure: Decodable { let error: String? }
        let error: String? = await withCheckedContinuation { done in
            queue.async { [app] in
                let out = method.withCString { m in String(decoding: body, as: UTF8.self).withCString { runit_call(app, m, $0) } }
                guard let out else { return done.resume(returning: "RunIt did not answer") }
                let data = Data(bytes: out, count: strlen(out))
                runit_free(out)
                done.resume(returning: (try? JSONDecoder().decode(Failure.self, from: data))?.error)
            }
        }
        if let error { throw CoreError(message: error) }
    }

    func close() { runit_close(app) }
}
