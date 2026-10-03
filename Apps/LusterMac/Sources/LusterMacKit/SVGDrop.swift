import Foundation
import UniformTypeIdentifiers

/// Turns a drop onto the stage into SVG bytes.
public enum SVGDrop {

    /// Drop types the stage accepts: a file URL (Finder) or the SVG data itself
    /// (browsers, design tools).
    public static let acceptedTypes: [UTType] = [.fileURL, .svg]

    public struct Item: Sendable {
        public let data: Data
        public let name: String

        public init(data: Data, name: String) {
            self.data = data
            self.name = name
        }
    }

    /// Why a drop did not become a badge.
    public enum Failure: LocalizedError, Equatable, Sendable {
        case nothingUsable
        case unreadable(name: String, reason: String)

        public var errorDescription: String? {
            switch self {
            case .nothingUsable:
                return String(localized: "There’s no SVG in what was dropped.")
            case let .unreadable(name, reason):
                return String(localized: "\(name) can’t be read. \(reason)")
            }
        }

        /// The file the failure is about, for the stage's caption.
        public var name: String? {
            switch self {
            case .nothingUsable: return nil
            case let .unreadable(name, _): return name
            }
        }
    }

    /// Reads the first usable item in a drop. Returns false when nothing in the
    /// drop is usable, as `onDrop` expects.
    @discardableResult
    public static func read(_ providers: [NSItemProvider],
                            completion: @escaping @Sendable (Result<Item, Failure>) -> Void) -> Bool {
        guard let provider = providers.first else {
            completion(.failure(.nothingUsable))
            return false
        }

        if provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
            _ = provider.loadObject(ofClass: URL.self) { url, error in
                guard let url else {
                    completion(.failure(.unreadable(name: String(localized: "The file"),
                                                    reason: reason(for: error))))
                    return
                }
                completion(read(contentsOf: url))
            }
            return true
        }
        if provider.hasItemConformingToTypeIdentifier(UTType.svg.identifier) {
            provider.loadDataRepresentation(forTypeIdentifier: UTType.svg.identifier) { data, error in
                guard let data else {
                    completion(.failure(.unreadable(name: "SVG",
                                                    reason: reason(for: error))))
                    return
                }
                completion(.success(Item(data: data, name: "dropped.svg")))
            }
            return true
        }
        completion(.failure(.nothingUsable))
        return false
    }

    /// Reads a file from disk and reports why a read failed instead of discarding it.
    public static func read(contentsOf url: URL) -> Result<Item, Failure> {
        // A dropped URL may be security scoped; the call is harmless when it is not.
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        do {
            let data = try Data(contentsOf: url)
            return .success(Item(data: data, name: url.lastPathComponent))
        } catch {
            return .failure(.unreadable(name: url.lastPathComponent,
                                        reason: reason(for: error)))
        }
    }

    private static func reason(for error: Error?) -> String {
        guard let error else { return String(localized: "The cause is unknown.") }
        let message = (error as NSError).localizedDescription
        // A refused macOS folder permission looks like a plain read failure; add a hint.
        if (error as NSError).code == NSFileReadNoPermissionError {
            return String(localized: """
                \(message) Allow Luster to access this folder in System Settings \
                > Privacy & Security.
                """)
        }
        return message
    }
}
