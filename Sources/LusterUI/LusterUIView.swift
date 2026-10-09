#if canImport(UIKit)
import Luster
import Combine
import Observation
import RealityKit
import UIKit
import UIKit.UIGestureRecognizerSubclass

/// `LusterView` for UIKit, as a plain view: RealityKit's `ARView` with no
/// camera, so neither SwiftUI nor a hosting controller is inside it, and it
/// goes wherever a view can.
///
/// It draws the same scene as `LusterView`, from the same stage: the same
/// minting, pose, drag and momentum.
public final class LusterUIView: UIView {
    public var source: LusterSource? {
        didSet {
            // The same source again does nothing, and leaves a placeholder
            // fading out to finish.
            guard source != oldValue else { return }
            reload()
            updatePlaceholder(animated: false)
        }
    }
    public var options = LusterOptions() {
        didSet { if options != oldValue { reload() } }
    }
    /// Named apart from AppKit's `NSView.appearance`, as the macOS view is.
    public var badgeAppearance: LusterAppearance {
        didSet { if badgeAppearance != oldValue { reapply() } }
    }
    /// Whether a flick keeps the badge turning. A flick never does while the
    /// system asks for reduced motion, whatever this says.
    public var momentumEnabled = true {
        didSet { reduceMotionChanged() }
    }
    public var onStateChange: (@MainActor (LusterState) -> Void)?
    /// Shown over the view while a source is set and no badge is on screen:
    /// while the first is struck, and if it fails. It sits in the middle at
    /// the size it asks for, or covers the view if it asks for none, and fades
    /// as the badge arrives. A badge struck later takes the place of the one
    /// on screen without it. The view shows it and takes it away, so a spinner
    /// can be left turning.
    public var placeholderView: UIView? {
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

    /// Never while the placeholder is up: there is no badge to name, and
    /// VoiceOver reaches the placeholder only if the view is not an element
    /// itself.
    public override var isAccessibilityElement: Bool {
        get { super.isAccessibilityElement && !placeholderUp }
        set { super.isAccessibilityElement = newValue }
    }

    /// Holds the placeholder and is what comes and goes, so that the
    /// placeholder's own visibility stays its own: a spinner hides itself
    /// when it stops.
    private let placeholderBox = UIView()
    /// What the placeholder's `translatesAutoresizingMaskIntoConstraints` was
    /// before the view laid it out, to be given back with it.
    private var placedTranslating = true
    private let stage = LusterStage()
    private let arView = ARView(frame: .zero, cameraMode: .nonAR,
                                automaticallyConfigureSession: false)
    private var updates: (any Cancellable)?
    private var loading: Task<Void, Never>?
    private var applying: Task<Void, Never>?

    public init(source: LusterSource? = nil, appearance: LusterAppearance = .init()) {
        self.source = source
        self.badgeAppearance = appearance
        super.init(frame: .zero)

        // The badge is drawn over whatever is behind the view.
        backgroundColor = .clear
        arView.backgroundColor = .clear
        arView.environment.background = .color(.clear)
        // Nothing of AR's: no camera grain, motion blur or depth of field,
        // and no extended range, as LusterView keeps to the standard one.
        arView.renderOptions = [.disableCameraGrain, .disableMotionBlur, .disableDepthOfField,
                                .disableHDR, .disableGroundingShadows,
                                .disableAREnvironmentLighting, .disablePersonOcclusion,
                                .disableFaceMesh]
        arView.frame = bounds
        arView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        addSubview(arView)
        placeholderBox.frame = bounds
        placeholderBox.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        placeholderBox.isHidden = true
        addSubview(placeholderBox)

        let anchor = AnchorEntity(world: .zero)
        anchor.addChild(stage.root)
        arView.scene.addAnchor(anchor)
        let stage = stage
        updates = arView.scene.subscribe(to: SceneEvents.Update.self) { event in
            MainActor.assumeIsolated { stage.step(event.deltaTime) }
        }

        addGestureRecognizer(DragGestureRecognizer(target: self, action: #selector(drag)))
        isAccessibilityElement = true
        accessibilityLabel = String(localized: "Badge")
        NotificationCenter.default.addObserver(
            self, selector: #selector(reduceMotionChanged),
            name: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil)
        reduceMotionChanged()
        watchState()
        reload()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    deinit {
        loading?.cancel()
        applying?.cancel()
    }

    public override func layoutSubviews() {
        super.layoutSubviews()
        stage.aspectRatio = Float(bounds.width / max(bounds.height, 1))
    }

    private func reload() {
        loading?.cancel()
        let (source, options, appearance) = (source, options, badgeAppearance)
        loading = Task { [stage] in
            await stage.load(source, options: options, appearance: appearance)
        }
    }

    private func reapply() {
        applying?.cancel()
        let appearance = badgeAppearance
        applying = Task { [stage] in await stage.apply(appearance) }
    }

    /// Reports each state the stage takes, as `LusterView` does.
    private func watchState() {
        withObservationTracking {
            _ = stage.change
        } onChange: { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                self.onStateChange?(self.stage.state)
                // A badge comes and goes only with a new state.
                self.updatePlaceholder(animated: true)
                self.watchState()
            }
        }
    }

    // MARK: The placeholder

    /// In the middle, no larger than the view; as large as it can be if it
    /// has no size of its own.
    private func place(_ placeholder: UIView) {
        let box = placeholderBox
        placedTranslating = placeholder.translatesAutoresizingMaskIntoConstraints
        placeholder.translatesAutoresizingMaskIntoConstraints = false
        box.addSubview(placeholder)
        let fill = [placeholder.widthAnchor.constraint(equalTo: box.widthAnchor),
                    placeholder.heightAnchor.constraint(equalTo: box.heightAnchor)]
        // Under the hugging of a view with a size of its own.
        for each in fill { each.priority = .fittingSizeLevel }
        NSLayoutConstraint.activate(fill + [
            placeholder.centerXAnchor.constraint(equalTo: box.centerXAnchor),
            placeholder.centerYAnchor.constraint(equalTo: box.centerYAnchor),
            placeholder.widthAnchor.constraint(lessThanOrEqualTo: box.widthAnchor),
            placeholder.heightAnchor.constraint(lessThanOrEqualTo: box.heightAnchor),
        ])
    }

    private var placeholderUp: Bool { placeholderView != nil && source != nil && !stage.showing }

    private func updatePlaceholder(animated: Bool) {
        let box = placeholderBox
        if placeholderUp {
            box.layer.removeAllAnimations()
            box.alpha = 1
            box.isHidden = false
        } else if !box.isHidden {
            guard animated else {
                box.isHidden = true
                return
            }
            UIView.animate(withDuration: 0.2, delay: 0, options: .curveEaseOut) {
                box.alpha = 0
            } completion: { [weak self] _ in
                // Unless it was wanted back meanwhile.
                guard let self, !self.placeholderUp else { return }
                box.isHidden = true
                box.alpha = 1
            }
        }
    }

    // MARK: Turning it

    /// While the placeholder is up there is no badge to turn, and a touch is
    /// left to the placeholder, so that a button in it can be pressed.
    public override func gestureRecognizerShouldBegin(_ gesture: UIGestureRecognizer) -> Bool {
        guard gesture is DragGestureRecognizer else {
            return super.gestureRecognizerShouldBegin(gesture)
        }
        return !placeholderUp
    }

    @objc private func drag(_ gesture: DragGestureRecognizer) {
        switch gesture.state {
        case .began, .changed:
            stage.dragChanged(gesture.translation)
        case .ended, .cancelled, .failed:
            stage.dragEnded(velocity: gesture.velocity)
        default:
            break
        }
    }

    @objc private func reduceMotionChanged() {
        stage.momentumEnabled = momentumEnabled && !UIAccessibility.isReduceMotionEnabled
    }
}

/// SwiftUI's `DragGesture(minimumDistance: 0)` in UIKit: it begins as the
/// finger lands, so a touch stops a turning badge before it moves, and it
/// reports the translation from there, in points. Its velocity at release is
/// the last tenth of a second's, as Android's and Flutter's are: a finger held
/// still before lifting leaves none, where SwiftUI's keeps the last movement's.
private final class DragGestureRecognizer: UIGestureRecognizer {
    private(set) var translation = CGSize.zero
    private(set) var velocity = CGSize.zero
    private var start = CGPoint.zero
    /// Recent positions, for the velocity at release.
    private var samples: [(time: TimeInterval, point: CGPoint)] = []
    /// How far back the velocity looks.
    private static let window: TimeInterval = 0.1

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        guard state == .possible, let touch = touches.first else {
            touches.forEach { ignore($0, for: event) }
            return
        }
        start = touch.location(in: view)
        translation = .zero
        velocity = .zero
        samples = [(touch.timestamp, start)]
        state = .began
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let touch = touches.first else { return }
        record(touch)
        state = .changed
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let touch = touches.first else { return }
        record(touch)
        // A finger held still before lifting leaves no velocity.
        let recent = samples.filter { touch.timestamp - $0.time <= Self.window }
        if let first = recent.first, let last = recent.last, last.time > first.time {
            let dt = last.time - first.time
            velocity = CGSize(width: (last.point.x - first.point.x) / dt,
                              height: (last.point.y - first.point.y) / dt)
        } else {
            velocity = .zero
        }
        state = .ended
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
        velocity = .zero
        state = .cancelled
    }

    private func record(_ touch: UITouch) {
        let point = touch.location(in: view)
        translation = CGSize(width: point.x - start.x, height: point.y - start.y)
        samples.append((touch.timestamp, point))
        samples.removeAll { touch.timestamp - $0.time > Self.window }
    }
}
#endif
