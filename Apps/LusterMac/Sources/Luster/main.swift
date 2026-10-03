import AppKit
import Luster

// A plain entry point instead of `@main` on the App, so the window can be
// opened in a known state before SwiftUI takes over.
func flagValue(_ flag: String) -> String? {
    guard let i = CommandLine.arguments.firstIndex(of: flag),
          i + 1 < CommandLine.arguments.count else { return nil }
    return CommandLine.arguments[i + 1]
}

// The studio's panorama and the material prototype take a moment to make;
// start now, while the window is coming up.
LusterEngine.prewarm()

// `--lighting`, `--appearance light|dark` and `--open art.svg` are for
// photographing the same badge the same way twice.
MainActor.assumeIsolated {
    if let name = flagValue("--appearance") {
        NSApplication.shared.appearance = NSAppearance(named: name == "dark" ? .darkAqua : .aqua)
    }
    if let lighting = flagValue("--lighting").flatMap(LusterLighting.init(rawValue:)) {
        AppModel.shared.lighting = lighting
    }
    if let path = flagValue("--open") {
        AppModel.shared.load(url: URL(fileURLWithPath: path))
    }
}

LusterApp.main()
