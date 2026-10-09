import Luster
import RealityKit
import SwiftUI

/// A badge minted from an SVG, lit in the studio. Drag to turn it; it keeps
/// spinning briefly with momentum.
///
/// Minting runs off the main actor; while it runs the previous badge stays on
/// screen. Changing `source` cancels a mint in flight. Changing `appearance`
/// only re-lights and re-plates.
///
/// Until a badge is on screen the view shows its placeholder, if it is given
/// one:
///
/// ```swift
/// LusterView(source: .url(url)) { state in
///     if case .failed = state { Image(systemName: "exclamationmark.triangle") }
///     else { ProgressView() }
/// }
/// ```
public struct LusterView: View {
    public var source: LusterSource?
    public var options: LusterOptions
    public var appearance: LusterAppearance
    /// Whether a flick keeps the badge turning. A flick never does while the
    /// system asks for reduced motion, whatever this says.
    public var momentumEnabled: Bool
    public var onStateChange: (@MainActor (LusterState) -> Void)?
    /// Erased, so that the view's type is the same with a placeholder or
    /// without, whatever the placeholder is.
    let placeholder: ((LusterState) -> AnyView)?
    /// Told whether a placeholder is due, for `LusterNSView`, which shows its
    /// own over the view.
    var onPlaceholderChange: (@MainActor (Bool) -> Void)?

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
        placeholder = nil
    }

    /// `placeholder` is shown over the view, in the middle, while a source is
    /// set and no badge is on screen: while the first is struck, given
    /// `.minting`, and if it fails, given `.failed`. A badge struck later
    /// takes the place of the one on screen without it.
    public init<Placeholder: View>(
        source: LusterSource?, options: LusterOptions = .init(),
        appearance: LusterAppearance = .init(),
        momentumEnabled: Bool = true,
        onStateChange: (@MainActor (LusterState) -> Void)? = nil,
        @ViewBuilder placeholder: @escaping (LusterState) -> Placeholder
    ) {
        self.source = source
        self.options = options
        self.appearance = appearance
        self.momentumEnabled = momentumEnabled
        self.onStateChange = onStateChange
        self.placeholder = { AnyView(placeholder($0)) }
    }

    /// Whether a source is set and no badge is on screen.
    private var placeholderDue: Bool { source != nil && !stage.showing }

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
        .onChange(of: placeholderDue, initial: true) { _, due in
            onPlaceholderChange?(due)
        }
        .accessibilityElement()
        .accessibilityLabel(Text("Badge"))
        // Outside the badge's element, so that what it says is heard.
        .overlay {
            if let placeholder {
                ZStack {
                    if placeholderDue {
                        placeholder(stage.placeholderState(for: source, options: options))
                            // There from the first frame, and fading as the
                            // badge arrives: the fade covers the frame or two
                            // a badge can take to be drawn.
                            .transition(.asymmetric(insertion: .identity, removal: .opacity))
                    }
                }
                .animation(.easeOut(duration: 0.2), value: placeholderDue)
            }
        }
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
