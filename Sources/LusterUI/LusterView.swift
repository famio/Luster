import Luster
import RealityKit
import SwiftUI

/// A badge minted from an SVG, lit in the studio. Drag to turn it; it keeps
/// spinning briefly with momentum.
///
/// Minting runs off the main actor; while it runs the previous badge stays on
/// screen. Changing `source` cancels a mint in flight. Changing `appearance`
/// only re-lights and re-plates.
public struct LusterView: View {
    public var source: LusterSource?
    public var options: LusterOptions
    public var appearance: LusterAppearance
    /// Whether a flick keeps the badge turning. A flick never does while the
    /// system asks for reduced motion, whatever this says.
    public var momentumEnabled: Bool
    public var onStateChange: (@MainActor (LusterState) -> Void)?

    @State private var stage = LusterStage()
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    public init(source: LusterSource?, options: LusterOptions = .init(),
                appearance: LusterAppearance = .init(),
                momentumEnabled: Bool = true,
                onStateChange: (@MainActor (LusterState) -> Void)? = nil) {
        self.source = source
        self.options = options
        self.appearance = appearance
        self.momentumEnabled = momentumEnabled
        self.onStateChange = onStateChange
    }

    public var body: some View {
        RealityView { content in
            // The studio's values are set against a standard range; letting
            // the view stretch them would light the badge differently from
            // every still taken of it.
            content.renderingEffects.dynamicRange = .standard
            content.add(stage.root)
            let stage = stage
            stage.updates = content.subscribe(to: SceneEvents.Update.self) { event in
                MainActor.assumeIsolated { stage.step(event.deltaTime) }
            }
        }
        .onGeometryChange(for: Float.self) { geometry in
            Float(geometry.size.width / max(geometry.size.height, 1))
        } action: { aspect in
            stage.aspectRatio = aspect
        }
        .gesture(
            DragGesture(minimumDistance: 0)
                .onChanged { stage.dragChanged($0.translation) }
                .onEnded { stage.dragEnded(velocity: $0.velocity) }
        )
        .task(id: Pair(source, options)) {
            await stage.load(source, options: options, appearance: appearance)
        }
        .task(id: appearance) {
            await stage.apply(appearance)
        }
        .onChange(of: !reduceMotion && momentumEnabled, initial: true) { _, momentum in
            stage.momentumEnabled = momentum
        }
        .onChange(of: stage.change) {
            onStateChange?(stage.state)
        }
        .accessibilityElement()
        .accessibilityLabel(Text("Badge"))
    }
}

/// Two values SwiftUI can watch together.
private struct Pair<A: Hashable & Sendable, B: Hashable & Sendable>: Hashable, Sendable {
    let a: A
    let b: B

    init(_ a: A, _ b: B) {
        self.a = a
        self.b = b
    }
}

extension Color {
    /// A colour from the engine's palette, in sRGB.
    public init(_ c: LusterColor) {
        self.init(.sRGB, red: Double(c.red), green: Double(c.green), blue: Double(c.blue))
    }
}
