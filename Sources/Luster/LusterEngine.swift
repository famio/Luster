import Foundation
internal import LusterCore

/// Entry points to the Rust engine. Minting never runs on the caller's actor.
public enum LusterEngine {
    /// The engine's version.
    public static var version: String { LusterCore.version() }

    /// Mints a badge from an SVG, on the engine's own threads; a web source is
    /// fetched with `loader` first. Fails with a `LusterError` when the
    /// document, its source or the engine fails.
    ///
    /// Cancelling the calling task stops the engine at its next checkpoint and
    /// throws `CancellationError`, unless someone else still wants the same
    /// badge. At most two mints run at once across the process; the rest wait
    /// their turn. Recent badges are kept by their SVG bytes and options, so
    /// asking again for the same source is immediate, and asking for one
    /// already being minted waits for that mint rather than starting another.
    /// The engine does all of this, as it does on every platform.
    public static func mint(_ source: LusterSource,
                            options: LusterOptions = .init()) async throws -> LusterBadge {
        let data = try await source.load()
        try Task.checkCancellation()
        let request = LusterCore.MintRequest(
            svg: data,
            options: .init(withoutHiddenFaces: options.withoutHiddenFaces,
                           metalLines: options.metalLines))
        do {
            // The engine's future does not hear of a cancelled task: it is
            // told.
            return LusterBadge(try await withTaskCancellationHandler {
                try await request.badge()
            } onCancel: {
                request.cancel()
            })
        } catch LusterCore.LusterError.Cancelled {
            throw CancellationError()
        } catch let error as LusterCore.LusterError {
            throw LusterError(error)
        }
    }

    /// Starts making the studio's shared resources (the panorama, its lighting
    /// environment, the material prototype), so the first badge on screen need
    /// not wait for them. Call it early, e.g. at launch.
    public static func prewarm() {
        Task { await LusterScene.prewarm() }
    }
}
