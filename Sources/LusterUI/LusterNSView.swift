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
    /// Shown over the view while a source is set and no badge is on screen:
    /// while the first is struck, and if it fails. It sits in the middle at
    /// the size it asks for, or covers the view if it asks for none, and fades
    /// as the badge arrives. A badge struck later takes the place of the one
    /// on screen without it. The view shows it and takes it away, so a spinner
    /// can be left turning.
    public var placeholderView: NSView? {
        didSet {
            guard placeholderView !== oldValue else { return }
            if let oldValue, oldValue.superview === placeholderBox {
                oldValue.removeFromSuperview()
                oldValue.translatesAutoresizingMaskIntoConstraints = placedTranslating
            }
            if let placeholderView { place(placeholderView) }
            updatePlaceholder(animated: false)
        }
    }

    /// Holds the placeholder and is what comes and goes, so that the
    /// placeholder's own visibility stays its own.
    private let placeholderBox = PlaceholderBox()
    /// Whether the badge view would show a placeholder now.
    private var placeholderDue = false
    /// What the placeholder's `translatesAutoresizingMaskIntoConstraints` was
    /// before the view laid it out, to be given back with it.
    private var placedTranslating = true
    private let hosting: NSHostingView<LusterView>

    public init(source: LusterSource? = nil, appearance: LusterAppearance = .init()) {
        self.source = source
        self.badgeAppearance = appearance
        hosting = NSHostingView(rootView: LusterView(source: source, appearance: appearance))
        super.init(frame: .zero)
        hosting.frame = bounds
        hosting.autoresizingMask = [.width, .height]
        addSubview(hosting)
        placeholderBox.wantsLayer = true
        placeholderBox.frame = bounds
        placeholderBox.autoresizingMask = [.width, .height]
        placeholderBox.isHidden = true
        addSubview(placeholderBox)
        refresh()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    private func refresh() {
        var view = LusterView(source: source, options: options,
                              appearance: badgeAppearance, momentumEnabled: momentumEnabled,
                              onStateChange: onStateChange)
        view.onPlaceholderChange = { [weak self] due in
            guard let self else { return }
            placeholderDue = due
            updatePlaceholder(animated: true)
        }
        hosting.rootView = view
    }

    // MARK: The placeholder

    /// In the middle, no larger than the view; as large as it can be if it
    /// has no size of its own.
    private func place(_ placeholder: NSView) {
        let box = placeholderBox
        placedTranslating = placeholder.translatesAutoresizingMaskIntoConstraints
        placeholder.translatesAutoresizingMaskIntoConstraints = false
        box.addSubview(placeholder)
        let fill = [placeholder.widthAnchor.constraint(equalTo: box.widthAnchor),
                    placeholder.heightAnchor.constraint(equalTo: box.heightAnchor)]
        // Under the hugging of a view with a size of its own.
        for each in fill { each.priority = .fittingSizeCompression }
        NSLayoutConstraint.activate(fill + [
            placeholder.centerXAnchor.constraint(equalTo: box.centerXAnchor),
            placeholder.centerYAnchor.constraint(equalTo: box.centerYAnchor),
            placeholder.widthAnchor.constraint(lessThanOrEqualTo: box.widthAnchor),
            placeholder.heightAnchor.constraint(lessThanOrEqualTo: box.heightAnchor),
        ])
    }

    private var placeholderUp: Bool { placeholderView != nil && placeholderDue }

    private func updatePlaceholder(animated: Bool) {
        let box = placeholderBox
        box.up = placeholderUp
        if box.up {
            box.layer?.removeAllAnimations()
            box.alphaValue = 1
            box.isHidden = false
        } else if !box.isHidden {
            guard animated else {
                box.isHidden = true
                return
            }
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.2
                context.timingFunction = CAMediaTimingFunction(name: .easeOut)
                box.animator().alphaValue = 0
            } completionHandler: { [weak self] in
                MainActor.assumeIsolated {
                    // Unless it was wanted back meanwhile.
                    guard let self, !self.placeholderUp else { return }
                    box.isHidden = true
                    box.alphaValue = 1
                }
            }
        }
    }
}

/// While the placeholder is up there is no badge to turn, and a click is left
/// to the placeholder, so that a button in it can be pressed. As it fades out
/// a click goes through to the badge.
private final class PlaceholderBox: NSView {
    var up = false

    override func hitTest(_ point: NSPoint) -> NSView? {
        up ? super.hitTest(point) : nil
    }
}
#endif
