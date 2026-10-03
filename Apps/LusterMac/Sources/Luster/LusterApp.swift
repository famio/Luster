import AppKit
import SwiftUI

struct LusterApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var model = AppModel.shared

    var body: some Scene {
        // A single `Window`, not a `WindowGroup`: a group opens a new window
        // with fresh state for events such as an open-file request.
        Window("Luster", id: "luster.main") {
            ContentView(model: model)
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentMinSize)
        .defaultSize(width: 1160, height: 760)
        .commands {
            CommandGroup(replacing: .newItem) {}
            CommandGroup(after: .newItem) {
                Button("Open SVG…") { model.pickFile() }
                    .keyboardShortcut("o", modifiers: .command)
                Button("Clear") { model.reset() }
                Divider()
                Button("Export GLB…") { model.exportGLB() }
                    .keyboardShortcut("e", modifiers: [.command, .shift])
                    .disabled(model.isExporting)
            }
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        // A bare SwiftPM binary has no bundle; set the activation policy by hand.
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    /// Handles "Open with Luster" and `open -a Luster art.svg` (the
    /// Info.plist declares the SVG type) by loading into the single window.
    func application(_ application: NSApplication, open urls: [URL]) {
        guard let svg = urls.first else { return }
        AppModel.shared.load(url: svg)
        NSApp.windows.first?.makeKeyAndOrderFront(nil)
    }
}
