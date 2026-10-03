import XCTest
@testable import LusterMacKit

final class BadgeFileNameTests: XCTestCase {

    func testExportBaseStripsTheExtension() {
        XCTAssertEqual(BadgeFileName.exportBase(from: "trophy.svg"), "trophy")
        XCTAssertEqual(BadgeFileName.exportBase(from: "my.logo.v2.svg"), "my.logo.v2")
        XCTAssertEqual(BadgeFileName.exportBase(from: "badge"), "badge")
    }

    /// The caption doubles as the export name, so its failure note is stripped.
    func testExportBaseStripsTheFailureNote() {
        let caption = BadgeFileName.failureCaption(for: "broken.svg")
        XCTAssertEqual(BadgeFileName.exportBase(from: caption), "broken")
    }

    /// Parentheses are legal in filenames; only the failure note is stripped.
    func testParenthesesInRealNamesSurvive() {
        XCTAssertEqual(BadgeFileName.exportBase(from: "logo (final).svg"), "logo (final)")
    }

    func testPlaceholderStillExportsToSomething() {
        XCTAssertEqual(BadgeFileName.exportBase(from: BadgeFileName.placeholder), "badge")
        XCTAssertEqual(BadgeFileName.exportBase(from: ""), "badge")
        // ".svg" is a dotfile name, not an extension: the stem is "svg", not empty.
        XCTAssertEqual(BadgeFileName.exportBase(from: ".svg"), "svg")
    }
}

extension BadgeFileNameTests {
    /// The leading dot is dropped so the export is not a hidden file.
    func testHiddenNamesDoNotProduceHiddenExports() {
        XCTAssertEqual(BadgeFileName.exportBase(from: ".hidden.svg"), "hidden")
        XCTAssertFalse(BadgeFileName.exportBase(from: ".hidden.svg").hasPrefix("."))
    }
}
