import Foundation
import Synchronization

/// Fetches the document a web `LusterSource.url` names. Set one as
/// `LusterEngine.loader` to fetch through an image pipeline's cache, such as
/// Nuke's or SDWebImage's (see the README); the default is `URLSession`'s.
///
/// What it hands back is held to `LusterSource.maximumBytes`. A loader is
/// asked only for http(s) URLs: file URLs are read directly.
public protocol LusterLoader: Sendable {
    func data(for url: URL) async throws -> Data
}

extension LusterEngine {
    private static let loaderLock = Mutex<any LusterLoader>(LusterURLSessionLoader())

    /// What web sources are fetched with from now on, process-wide.
    public static var loader: any LusterLoader {
        get { loaderLock.withLock { $0 } }
        set { loaderLock.withLock { $0 = newValue } }
    }
}

/// Fetches with a `URLSession`, which keeps responses in its `URLCache` as the
/// server's headers allow.
public struct LusterURLSessionLoader: LusterLoader {
    public let session: URLSession

    public init(session: URLSession = .shared) {
        self.session = session
    }

    public func data(for url: URL) async throws -> Data {
        let (data, response) = try await session.data(from: url)
        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            throw LusterError.unreadableSource("HTTP \(http.statusCode)")
        }
        return data
    }
}
