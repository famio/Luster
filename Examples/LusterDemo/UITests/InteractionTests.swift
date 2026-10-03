import XCTest

/// Drives the demo like a user: drag the badge, flick it, switch to the tab
/// that draws it with UIKit. Screenshots are attached to the result bundle,
/// and written to $LUSTER_SHOTS as well when that is set and reachable.
final class InteractionTests: XCTestCase {
    @MainActor
    func testDragTurnsTheBadgeAndTheUIKitTabShowsOne() throws {
        let app = XCUIApplication()
        app.launch()
        let badge = app.otherElements["Badge"].firstMatch
        XCTAssertTrue(badge.waitForExistence(timeout: 20))
        sleep(3)
        shot(app, "1-initial")

        // A slow drag to the right and down: spin and tilt, no momentum.
        let start = badge.coordinate(withNormalizedOffset: CGVector(dx: 0.3, dy: 0.5))
        let end = badge.coordinate(withNormalizedOffset: CGVector(dx: 0.7, dy: 0.6))
        start.press(forDuration: 0.1, thenDragTo: end, withVelocity: 150, thenHoldForDuration: 0.3)
        sleep(1)
        shot(app, "2-dragged")

        // A fast flick: momentum keeps it turning after release.
        start.press(forDuration: 0.05, thenDragTo: end, withVelocity: 2500, thenHoldForDuration: 0)
        usleep(150_000)
        shot(app, "3-flick-a")
        usleep(300_000)
        shot(app, "3-flick-b")

        app.buttons["UIKit"].tap()
        sleep(4)
        shot(app, "4-uikit")
        app.buttons["Light"].firstMatch.tap()
        app.buttons["Off"].firstMatch.tap()
        sleep(1)
        shot(app, "5-uikit-off")
    }

    /// The demo opens on Namiura with a button for another SVG and the three
    /// menus; picking from one shows on its button, and the tabs, which only
    /// change the view that draws the badge, keep what was picked.
    @MainActor
    func testTheTabsShareTheMenusAndWhatIsPicked() throws {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.otherElements["Badge"].firstMatch.waitForExistence(timeout: 20))

        let svg = app.buttons.matching(identifier: "SVG").firstMatch
        XCTAssertTrue(svg.waitForExistence(timeout: 5), "no SVG button")
        XCTAssertTrue(svg.label.contains("Namiura"), "the SVG button shows \(svg.label)")
        var menus: [String] = []
        for menu in ["Lines", "Metal", "Light"] {
            let button = app.buttons.matching(identifier: menu).firstMatch
            XCTAssertTrue(button.waitForExistence(timeout: 5), "no \(menu) menu")
            button.tap()
            sleep(1)
            if menu == "Metal" { shot(app, "menu") }
            let items = app.collectionViews.buttons.allElementsBoundByIndex.map(\.label)
            menus.append("\(menu): \(items.joined(separator: ", "))")
            if menu == "Metal" {
                app.collectionViews.buttons["Silver"].tap()
            } else {
                // Dismiss without choosing.
                app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.1)).tap()
            }
            sleep(1)
        }
        print("menus:", menus)
        for tab in ["SwiftUI", "UIKit", "SwiftUI"] {
            app.buttons[tab].tap()
            XCTAssertTrue(app.buttons[tab].isSelected, "\(tab): the tab did not open")
            XCTAssertTrue(app.otherElements["Badge"].firstMatch.waitForExistence(timeout: 20),
                          "\(tab): no badge")
            let metal = app.buttons.matching(identifier: "Metal").firstMatch
            XCTAssertTrue(metal.label.contains("Silver"), "\(tab): the Metal button shows \(metal.label)")
            let status = app.staticTexts.matching(identifier: "status").firstMatch
            wait(for: [expectation(for: NSPredicate(format: "label BEGINSWITH 'ready'"), evaluatedWith: status)],
                 timeout: 60)
            sleep(2)
            shot(app, "silver-\(tab)")
        }
    }

    /// On either tab, the GLB button offers to save the badge once it is
    /// ready, and swiping the save sheet away saves nothing.
    @MainActor
    func testBothTabsOfferToSaveAGLB() throws {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.otherElements["Badge"].firstMatch.waitForExistence(timeout: 20))
        for tab in ["SwiftUI", "UIKit"] {
            app.buttons[tab].tap()
            XCTAssertTrue(app.buttons[tab].isSelected, "\(tab): the tab did not open")
            let glb = app.buttons.matching(identifier: "GLB").firstMatch
            XCTAssertTrue(glb.waitForExistence(timeout: 5), "\(tab): no GLB button")
            wait(for: [expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: glb)],
                 timeout: 60)
            glb.tap()
            // The sheet is another process's and not in the app's tree, so
            // the demo's status line is what says it is up.
            let status = app.staticTexts.matching(identifier: "status").firstMatch
            wait(for: [expectation(for: NSPredicate(format: "label == 'saving Namiura.glb…'"),
                                   evaluatedWith: status)], timeout: 10)
            sleep(2)
            shot(app, "glb-\(tab)")
            // Swipe the sheet away; one that is still loading can ignore it.
            for _ in 0..<5 where status.label != "not saved" {
                app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.1))
                    .press(forDuration: 0.1, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.95)))
                sleep(2)
            }
            XCTAssertEqual(status.label, "not saved", "\(tab): the save sheet did not go away")
        }
    }

    private func shot(_ app: XCUIApplication, _ name: String) {
        let png = app.screenshot().pngRepresentation
        // On a device the runner cannot reach the Mac's filesystem, so the
        // still rides home in the result bundle as well.
        let attachment = XCTAttachment(data: png, uniformTypeIdentifier: "public.png")
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        if let dir = ProcessInfo.processInfo.environment["LUSTER_SHOTS"] {
            let url = URL(fileURLWithPath: dir).appendingPathComponent(name + ".png")
            try? png.write(to: url)
        }
    }
}
