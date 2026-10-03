internal import LusterCore

/// Why a badge could not be made.
public enum LusterError: Error, Sendable, Hashable {
    /// The data is not an SVG document.
    case invalidSVG(String)
    /// The SVG exceeds the engine's limits (shapes, path segments, …).
    case inputTooComplex(String)
    /// The SVG draws nothing a badge can be made of.
    case nothingToMint
    /// The source is larger than `LusterSource.maximumBytes`.
    case tooLarge(Int)
    /// The file or URL could not be read.
    case unreadableSource(String)
    /// The engine itself went wrong: a bug, not the document's doing.
    case engineFailure(String)

    init(_ error: LusterCore.LusterError) {
        switch error {
        case .InvalidSvg(let reason): self = .invalidSVG(reason)
        case .InputTooComplex(let reason): self = .inputTooComplex(reason)
        case .NothingToMint: self = .nothingToMint
        case .Internal(let reason): self = .engineFailure(reason)
        case .Cancelled: preconditionFailure("cancellation is a CancellationError")
        }
    }
}
