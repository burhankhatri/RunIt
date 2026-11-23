import Foundation

enum Tier: String, Decodable, Sendable { case comfortable, tight, partial, tooBig = "too_big" }

struct Prediction: Decodable, Hashable, Sendable {
    let tier: Tier
    let needBytes, capacityBytes: Double
    let tokensPerS, tokensPerSLow, tokensPerSHigh, tokensPerSDeep, tokensPerSSustained, promptTokensPerS: Double
    let gpuShare: Double
    let rough: Bool
}

struct Rating: Decodable, Hashable, Sendable { let name: String; let rating, votes: Int; let coding: Int? }
struct RemoteFile: Decodable, Hashable, Sendable { let path: String; let size: Double; let sha256: String }
struct Variant: Decodable, Hashable, Sendable { let label: String; let bits, size: Double; let files: [RemoteFile] }

struct Choice: Decodable, Hashable, Identifiable, Sendable {
    let id, repo, name, license: String
    let downloads: Int
    let rating: Rating?
    let variant: Variant
    let prediction: Prediction
}

struct Pick: Decodable, Hashable, Identifiable, Sendable {
    let role, title, why: String
    let also: [String]
    let choice: Choice
    var id: String { role }
}

struct Shortlist: Decodable, Sendable {
    let picks: [Pick]
    let ranked, unrated: [Choice]
    let ratingsDate, ratingsSource, catalogDate: String
    let catalogTotal: Int
}

struct Hit: Decodable, Hashable, Identifiable, Sendable {
    let id, repo, name, kind, license: String
    let downloads: Int
    let formats: [String]
    let size: Double?
    let label: String?
    let rating: Rating?
    let prediction: Prediction?
}
struct Page: Decodable, Sendable { let hits: [Hit]; let total: Int; let next: String? }

struct Offer: Decodable, Hashable, Sendable { let variant: Variant; let prediction: Prediction? }
struct Detail: Decodable, Sendable {
    let id, repo, name, license: String
    let note: String?
    let rating: Rating?
    let variants: [Offer]
    let others: [RemoteFile]
    let capacity: Double?
}

struct Device: Decodable, Hashable, Sendable { let id, name: String; let totalMib, freeMib: Double; let integrated: Bool }
struct Machine: Decodable, Hashable, Sendable {
    let os, cpu: String
    let cores, threads, performanceCores: Int
    let ramBytes, ramFreeBytes: Double
    let gpu: Device?
}
struct BenchRun: Decodable, Hashable, Sendable { let place: String; let modelBytes, decode, prompt: Double }
struct Calibration: Decodable, Hashable, Sendable { let sustained: Double }
struct Report: Decodable, Hashable, Sendable {
    let machine: Machine
    let calibration: Calibration
    let runs: [BenchRun]
    let takenAt, seconds, bytesPerS: Double
}

struct Conditions: Decodable, Hashable, Sendable { let hot, onBattery: Bool }
struct Measured: Decodable, Hashable, Sendable { let tokensPerS, promptTokensPerS: Double; let conditions: Conditions }
enum Status: String, Decodable, Sendable { case queued, active, paused, failed, ready }
struct Item: Decodable, Hashable, Identifiable, Sendable {
    let key, id, repo, name, label: String
    let files: [RemoteFile]
    let total, done: Double
    let status: Status
    let error: String?
    let prediction: Prediction?
    let measured: Measured?
    let bytesPerS: Double
    var runnable: Bool { status == .ready && (files.first?.path.lowercased().hasSuffix(".gguf") ?? false) }
}

struct AppState: Decodable, Sendable {
    let version: String
    let report: Report?
    let engine: Bool
    let items: [Item]
    let modelsDir: String
    let diskFree: Double?
    let ratingsDate: String
}

struct Event: Decodable, Sendable {
    let type: String
    let kind, key, text, error, removed: String?
    let progress, tokensPerS: Double?
    let report: Report?
    let item: Item?
    let measured: Measured?
}
