import Luster

/// What a badge view is showing.
public enum LusterState: Sendable {
    /// No source yet.
    case idle
    /// Minting the source; the previous badge, if any, stays on screen.
    case minting
    case ready(LusterBadge)
    case failed(any Error)

    public var badge: LusterBadge? {
        if case .ready(let badge) = self { badge } else { nil }
    }
}
