import Luster
import Observation
import RealityKit
import SwiftUI

/// The live state behind a badge view: the scene on stage, the pose and the
/// drag momentum.
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
    @ObservationIgnored var aspectRatio: Float = 1 {
        didSet { scene?.aspectRatio = aspectRatio }
    }

    private static let handling = LusterHandling.standard
    static let spinPerPoint = handling.spinPerPoint
    static let tiltPerPoint = handling.tiltPerPoint
    static let tiltLimit = handling.tiltLimit

    func load(_ source: LusterSource?, options: LusterOptions,
              appearance: LusterAppearance) async {
        guard let source else {
            show(nil)
            state = .idle
            return
        }
        self.appearance = appearance
        state = .minting
        do {
            let badge = try await LusterEngine.mint(source, options: options)
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
        } catch {
            state = .failed(error)
        }
    }

    func apply(_ appearance: LusterAppearance) async {
        self.appearance = appearance
        await scene?.apply(appearance)
    }

    private func show(_ scene: LusterScene?) {
        self.scene?.root.removeFromParent()
        self.scene = scene
        guard let scene else { return }
        scene.pose = pose
        scene.aspectRatio = aspectRatio
        root.addChild(scene.root)
    }

    // MARK: Interaction

    func dragChanged(_ translation: CGSize) {
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
        self.velocity = momentumEnabled ? Float(velocity.width) * Self.spinPerPoint : 0
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
