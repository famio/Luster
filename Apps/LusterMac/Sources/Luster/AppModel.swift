import AppKit
import Luster
import LusterMacKit
import LusterUI
import SwiftUI
import UniformTypeIdentifiers

/// An error shown to the user as an alert.
struct AppAlert: Identifiable {
    let id = UUID()
    let title: String
    let message: String
}

/// The window's state: what to strike, the loaded artwork, and file loading
/// and export. Holds the badge on stage so export can write it out.
@MainActor
final class AppModel: ObservableObject {
    /// Shared so the app delegate can route Finder open requests to the
    /// single window's model.
    static let shared = AppModel()

    /// The metals offered in the sidebar, as the demos name them.
    static let metals: [(name: String, color: LusterColor)] = [
        ("Gold", .gold), ("Silver", .silver), ("Copper", .copper),
    ]

    @Published var metal: LusterColor = .gold
    @Published var lighting: LusterLighting = .showcase
    /// The lines and the edge's top left in the metal rather than plated in
    /// the art's colours.
    @Published var metalLines = false
    /// The stage caption.
    @Published private(set) var fileName: String = BadgeFileName.placeholder
    /// The source SVG bytes; nil until a file is loaded.
    @Published private(set) var svgData: Data?
    @Published var isExporting = false
    @Published var alert: AppAlert?

    /// The badge on stage, as the view reports it. Not published: it changes
    /// during a view update, and only export reads it.
    var badge: LusterBadge?

    /// What the engine is asked for, from the sidebar's controls.
    var options: LusterOptions {
        LusterOptions(metalLines: metalLines)
    }

    var appearance: LusterAppearance {
        LusterAppearance(metal: metal, lighting: lighting)
    }

    // MARK: Loading

    func pickFile() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.svg]
        panel.allowsMultipleSelection = false
        panel.message = String(localized: "Choose an SVG to make into a 3D badge")
        guard panel.runModal() == .OK, let url = panel.url else { return }
        load(url: url)
    }

    func load(url: URL) {
        apply(SVGDrop.read(contentsOf: url))
    }

    /// The stage's drop handler. Returns false when the drop holds nothing
    /// usable, as `onDrop` expects.
    func receive(_ providers: [NSItemProvider]) -> Bool {
        SVGDrop.read(providers) { [weak self] result in
            Task { @MainActor in self?.apply(result) }
        }
    }

    func load(data: Data, name: String) {
        apply(.success(SVGDrop.Item(data: data, name: name)))
    }

    private func apply(_ result: Result<SVGDrop.Item, SVGDrop.Failure>) {
        switch result {
        case let .success(item):
            svgData = item.data
            fileName = item.name
            alert = nil
        case let .failure(failure):
            report(failure)
        }
    }

    /// What the stage could not make of the document. The caption carries it
    /// and the last good badge stays on screen.
    func report(_ error: Error) {
        fileName = BadgeFileName.failureCaption(for: fileName)
        alert = AppAlert(title: String(localized: "Couldn’t Make the Badge"),
                         message: error.localizedDescription)
    }

    private func report(_ failure: SVGDrop.Failure) {
        if let name = failure.name {
            fileName = BadgeFileName.failureCaption(for: name)
        }
        alert = AppAlert(title: String(localized: "Couldn’t Open the File"),
                         message: failure.errorDescription ?? "")
    }

    func reset() {
        svgData = nil
        badge = nil
        fileName = BadgeFileName.placeholder
    }

    // MARK: Export

    func exportGLB() {
        guard let badge else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [UTType(filenameExtension: "glb") ?? .data]
        panel.nameFieldStringValue = "\(BadgeFileName.exportBase(from: fileName)).glb"
        panel.message = String(localized: "Where to save the GLB")
        guard panel.runModal() == .OK, let url = panel.url else { return }

        isExporting = true
        Task {
            // The engine writes the file's bytes off the main actor; only the
            // panels and the alert belong here.
            let glb = await badge.glb(metal: metal)
            do {
                try glb.write(to: url)
            } catch {
                alert = AppAlert(title: String(localized: "Couldn’t Export"),
                                 message: error.localizedDescription)
            }
            isExporting = false
        }
    }
}
