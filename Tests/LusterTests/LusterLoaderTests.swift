import Foundation
import Testing
@testable import Luster

/// Serves documents from memory under https://luster.test/, as an image
/// pipeline would from its cache; anything else is not found.
private struct Shelf: LusterLoader {
    static let svg: Data = {
        let here = URL(fileURLWithPath: #filePath)
        let file = here.deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("fixtures/svg/sample-badge.svg")
        return (try? Data(contentsOf: file)) ?? Data()
    }()

    func data(for url: URL) async throws -> Data {
        switch url.path {
        case "/badge.svg": return Self.svg + Data("<!-- shelf -->".utf8)
        case "/huge.svg": return Data(count: LusterSource.maximumBytes + 1)
        default: throw URLError(.fileDoesNotExist)
        }
    }
}

// The loader is the process's, so these run one after another, and only they
// ask for web sources.
@Suite(.serialized) struct LusterLoaderTests {
    init() { LusterEngine.loader = Shelf() }

    @Test func aWebSourceIsFetchedWithTheLoader() async throws {
        let badge = try await LusterEngine.mint(.url(URL(string: "https://luster.test/badge.svg")!))
        let same = try await LusterEngine.mint(.data(Shelf.svg + Data("<!-- shelf -->".utf8)))
        #expect(badge == same)
    }

    @Test func whatTheLoaderCannotFetchIsUnreadable() async throws {
        await #expect {
            try await LusterEngine.mint(.url(URL(string: "https://luster.test/missing.svg")!))
        } throws: { error in
            if case LusterError.unreadableSource = error { true } else { false }
        }
    }

    @Test func tooMuchIsRefused() async throws {
        await #expect(throws: LusterError.tooLarge(LusterSource.maximumBytes + 1)) {
            try await LusterEngine.mint(.url(URL(string: "https://luster.test/huge.svg")!))
        }
    }
}
