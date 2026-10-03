import Foundation

/// Where a badge's SVG comes from.
///
/// A struct with factories rather than an enum, so new kinds of source can be
/// added without breaking callers that switch over them.
public struct LusterSource: Sendable, Hashable {
    enum Storage: Sendable, Hashable {
        case url(URL)
        case data(Data)
    }

    let storage: Storage

    /// A file URL, or an http(s) URL fetched with `LusterEngine.loader`:
    /// `URLSession.shared` unless the app has set another.
    public static func url(_ url: URL) -> LusterSource { .init(storage: .url(url)) }

    /// SVG document bytes.
    public static func data(_ data: Data) -> LusterSource { .init(storage: .data(data)) }

    /// SVG document text.
    public static func svg(_ text: String) -> LusterSource { .init(storage: .data(Data(text.utf8))) }

    /// Larger inputs are refused before they are read in full.
    public static let maximumBytes = 20 << 20

    /// The SVG bytes. Reads files off the caller's actor.
    func load() async throws -> Data {
        switch storage {
        case .data(let data):
            guard data.count <= Self.maximumBytes else { throw LusterError.tooLarge(data.count) }
            return data
        case .url(let url) where url.isFileURL:
            return try await Self.read(file: url)
        case .url(let url):
            let data: Data
            do {
                data = try await LusterEngine.loader.data(for: url)
            } catch let error as LusterError {
                throw error
            } catch is CancellationError {
                throw CancellationError()
            } catch let error as URLError where error.code == .cancelled {
                throw CancellationError()
            } catch {
                throw LusterError.unreadableSource(error.localizedDescription)
            }
            guard data.count <= Self.maximumBytes else { throw LusterError.tooLarge(data.count) }
            return data
        }
    }

    @concurrent
    private static func read(file url: URL) async throws -> Data {
        // Files the user picked outside the sandbox need their scope opened.
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        do {
            let size = try url.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0
            guard size <= maximumBytes else { throw LusterError.tooLarge(size) }
            return try Data(contentsOf: url)
        } catch let error as LusterError {
            throw error
        } catch {
            throw LusterError.unreadableSource(error.localizedDescription)
        }
    }
}
