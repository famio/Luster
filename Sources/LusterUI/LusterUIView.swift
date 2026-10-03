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
        didSet { if source != oldValue { reload() } }
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
                self.watchState()
            }
        }
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
