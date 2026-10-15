import Luster
import Observation
import RealityKit
import SwiftUI

/// The live state behind a badge view: the scene on stage, the pose, the drag
/// momentum, and how close a pinch has brought the badge.
@MainActor @Observable
final class LusterStage {
    /// Added to the RealityView once; scenes come and go beneath it.
    @ObservationIgnored let root = Entity()
    private(set) var state = LusterState.idle {
        didSet { change &+= 1 }
    }
    /// Moves on every time the state is set, so a view watching it hears of a
    /// new badge even when it looks like the last one, or of a second failure
    /// worded as the first was. Setting the same state again counts: a second
    /// `.minting` is a second mint.
    private(set) var change = 0
    @ObservationIgnored private(set) var scene: LusterScene?
    /// Whether a badge is in the scene. A view with none shows its
    /// placeholder.
    private(set) var showing = false
    /// The source and options `state` is about, once a load has begun.
    @ObservationIgnored private var loaded: (source: LusterSource, options: LusterOptions)?
    /// The appearance last asked for, which a badge still being struck has to
    /// arrive in.
    @ObservationIgnored private var appearance = LusterAppearance()
    @ObservationIgnored var updates: EventSubscription?

    /// Tilt (about x) and spin (about y), radians. Kept across badges.
    @ObservationIgnored private(set) var pose = LusterScene.restingPose
    @ObservationIgnored private(set) var velocity: Float = 0
    @ObservationIgnored private var dragging = false
    @ObservationIgnored private var lastTranslation = CGSize.zero
    @ObservationIgnored var momentumEnabled = true
    /// The view's size, points.
    @ObservationIgnored var size = CGSize(width: 1, height: 1) {
        didSet { scene?.aspectRatio = aspectRatio }
    }
    private var aspectRatio: Float { Float(size.width / max(size.height, 1)) }

    /// Whether a pinch brings the badge closer; turned off, it is put back.
    @ObservationIgnored var zoomEnabled = true {
        didSet {
            guard !zoomEnabled else { return }
            pinch = nil
            see(.atRest)
        }
    }
    /// How close a pinch has brought the badge, and where it looks. Kept
    /// across badges, as the pose is.
    @ObservationIgnored private(set) var look = LusterLook.atRest
    /// The zoom and the point of the badge between the fingers as a pinch
    /// began.
    @ObservationIgnored private var pinch: (zoom: Float, point: SIMD2<Float>)?
    /// A pinch was made since the fingers came down: the one left behind
    /// does not turn the badge.
    @ObservationIgnored private var pinched = false
    /// A finger or the pointer is down, dragging.
    @ObservationIgnored private var touching = false

    private static let handling = LusterHandling.standard
    static let spinPerPoint = handling.spinPerPoint
    static let tiltPerPoint = handling.tiltPerPoint
    static let tiltLimit = handling.tiltLimit

    func load(
        _ source: LusterSource?, options: LusterOptions,
        appearance: LusterAppearance
    ) async {
        guard let source else {
            loaded = nil
            show(nil)
            state = .idle
            return
        }
        self.appearance = appearance
        loaded = (source, options)
        state = .minting
        // The studio's shared resources are made while the badge is struck,
        // if the app has not made them already.
        LusterEngine.prewarm()
        do {
            let badge = try await LusterEngine.mint(source, options: options)
            // The badge on stage already, as when a view comes back on screen:
            // kept, rather than built again.
            if let scene, scene.badge == badge {
                while scene.appearance != self.appearance {
                    await scene.apply(self.appearance)
                }
                try Task.checkCancellation()
                state = .ready(badge)
                return
            }
            let scene = try await LusterScene.make(badge, appearance: self.appearance)
            // The appearance may have changed while the scene was built.
            while scene.appearance != self.appearance {
                await scene.apply(self.appearance)
            }
            // A newer source may have taken over while this one was built.
            try Task.checkCancellation()
            show(scene)
            state = .ready(badge)
        } catch is CancellationError {
            // Superseded: the newer load reports its own state.
        } catch _ where Task.isCancelled {
            // Superseded too, failing as its loader was stopped: a loader
            // may say so in its own words.
        } catch {
            state = .failed(error)
        }
    }

