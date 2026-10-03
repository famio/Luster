import XCTest

/// `-compare`: LusterView and LusterUIView set the same way, so their stills
/// can be compared. Written to $LUSTER_SHOTS. Run it once for each view:
///
///     LUSTER_SHOTS=<dir> TEST_RUNNER_LUSTER_COMPARE_VIEW=swiftui xcodebuild test \
///         -scheme LusterDemo-iOS -only-testing:LusterDemo-iOSUITests/CompareTests …
///     LUSTER_SHOTS=<dir> xcodebuild test … (the same, for LusterUIView)
///
/// then compare s-<name>.png with u-<name>.png: drawn alike, they match pixel
/// for pixel.
@MainActor
final class CompareTests: XCTestCase {
    /// One view alone, LusterUIView unless $LUSTER_COMPARE_VIEW is swiftui,
    /// through every setting at rest, then flicked and stopped. The stills,
    /// u-… and s-…, are compared afterwards.
    func testEachSettingOnOneViewAlone() throws {
        let app = XCUIApplication()
        let swiftUI = ProcessInfo.processInfo.environment["LUSTER_COMPARE_VIEW"] == "swiftui"
        let which = swiftUI ? "-only-swiftui" : "-only-uiview"
        let tag = swiftUI ? "s" : "u"
        app.launchArguments = ["-compare", "-sample", "fuji", which]
        app.launch()
        func ready() {
            let ready = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'ready'"))
            let deadline = Date().addingTimeInterval(60)
            while ready.count < 1 && Date() < deadline { usleep(250_000) }
            sleep(2)
        }
        ready()
        shot(app, "\(tag)-1-showcase")
        pick(app, menu: "Light", "Off")
        shot(app, "\(tag)-2-off")
        pick(app, menu: "Light", "Showcase")
        pick(app, menu: "Metal", "Silver")
        shot(app, "\(tag)-3-silver")
        pick(app, menu: "Metal", "Copper")
        shot(app, "\(tag)-4-copper")
        pick(app, menu: "Metal", "Gold")
        pick(app, menu: "Lines", "Metal")
        ready()
        shot(app, "\(tag)-5-metal-lines")
        pick(app, menu: "Lines", "Plated")
        app.buttons.matching(identifier: "SVG").firstMatch.tap()
        sleep(1)
        ready()
        shot(app, "\(tag)-6-next-sample")
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(3)
        shot(app, "\(tag)-7-landscape")
        XCUIDevice.shared.orientation = .portrait
        sleep(3)
        let badge = app.otherElements.matching(identifier: "Badge").firstMatch
        XCTAssertTrue(badge.exists, "the badge is an accessibility element named Badge")
        flick(badge)
        usleep(100_000)
        shot(app, "\(tag)-8-flick-a")
        usleep(250_000)
        shot(app, "\(tag)-8-flick-b")
        sleep(5)
        shot(app, "\(tag)-8-flick-rest")
        flick(badge)
        usleep(150_000)
        badge.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).press(forDuration: 0.3)
        shot(app, "\(tag)-9-stopped-a")
        usleep(600_000)
        shot(app, "\(tag)-9-stopped-b")
    }

    private func flick(_ badge: XCUIElement) {
        badge.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.5))
            .press(forDuration: 0.05,
                   thenDragTo: badge.coordinate(withNormalizedOffset: CGVector(dx: 0.7, dy: 0.5)),
                   withVelocity: 2500, thenHoldForDuration: 0)
    }

    private func pick(_ app: XCUIApplication, menu: String, _ choice: String) {
        app.buttons.matching(identifier: menu).firstMatch.tap()
        sleep(1)
        // The choice, inside the open menu: "Metal" is a menu's name as well.
        app.collectionViews.buttons[choice].firstMatch.tap()
        sleep(2)
    }

    private func shot(_ app: XCUIApplication, _ name: String) {
        let png = app.screenshot().pngRepresentation
        let attachment = XCTAttachment(data: png, uniformTypeIdentifier: "public.png")
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        if let dir = ProcessInfo.processInfo.environment["LUSTER_SHOTS"] {
            try? png.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name + ".png"))
        }
    }
}
