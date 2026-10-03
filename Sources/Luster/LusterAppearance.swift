internal import LusterCore

/// An sRGB color.
public struct LusterColor: Sendable, Hashable {
    public var red: Float
    public var green: Float
    public var blue: Float

    public init(red: Float, green: Float, blue: Float) {
        self.red = red
        self.green = green
        self.blue = blue
    }

    /// The engine's default metal: pale gold.
    public static let gold = LusterColor(LusterCore.defaultGold())
    public static let silver = LusterColor(LusterCore.silver())
    public static let copper = LusterColor(LusterCore.copper())

    init(_ rgba: LusterCore.Rgba) {
        self.init(red: rgba.r, green: rgba.g, blue: rgba.b)
    }

    var core: LusterCore.Rgba { .init(r: red, g: green, b: blue, a: 1) }
}

/// The raw value is the lighting's name, for a command line or a setting.
public enum LusterLighting: String, Sendable, Hashable, CaseIterable {
    /// A product photographer's studio: strip lights that sweep across the
    /// enamel as a band of sheen when the badge turns, over a dim tent that
    /// keeps plated lines their colour at any angle, and no lamps. The
    /// enamel shows the colours the document paints.
    case showcase
    /// For checking a badge's shape: lamps alone, no reflections, and nothing
    /// that shines.
    case off

    var core: LusterCore.Lighting {
        switch self {
        case .showcase: .showcase
        case .off: .off
        }
    }
}

/// What to strike. Changing any of it rebuilds the badge.
///
/// A badge is the art's silhouette in metal, its fills set into it as enamel
/// that runs on to the edge, recessed below the metal its lines stand in.
///
/// The metal's colour is not here: it changes no part of the badge, only how
/// it is plated, and belongs to `LusterAppearance` (and to `LusterBadge.glb`,
/// for a file).
public struct LusterOptions: Sendable, Hashable {
    /// Leave out the faces no view can see. Worth it for a badge being sent
    /// somewhere, not for one on screen: the search costs more than the
    /// triangles do.
    public var withoutHiddenFaces: Bool
    /// Leave the lines and the edge's top in the metal. By default they are
    /// plated in the colours the document paints on them, each line in its
    /// stroke colour.
    public var metalLines: Bool

    public init(metalLines: Bool = false, withoutHiddenFaces: Bool = false) {
        self.metalLines = metalLines
        self.withoutHiddenFaces = withoutHiddenFaces
    }
}

/// How a badge is shown. Changing it never rebuilds the badge's geometry.
public struct LusterAppearance: Sendable, Hashable {
    public var metal: LusterColor
    public var lighting: LusterLighting

    public init(metal: LusterColor = .gold, lighting: LusterLighting = .showcase) {
        self.metal = metal
        self.lighting = lighting
    }
}
