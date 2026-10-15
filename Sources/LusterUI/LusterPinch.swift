import SwiftUI

extension View {
    /// A pinch brings the badge closer and two fingers move it about.
    func pinchable(_ stage: LusterStage) -> some View {
        #if canImport(UIKit)
            gesture(PinchGesture(stage: stage))
        #else
            // A trackpad's pinch, about the point under the pointer, which
            // stays put as the fingers spread.
            simultaneousGesture(
                MagnifyGesture()
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
            pinch.delegate = context.coordinator
            return pinch
        }

        func makeCoordinator(converter: CoordinateSpaceConverter) -> Alongside {
            Alongside()
        }

        func handleUIGestureRecognizerAction(
            _ pinch: UIPinchGestureRecognizer, context: Context
        ) {
            let at = context.converter.localLocation
            switch pinch.state {
            case .began:
                stage.pinchBegan(at: at)
                stage.pinchChanged(scale: pinch.scale, at: at)
            case .changed:
                // A finger lifted leaves one: the pinch is over, not moved.
                if pinch.numberOfTouches >= 2 { stage.pinchChanged(scale: pinch.scale, at: at) }
            default:
                stage.pinchEnded()
            }
        }
    }

    /// Lets the pinch run with the drag that is already turning the badge.
    final class Alongside: NSObject, UIGestureRecognizerDelegate {
        func gestureRecognizer(
            _ gesture: UIGestureRecognizer,
            shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer
        ) -> Bool { true }
    }
#elseif canImport(AppKit)
    import AppKit

    /// A trackpad's two-finger slide over the badge brought closer moves it;
    /// at its size the slide scrolls what is round it, as before. Seen
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
        private var monitor: Any?

        init(stage: LusterStage) {
            self.stage = stage
            super.init(frame: .zero)
        }

        @available(*, unavailable)
        required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

        override func hitTest(_ point: NSPoint) -> NSView? { nil }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
            guard window != nil else { return }
            monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
                guard let self, event.window === window,
                    bounds.contains(convert(event.locationInWindow, from: nil))
                else { return event }
                // A mouse's wheel counts lines.
                let unit: CGFloat = event.hasPreciseScrollingDeltas ? 1 : 16
                let delta = CGSize(
                    width: event.scrollingDeltaX * unit, height: event.scrollingDeltaY * unit)
                return MainActor.assumeIsolated { stage.slide(by: delta) } ? nil : event
            }
        }
    }
#endif
