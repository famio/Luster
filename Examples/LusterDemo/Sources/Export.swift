import Foundation
import LusterUI
import SwiftUI
import UniformTypeIdentifiers

extension UTType {
    /// A glTF binary. The system declares no type for it, so it is known by
    /// its extension.
    static let glb = UTType(filenameExtension: "glb", conformingTo: .data) ?? .data
}

extension DemoSettings {
    /// `badge` as a GLB, in the metal it is shown in.
    func glb(of badge: LusterBadge) async -> Data {
        await badge.glb(metal: appearance.metal)
    }

    /// What the GLB is called: the document's name.
    var exportName: String { sourceTitle }

    /// What the status line says while the save sheet is up.
    static func saving(as name: String) -> String { "saving \(name).glb…" }

    /// What the status line says once `bytes` are saved as `name`.
    static func exported(_ bytes: Data, as name: String) -> String {
        "exported \(name).glb · \(bytes.count / 1024) KB"
    }

    /// What the status line says when the save sheet is dismissed unsaved.
    static let notSaved = "not saved"
}

/// A GLB for SwiftUI's file exporter. It is only ever written.
struct GLBFile: FileDocument {
    static let readableContentTypes: [UTType] = [.glb]
    let data: Data

    init(_ data: Data) { self.data = data }

    init(configuration: ReadConfiguration) throws {
        throw CocoaError(.featureUnsupported)
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: data)
    }
}
