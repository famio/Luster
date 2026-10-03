import Foundation
import LusterUI

/// Everything the demo's menus set.
struct DemoSettings: Equatable {
    /// `-url <url>` opens on a document on the web instead of the first sample.
    var source: LusterSource? = DemoSettings.startingURL.map { .url($0) }
        ?? Sample.first.map { .url($0.url) }
    /// What the SVG button shows: the name of the document on the badge.
    var sourceTitle = DemoSettings.startingURL.map { $0.deletingPathExtension().lastPathComponent }
        ?? Sample.first?.title ?? "—"

    private static var startingURL: URL? {
        let arguments = CommandLine.arguments
        guard let index = arguments.firstIndex(of: "-url"), index + 1 < arguments.endIndex
        else { return nil }
        return URL(string: arguments[index + 1])
    }
    /// The lines and the edge's top left in the metal rather than plated in
    /// the art's colours. `-metal-lines` opens on it, for screenshots.
    var metalLines = CommandLine.arguments.contains("-metal-lines")
    /// The metal and the light. `-lighting off` opens on the lamps' light,
    /// for screenshots.
    var appearance = LusterAppearance(lighting: DemoSettings.startingLighting)

    private static var startingLighting: LusterLighting {
        let arguments = CommandLine.arguments
        guard let index = arguments.firstIndex(of: "-lighting"), index + 1 < arguments.endIndex
        else { return .showcase }
        return LusterLighting(rawValue: arguments[index + 1]) ?? .showcase
    }

    var options: LusterOptions { LusterOptions(metalLines: metalLines) }

    /// Puts `source` on the badge, under `title`.
    mutating func show(_ source: LusterSource, titled title: String) {
        self.source = source
        sourceTitle = title
    }

    /// Puts a file the user picked on the badge, under its name.
    mutating func open(_ url: URL) {
        show(.url(url), titled: url.deletingPathExtension().lastPathComponent)
    }
}

/// One of the demo's menus: what it is called, its choices, and which one is
/// set. The SVG and the GLB are not menus: one has a button that opens a
/// file, the other a button that saves one.
struct DemoMenu: Identifiable {
    struct Item: Identifiable {
        let title: String
        let isOn: (DemoSettings) -> Bool
        let apply: (inout DemoSettings) -> Void
        var id: String { title }
    }

    let title: String
    let symbol: String
    let items: [Item]
    var id: String { title }

    /// What the menu's button shows: the choice that is set.
    func current(_ settings: DemoSettings) -> String {
        items.first { $0.isOn(settings) }?.title ?? "—"
    }

    @MainActor static let all: [DemoMenu] = [lines, metal, light]

    /// The button before the menus, which opens an SVG file.
    static let openTitle = "SVG"
    static let openSymbol = "folder"

    /// The button after the menus, which saves the badge as a GLB.
    static let exportTitle = "GLB"
    static let exportSymbol = "square.and.arrow.up"

    @MainActor static let lines = choices("Lines", "scribble.variable", \.metalLines,
                               [("Plated", false), ("Metal", true)])

    @MainActor static let metal = choices("Metal", "paintpalette", \.appearance.metal, [
        ("Gold", .gold),
        ("Silver", .silver),
        ("Copper", .copper),
    ])

    @MainActor static let light = choices("Light", "lightbulb", \.appearance.lighting,
                               [("Showcase", .showcase), ("Off", .off)])

    @MainActor private static func choices<Value: Equatable>(
        _ title: String, _ symbol: String,
        _ path: WritableKeyPath<DemoSettings, Value>,
        _ values: [(String, Value)]
    ) -> DemoMenu {
        DemoMenu(title: title, symbol: symbol, items: values.map { name, value in
            Item(title: name, isOn: { $0[keyPath: path] == value }, apply: { $0[keyPath: path] = value })
        })
    }
}
