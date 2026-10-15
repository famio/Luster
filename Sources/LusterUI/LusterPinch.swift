import SwiftUI

extension View {
    /// A pinch brings the badge closer and two fingers move it about.
    /// `pinching` is up while a trackpad pinches, for its end to be seen when
    /// the gesture is cancelled.
    func pinchable(_ stage: LusterStage, pinching: GestureState<Bool>) -> some View {
        #if canImport(UIKit)
        gesture(PinchGesture(stage: stage))
        #else
        // A trackpad's pinch, about the point under the pointer, which
        // stays put as the fingers spread.
        simultaneousGesture(
            MagnifyGesture()
                .updating(pinching) { _, state, _ in state = true }
                .onChanged { stage.pinch(scale: $0.magnification, at: $0.startLocation) }
                .onEnded { _ in stage.pinchEnded() }
        )
        .background(TrackpadSlide(stage: stage))
        #endif
    }
}

#if canImport(UIKit)
import UIKit

/// Two fingers on the badge: their spread brings it closer and their
/// middle carries it about. UIKit's own pinch, for the point between the
/// fingers as they move, which SwiftUI's magnify gesture does not give.
struct PinchGesture: UIGestureRecognizerRepresentable {
    let stage: LusterStage

    func makeUIGestureRecognizer(context: Context) -> UIPinchGestureRecognizer {
        let pinch = UIPinchGestureRecognizer()
        pinch.delegate = context.coordinator.alongside
        return pinch
    }

    func makeCoordinator(converter: CoordinateSpaceConverter) -> Coordinator {
        // SwiftUI's drag is on a view of its own: the pinch runs with any.
        Coordinator(alongside: Alongside(within: nil))
    }

    func handleUIGestureRecognizerAction(
        _ pinch: UIPinchGestureRecognizer, context: Context
    ) {
        context.coordinator.follower.follow(
            pinch, at: context.converter.localLocation, stage: stage)
    }

    final class Coordinator {
        let alongside: Alongside
        let follower = PinchFollower()

        init(alongside: Alongside) {
            self.alongside = alongside
        }
    }
}

/// Hands a pinch to the stage. A finger lifted holds the badge where it is,
/// and a second one down again carries on from there.
@MainActor
final class PinchFollower {
    private var resting = false

    func follow(_ pinch: UIPinchGestureRecognizer, at location: CGPoint, stage: LusterStage) {
        switch pinch.state {
        case .began:
            resting = false
            stage.pinchBegan(at: location)
            stage.pinchChanged(scale: pinch.scale, at: location)
        case .changed:
            guard pinch.numberOfTouches >= 2 else {
                resting = true
                return
            }
            if resting {
                resting = false
                pinch.scale = 1
                stage.pinchBegan(at: location)
            }
            stage.pinchChanged(scale: pinch.scale, at: location)
        default:
            stage.pinchEnded()
        }
    }
}

/// Lets the badge's gestures run together: a pinch with the drag already
/// turning it, a double tap with both.
final class Alongside: NSObject, UIGestureRecognizerDelegate {
    /// The view whose gestures these run with; nil for any.
    private weak var view: UIView?
    /// Whether a touch is the badge's, rather than its placeholder's.
    var receives: (UITouch) -> Bool = { _ in true }

    init(within view: UIView?) {
        self.view = view
    }

    func gestureRecognizer(
        _ gesture: UIGestureRecognizer,
        shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer
    ) -> Bool {
        view == nil || other.view === view
    }

    func gestureRecognizer(_ gesture: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        receives(touch)
    }
}
#elseif canImport(AppKit)
import AppKit

/// A trackpad's two-finger slide, or a mouse's wheel, over the badge brought
/// closer moves it; at its size the slide scrolls what is round it. Seen
/// before the views under the pointer, as a slide reaches no gesture.
struct TrackpadSlide: NSViewRepresentable {
    let stage: LusterStage

    func makeNSView(context: Context) -> SlideCatcher {
        SlideCatcher(stage: stage)
    }

    func updateNSView(_ view: SlideCatcher, context: Context) {}
}

final class SlideCatcher: NSView {
    private let stage: LusterStage
    nonisolated(unsafe) private var monitor: Any?
    /// Whether the slide under way is the badge's: decided as it begins, so
    /// a scroll that starts elsewhere, and its momentum, pass over the badge.
    private var taking = false

    init(stage: LusterStage) {
        self.stage = stage
        super.init(frame: .zero)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    deinit {
        if let monitor { NSEvent.removeMonitor(monitor) }
    }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        guard window != nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
            guard let self else { return event }
            return MainActor.assumeIsolated { self.take(event) } ? nil : event
        }
    }

    private func take(_ event: NSEvent) -> Bool {
        let begins = event.phase.contains(.began) || event.phase.contains(.mayBegin)
        // A mouse's wheel has no phases: each turn of it is a slide of its own.
        let wheel = event.phase.isEmpty && event.momentumPhase.isEmpty
        if begins || wheel {
            // Over the part of the badge on screen, not one scrolled away.
            taking =
                event.window === window
                && visibleRect.contains(convert(event.locationInWindow, from: nil))
                && stage.canSlide
        }
        guard taking else { return false }
        // A mouse's wheel counts lines.
        let unit: CGFloat = event.hasPreciseScrollingDeltas ? 1 : 16
        return stage.slide(
            by: CGSize(width: event.scrollingDeltaX * unit, height: event.scrollingDeltaY * unit))
    }
}
#endif
