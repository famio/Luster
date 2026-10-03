import Foundation
import Testing
@testable import Luster

/// The engine linked in is the one this checkout builds: its version is the
/// workspace's, which Scripts/release.sh moves.
@Test func theEngineReportsItsVersion() throws {
    let cargo = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("rust/Cargo.toml")
    let line = try String(contentsOf: cargo, encoding: .utf8)
        .split(separator: "\n").first { $0.hasPrefix("version = ") }
    let version = try #require(line?.split(separator: "\"").dropFirst().first)
    #expect(LusterEngine.version == String(version))
}

/// A badge with enough in it that a mint takes long enough to cancel.
///
/// Each test asks for its own copy: the engine keeps recent badges by the bytes
/// it was given, and tests run side by side, so sharing a document would mean
/// sharing a cache entry.
private func badge(_ mark: String = #function) throws -> LusterSource {
    let here = URL(fileURLWithPath: #filePath)
    let svg = here.deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("fixtures/svg/sample-badge.svg")
    var data = try Data(contentsOf: svg)
    data.append(contentsOf: Array("<!-- \(mark) -->".utf8))
    return LusterSource.data(data)
}

@Test func cancellingAMintThrows() async throws {
    let source = try badge()
    let task = Task { try await LusterEngine.mint(source, options: .init(metalLines: true)) }
    task.cancel()
    await #expect(throws: CancellationError.self) { try await task.value }
}

@Test func theLastOfSeveralSourcesIsTheOneThatArrives() async throws {
    let source = try badge()
    // What a view does when the source changes under it: cancel the one in
    // flight and start again. Only the last is awaited.
    var superseded: [Task<LusterBadge, Error>] = []
    for _ in 0..<3 {
        superseded.append(Task { try await LusterEngine.mint(source) })
        superseded.last?.cancel()
    }
    let badge = try await LusterEngine.mint(source)
    #expect(!badge.designKey.isEmpty)
    for task in superseded {
        // Each either never started or stopped; none of them hands back a badge
        // the view would have to throw away.
        await #expect(throws: CancellationError.self) { try await task.value }
    }
}

@Test func theSameSourceIsMintedOnce() async throws {
    let source = try badge()
    let first = try await LusterEngine.mint(source)
    let second = try await LusterEngine.mint(source)
    // The engine keeps it: the second ask is answered from what it kept.
    // (That it is not struck again is the engine's own test.)
    #expect(first == second)
}

@Test func mintsRunTogetherWithoutTrippingOverEachOther() async throws {
    let source = try badge()
    let keys = try await withThrowingTaskGroup(of: String.self) { group in
        for metalLines in [false, true, false, true] {
            group.addTask { try await LusterEngine.mint(source, options: .init(metalLines: metalLines)).designKey }
        }
        return try await group.reduce(into: Set<String>()) { $0.insert($1) }
    }
    // Two badges, struck twice each: the options are part of what a key names.
    #expect(keys.count == 2)
}

@Test func askingTwiceWhileMintingSharesTheMint() async throws {
    let source = try badge()
    async let first = LusterEngine.mint(source)
    async let second = LusterEngine.mint(source)
    let (a, b) = try await (first, second)
    // One mint, one badge: the second ask waited for the first's.
    #expect(a == b)
}

@Test func oneCallerGivingUpLeavesTheOtherItsBadge() async throws {
    let source = try badge()
    let quitter = Task { try await LusterEngine.mint(source) }
    let stayer = Task { try await LusterEngine.mint(source) }
    try await Task.sleep(for: .milliseconds(20))
    quitter.cancel()
    await #expect(throws: CancellationError.self) { try await quitter.value }
    // The mint they share goes on for the one still waiting.
    let badge = try await stayer.value
    #expect(!badge.designKey.isEmpty)
}
