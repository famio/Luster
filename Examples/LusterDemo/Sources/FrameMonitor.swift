import Observation
import QuartzCore

/// Measures how long the main thread goes without delivering a frame. A mint
/// that blocked the main thread shows up as a long gap.
@MainActor @Observable
final class FrameMonitor: NSObject {
    /// The longest gap between frames since the last `reset()`, milliseconds.
    private(set) var longestGap: Double = 0
    @ObservationIgnored private var last: CFTimeInterval = 0
    @ObservationIgnored private var link: CADisplayLink?

    func start() {
        guard link == nil else { return }
        let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
        link.add(to: .main, forMode: .common)
        self.link = link
    }

    func reset() {
        longestGap = 0
        last = 0
    }

    @objc private func tick(_ link: CADisplayLink) {
        let now = link.timestamp
        if last > 0 { longestGap = max(longestGap, (now - last) * 1000) }
        last = now
    }
}