    func apply(_ appearance: LusterAppearance) async {
        self.appearance = appearance
        await scene?.apply(appearance)
    }

    /// What a placeholder for `source` and `options` is told: their failure
    /// once they have failed, and `.minting` until then, before their load
    /// has begun too.
    func placeholderState(for source: LusterSource?, options: LusterOptions) -> LusterState {
        if case .failed = state, let loaded, loaded.source == source, loaded.options == options {
            return state
        }
        return .minting
    }

    private func show(_ scene: LusterScene?) {
        self.scene?.root.removeFromParent()
        self.scene = scene
        showing = scene != nil
        guard let scene else { return }
        scene.pose = pose
        scene.aspectRatio = aspectRatio
        scene.look = look
        root.addChild(scene.root)
    }

    // MARK: Interaction

    func dragChanged(_ translation: CGSize) {
        touching = true
        guard !pinched else { return }
        if !dragging {
            dragging = true
            velocity = 0
            lastTranslation = .zero
        }
        let dx = Float(translation.width - lastTranslation.width)
        let dy = Float(translation.height - lastTranslation.height)
        lastTranslation = translation
        pose.y += dx * Self.spinPerPoint
        // Screen y grows downward: dragging down tips the badge's top toward
        // the viewer.
        pose.x = min(Self.tiltLimit, max(-Self.tiltLimit, pose.x + dy * Self.tiltPerPoint))
        scene?.pose = pose
    }

    func dragEnded(velocity: CGSize) {
        dragging = false
        self.velocity = momentumEnabled && !pinched ? Float(velocity.width) * Self.spinPerPoint : 0
        pinched = false
        touching = false
    }

    // MARK: Bringing it closer

    /// Two fingers or a trackpad pinch at `location`, points in the view.
    /// The drag gives way to it, without a flick.
    func pinchBegan(at location: CGPoint) {
        guard zoomEnabled, scene != nil else { return }
        pinched = true
        dragging = false
        velocity = 0
        pinch = (look.zoom, look.under(fraction(location)))
    }

    /// The pinch is `scale` times its spread as it began, between the
    /// fingers at `location`: the point of the badge that was there stays
    /// there.
    func pinchChanged(scale: CGFloat, at location: CGPoint) {
        guard let pinch else { return }
        see(
            LusterHandling.look(
                pinch.zoom * Float(scale), point: pinch.point, at: fraction(location)))
    }

    /// The same, from a gesture that gives no beginning of its own.
    func pinch(scale: CGFloat, at location: CGPoint) {
        if pinch == nil { pinchBegan(at: location) }
        pinchChanged(scale: scale, at: location)
    }

    func pinchEnded() {
        pinch = nil
        // A trackpad's pinch comes with no drag to end it.
        if !touching { pinched = false }
    }

    /// Moves the badge brought closer by `delta` points, as a trackpad's
    /// two fingers slide; false at its size, where the slide is not its.
    func slide(by delta: CGSize) -> Bool {
        guard zoomEnabled, scene != nil, look.zoom > 1 else { return false }
        let frame = SIMD2(Float(size.width), Float(size.height)) * look.zoom
        let middle = SIMD2(look.x, look.y) - SIMD2(Float(delta.width), Float(delta.height)) / frame
        see(LusterHandling.look(look.zoom, point: middle, at: .zero))
        return true
    }

    /// Back at its size, in the middle: a double tap.
    func putBack() {
        see(.atRest)
    }

    private func see(_ look: LusterLook) {
        self.look = look
        scene?.look = look
    }

    /// A point in the view, in fractions of it from its middle.
    private func fraction(_ location: CGPoint) -> SIMD2<Float> {
        SIMD2(
            Float(location.x / max(size.width, 1)) - 0.5,
            Float(location.y / max(size.height, 1)) - 0.5)
    }

    /// Advances momentum by `dt` seconds; called every frame.
    func step(_ dt: TimeInterval) {
        guard !dragging, velocity != 0 else { return }
        let coast = LusterHandling.coast(velocity, dt: Float(dt))
        pose.y += coast.turn
        velocity = coast.velocity
        scene?.pose = pose
    }
}
