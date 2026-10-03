#if !canImport(UIKit) && canImport(AppKit)
import AppKit
import Luster
import SwiftUI

/// `LusterView` for AppKit, as a plain view.
public final class LusterNSView: NSView {
    public var source: LusterSource? { didSet { refresh() } }
    public var options = LusterOptions() { didSet { refresh() } }
    /// Named apart from `NSView.appearance`.
    public var badgeAppearance = LusterAppearance() { didSet { refresh() } }
    /// Whether a flick keeps the badge turning. A flick never does while the
    /// system asks for reduced motion, whatever this says.
    public var momentumEnabled = true { didSet { refresh() } }
    public var onStateChange: (@MainActor (LusterState) -> Void)? { didSet { refresh() } }

    private let hosting: NSHostingView<LusterView>

    public init(source: LusterSource? = nil, appearance: LusterAppearance = .init()) {
        self.source = source
        self.badgeAppearance = appearance
        hosting = NSHostingView(rootView: LusterView(source: source, appearance: appearance))
        super.init(frame: .zero)
        hosting.frame = bounds
        hosting.autoresizingMask = [.width, .height]
        addSubview(hosting)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    private func refresh() {
        hosting.rootView = LusterView(source: source, options: options,
                                      appearance: badgeAppearance, momentumEnabled: momentumEnabled,
                                      onStateChange: onStateChange)
    }
}
#endif
