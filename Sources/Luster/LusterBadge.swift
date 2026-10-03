import Foundation
internal import LusterCore

/// A struck badge: the meshes and materials a renderer needs, and the bytes to
/// send one somewhere else. Two are equal when their `designKey`s are.
public final class LusterBadge: Sendable, Hashable {
    let core: LusterCore.LusterBadge

    /// Identifies the badge: the document and the options it was struck with.
    /// Equal keys mean the same badge.
    public let designKey: String

    init(_ core: LusterCore.LusterBadge) {
        self.core = core
        self.designKey = core.designKey()
    }

    /// The badge as a glTF binary: meshes, materials and textures in one file,
    /// its metal plated in `metal`, `scale` badge-widths across.
    ///
    /// The same badge always writes the same bytes, so two exports can be
    /// compared directly. Runs off the calling actor.
    @concurrent
    public func glb(scale: Float = 1, metal: LusterColor = .gold) async -> Data {
        Data(core.glb(scale: scale, metal: metal.core))
    }

    public static func == (a: LusterBadge, b: LusterBadge) -> Bool { a.designKey == b.designKey }

    public func hash(into hasher: inout Hasher) { hasher.combine(designKey) }
}
