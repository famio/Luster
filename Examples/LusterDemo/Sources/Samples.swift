import Foundation
import LusterUI

/// The SVGs bundled with the demo: the fixtures. The demo opens on Namiura and offers
/// no other; anything else is opened from a file. The rest are there for
/// `-cycle` to mint.
struct Sample: Identifiable, Hashable {
    let name: String
    let title: String
    let url: URL
    var id: URL { url }

    /// The one the demo opens with.
    static var first: Sample? { benchmarked.first }

    /// The fixtures `-cycle` mints, in order, and what each is for.
    private static let showcase: [(name: String, title: String)] = [
        ("namiura", "Namiura"),                // an illustration: fine curls, wires on top
        ("sample-badge", "Seal"),              // solid fills: the plain case
        ("pin-with-stroke", "Pin"),            // line work and pieces apart
        ("fuji", "Fuji"),                      // layered fills, wires on top
        ("many-colour-cells", "Colour wheel"), // thirty colours under a glaze
        ("wreathed-emblem", "Wreath"),         // many small pieces, fine lines
    ]

    /// What `-cycle` mints: the showcase.
    static let benchmarked: [Sample] = {
        let fixtures = files(in: "svg")
        return showcase.compactMap { entry in
            fixtures.first { $0.deletingPathExtension().lastPathComponent == entry.name }
                .map { Sample(name: entry.name, title: entry.title, url: $0) }
        }
    }()

    private static func files(in folder: String) -> [URL] {
        guard let dir = Bundle.main.url(forResource: folder, withExtension: nil),
              let files = try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)
        else { return [] }
        return files.filter { $0.pathExtension == "svg" }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
    }
}
