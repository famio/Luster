import UniformTypeIdentifiers
import XCTest
@testable import LusterMacKit

/// Drop handling. A drop that delivers nothing usable is reported differently from
/// art that fails to parse.
final class SVGDropTests: XCTestCase {

    private var directory: URL!

    override func setUpWithError() throws {
        directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("luster-drop-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        // Lets cleanup succeed after a test has chmodded a file to 000.
        if let directory {
            try? FileManager.default.setAttributes([.posixPermissions: 0o755],
                                                   ofItemAtPath: directory.path)
            try? FileManager.default.removeItem(at: directory)
        }
    }

    private static let sample = """
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
      <path d="M20,20 H80 V80 H20 Z" fill="black"/>
    </svg>
    """

    private func write(_ body: String = sample, named name: String) throws -> URL {
        let url = directory.appendingPathComponent(name)
        try Data(body.utf8).write(to: url)
        return url
    }

    /// Reads a drop, asserts whether it was accepted, and returns the delivered outcome.
    private func result(of providers: [NSItemProvider],
                        accepted: Bool,
                        file: StaticString = #filePath, line: UInt = #line)
        throws -> Result<SVGDrop.Item, SVGDrop.Failure> {
        let arrived = expectation(description: "drop delivered")
        // Written once by whatever queue the provider answers on, read after
        // the wait; the expectation is the handover.
        nonisolated(unsafe) var captured: Result<SVGDrop.Item, SVGDrop.Failure>?
        let handled = SVGDrop.read(providers) { outcome in
            captured = outcome
            arrived.fulfill()
        }
        XCTAssertEqual(handled, accepted, file: file, line: line)
        wait(for: [arrived], timeout: 5)
        return try XCTUnwrap(captured, file: file, line: line)
    }

    // MARK: A file dragged from the Finder

    func testFileURLProviderDeliversTheFile() throws {
        let url = try write(named: "badge.svg")
        let outcome = try result(of: [NSItemProvider(contentsOf: url)!], accepted: true)

        let item = try XCTUnwrap(try? outcome.get())
        XCTAssertEqual(item.name, "badge.svg")
        XCTAssertEqual(String(decoding: item.data, as: UTF8.self), Self.sample,
                       "the bytes a drop delivers have to be the ones written")
    }

    /// Drags from a browser or design tool carry SVG data rather than a file URL.
    func testRawSVGDataProviderIsAccepted() throws {
        let provider = NSItemProvider()
        provider.registerDataRepresentation(forTypeIdentifier: UTType.svg.identifier,
                                            visibility: .all) { completion in
            completion(Data(Self.sample.utf8), nil)
            return nil
        }
        let item = try XCTUnwrap(try? result(of: [provider], accepted: true).get())
        XCTAssertEqual(item.name, "dropped.svg")
        XCTAssertEqual(String(decoding: item.data, as: UTF8.self), Self.sample)
    }

    func testUnusableDropIsRejected() throws {
        let provider = NSItemProvider(object: "just text" as NSString)
        let outcome = try result(of: [provider], accepted: false)
        XCTAssertEqual(try? outcome.get() != nil, nil)
        guard case let .failure(failure) = outcome else {
            return XCTFail("plain text is not a drop this app can use")
        }
        XCTAssertEqual(failure, .nothingUsable)
    }

    func testEmptyDropIsRejected() throws {
        guard case let .failure(failure) = try result(of: [], accepted: false) else {
            return XCTFail("an empty drop delivers nothing")
        }
        XCTAssertEqual(failure, .nothingUsable)
    }

    // MARK: Reading the file

    func testMissingFileReportsWhy() throws {
        let missing = directory.appendingPathComponent("not-there.svg")
        guard case let .failure(failure) = SVGDrop.read(contentsOf: missing) else {
            return XCTFail("a file that is not there cannot be read")
        }
        XCTAssertEqual(failure.name, "not-there.svg")
    }

    /// macOS denies an unsandboxed app Desktop, Documents and Downloads until allowed,
    /// as a plain read error; it must be surfaced and point at the privacy settings.
    func testPermissionFailureSaysSoAndPointsAtSettings() throws {
        let url = try write(named: "locked.svg")
        try FileManager.default.setAttributes([.posixPermissions: 0o000],
                                              ofItemAtPath: url.path)
        try XCTSkipIf(FileManager.default.isReadableFile(atPath: url.path),
                      "running with rights that ignore file permissions")

        guard case let .failure(failure) = SVGDrop.read(contentsOf: url) else {
            return XCTFail("an unreadable file cannot be read")
        }
        let message = try XCTUnwrap(failure.errorDescription)
        XCTAssertTrue(message.contains("locked.svg"), message)
        XCTAssertTrue(message.contains("Privacy & Security"), message)
    }

    // MARK: The documents on disk

    /// The fixtures load through a drop. What the engine makes of them is the
    /// engine's own business; this is about the read.
    func testFixturesRoundTripThroughADrop() throws {
        // …/Apps/LusterMac/Tests/LusterMacKitTests/ up to the repository.
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() }
        let fixtures = root.appendingPathComponent("fixtures/svg")
        let names = (try? FileManager.default.contentsOfDirectory(atPath: fixtures.path)) ?? []
        let svgs = names.filter { $0.hasSuffix(".svg") }
        XCTAssertFalse(svgs.isEmpty, "fixtures/svg should hold the documents")

        for name in svgs {
            let item = try XCTUnwrap(try? SVGDrop.read(contentsOf: fixtures
                .appendingPathComponent(name)).get())
            XCTAssertEqual(item.name, name)
            XCTAssertGreaterThan(item.data.count, 100, "\(name) came back empty")
            XCTAssertTrue(String(decoding: item.data.prefix(512), as: UTF8.self).contains("<svg"),
                          "\(name) does not look like an SVG")
        }
    }
}
