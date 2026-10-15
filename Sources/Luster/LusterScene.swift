import Foundation
internal import LusterCore
import RealityKit

#if canImport(UIKit)
import UIKit
typealias PlatformColor = UIColor
#else
import AppKit
typealias PlatformColor = NSColor
#endif

/// Times the stages of building a scene and writes them to standard error,
/// when `LUSTER_TIME` is set. A way to see where a slow badge spends itself
/// without attaching Instruments.
struct Stopwatch {
    private let on = ProcessInfo.processInfo.environment["LUSTER_TIME"] != nil
    private let start = ContinuousClock.now
    private let last = Mark()

    private final class Mark: @unchecked Sendable {
        var at = ContinuousClock.now
    }

    func lap(_ stage: String) {
        guard on else { return }
        let now = ContinuousClock.now
        let since = { (then: ContinuousClock.Instant) in
            let d = now - then
            return Double(d.components.seconds) * 1000 + Double(d.components.attoseconds) / 1e15
        }
        let line = String(
            format: "  %@%@%6.0f ms (%6.0f)\n", stage,
            String(repeating: " ", count: max(1, 16 - stage.count)),
            since(last.at), since(start))
        FileHandle.standardError.write(Data(line.utf8))
        last.at = now
    }
}

/// A badge set up in the studio for RealityKit: the model, its lights, the
/// image-based light and a camera. Add `root` to a `RealityView`'s content.
///
/// Building the mesh happens off the main actor; only entity assembly runs on
/// it. Appearance changes re-light and re-plate without rebuilding.
@MainActor
public final class LusterScene {
    public let root = Entity()
    /// Turns the badge; the lights and camera stay put.
    let pivot = Entity()
    public let model: ModelEntity
    public let camera: PerspectiveCamera
    private let imageBasedLight = Entity()
    private var lights: [LusterCore.LightRole: DirectionalLight] = [:]
    private let materials: [LusterCore.Material]
    public let badge: LusterBadge
    public private(set) var appearance: LusterAppearance
    /// What the badge measures across its face, wide by tall.
    private var size = SIMD2<Float>(1, 1)

    /// Slightly off-axis, so the edge shows depth: the pose a still is taken
    /// at, and the one a view opens on.
    public static let restingPose: SIMD2<Float> = {
        let handling = LusterCore.handling()
        return SIMD2(handling.restingTilt, handling.restingSpin)
    }()

    /// Tilt (about x) and spin (about y), radians.
    public var pose = LusterScene.restingPose {
        didSet { pivot.orientation = Self.orientation(pose) }
    }

    /// Width over height of the view showing the scene. The field of view is
    /// vertical; a view too narrow to hold the badge at that framing is
    /// widened until it fits, so a portrait phone shows the whole badge.
    public var aspectRatio: Float = 1 {
        didSet { frame() }
    }

    /// How much closer the view has brought the badge, and where it looks.
    package var look = LusterLook.atRest {
        didSet { if look != oldValue { frame() } }
    }

    private func frame() {
        let fits = LusterCore.fieldOfView(
            width: size.x, height: size.y,
            aspect: aspectRatio)
        let half = tan(fits * .pi / 360)
        camera.camera.fieldOfViewOrientation = .vertical
        camera.camera.fieldOfViewInDegrees = atan(half / look.zoom) * 360 / .pi
        // A point off the middle is looked at by turning the camera where it
        // stands, as the other platforms shift their lens: what each point of
        // the badge reflects depends on where the camera is, so the light
        // and its reflections stay as they are.
        let across = 2 * look.x * half * aspectRatio
        let up = -2 * look.y * half
        let turn =
            simd_quatf(angle: -atan(across), axis: [0, 1, 0])
            * simd_quatf(angle: atan2(up, (1 + across * across).squareRoot()), axis: [1, 0, 0])
        camera.orientation = turn
        // The headlight rides on the camera, and keeps pointing as it did.
        if let headlight = lights[.headlight] {
            headlight.orientation = turn.inverse * headlightOrientation
        }
    }

    /// The headlight's own turn, before the camera's.
    private var headlightOrientation = simd_quatf(ix: 0, iy: 0, iz: 0, r: 1)

    public static func make(_ badge: LusterBadge, appearance: LusterAppearance) async throws
        -> LusterScene
    {
        let clock = Stopwatch()
        // None of these waits on another: the mesh is built off the main
        // actor while the materials are made on it, and the studio's shared
        // resources are ready at once unless this is the first badge.
        async let mesh = Self.mesh(for: badge)
        async let environment = StudioResources.environment()
        async let grit = StudioResources.sandblast()
        let materials = badge.core.materials()
        // The faces the document paints, and the reverse's sandblast.
        let images = await Self.images(for: badge)
        clock.lap("images \(images.count)")
        let textures = try await StudioResources.textures(images)
        clock.lap("textures")
        let sandblast = try await grit
        clock.lap("grit")
        try Task.checkCancellation()
        let built = await Self.materials(materials, appearance, textures: textures, grit: sandblast)
        clock.lap("materials \(materials.count)")
        let made = try await mesh
        clock.lap("mesh")
        let lit = try await environment
        clock.lap("environment")
        return LusterScene(
            badge: badge, mesh: made, materials: materials, built: built,
            environment: lit, appearance: appearance,
            textures: textures, grit: sandblast)
    }

    /// The badge's mesh, built off the main actor.
    @concurrent
    nonisolated static func mesh(for badge: LusterBadge) async throws -> MeshResource {
        try await MeshResource(from: descriptors(for: badge))
    }

    /// The badge's textures as images, made off the main actor.
    @concurrent
    nonisolated static func images(for badge: LusterBadge) async -> [CGImage] {
        badge.core.textures().compactMap(CGImage.srgb)
    }

    private init(
        badge: LusterBadge, mesh: MeshResource, materials: [LusterCore.Material],
        built: [PhysicallyBasedMaterial], environment: EnvironmentResource?,
        appearance: LusterAppearance, textures: [TextureResource] = [],
        grit: TextureResource? = nil
    ) {
        self.textures = textures
        self.grit = grit
        self.badge = badge
        self.appearance = appearance
        self.materials = materials
        model = ModelEntity(mesh: mesh, materials: built)
        camera = PerspectiveCamera()

        root.addChild(pivot)
        pivot.addChild(model)

        let setup = LusterCore.studioCamera()
        let bounds = model.visualBounds(relativeTo: nil).extents
        size = SIMD2(bounds.x, bounds.y)
        camera.camera.near = 0.1
        camera.position = [0, 0, setup.distance]
        root.addChild(camera)
        frame()

        for light in LusterCore.studioLights() {
            let entity = DirectionalLight()
            entity.light.color = PlatformColor(light.color)
            entity.orientation = Self.orientation(SIMD2(light.pitch, light.yaw))
            if light.role == .headlight {
                headlightOrientation = entity.orientation
                camera.addChild(entity)
            } else {
                root.addChild(entity)
            }
            lights[light.role] = entity
        }

        root.addChild(imageBasedLight)
        if let environment {
            imageBasedLight.components.set(ImageBasedLightComponent(source: .single(environment)))
        }
        model.components.set(ImageBasedLightReceiverComponent(imageBasedLight: imageBasedLight))

        pivot.orientation = Self.orientation(pose)
        relight()
    }

    /// The textures this badge's materials refer to, made once.
    private var textures: [TextureResource] = []
    private var grit: TextureResource?

    /// Re-plates and re-lights the badge; the mesh stays.
    public func apply(_ appearance: LusterAppearance) async {
        guard appearance != self.appearance else { return }
        let old = self.appearance
        self.appearance = appearance
        if appearance.metal != old.metal || appearance.lighting != old.lighting {
            let built = await Self.materials(materials, appearance, textures: textures, grit: grit)
            // A later call may have moved on while these were made.
            guard appearance == self.appearance else { return }
            model.model?.materials = built
        }
        relight()
    }

    private func relight() {
        let preset = LusterCore.lightingPreset(lighting: appearance.lighting.core)

        // The studio's own numbers, in the units this renderer works in.
        // The two overrides are for calibrating against another renderer.
        let environment = ProcessInfo.processInfo.environment
        let lux = environment["LUSTER_LUX"].flatMap(Float.init) ?? preset.lux
        let boost = environment["LUSTER_IBL"].flatMap(Float.init) ?? 0
        for (role, light) in lights {
            light.light.intensity =
                LusterCore.lamp(role: role, lighting: appearance.lighting.core) * lux
            // A lamp at nothing still costs every pixel its share of the
            // shading; the showcase has none lit.
            light.isEnabled = light.light.intensity > 0
        }

        if var ibl = imageBasedLight.components[ImageBasedLightComponent.self] {
            // The environment's intensity is a multiplier; RealityKit takes EV.
            ibl.intensityExponent = preset.iblExponent + boost
            imageBasedLight.components.set(ibl)
        }
    }

    /// RealityKit materials for the badge's.
    ///
    /// They must be made on the main thread: `PhysicallyBasedMaterial` loads
    /// engine resources that assert the main queue, although its API is not
    /// marked `@MainActor`. A copy costs nothing, but every change to a
    /// colour or a number in one costs about 2 ms, so each finish, and each
    /// colour of it, is set once and copied after; what is left is split
    /// into slices with a frame between them rather than done in one hitch.
    static func materials(
        _ materials: [LusterCore.Material],
        _ appearance: LusterAppearance,
        textures: [TextureResource] = [],
        grit: TextureResource? = nil
    ) async -> [PhysicallyBasedMaterial] {
        var built: [PhysicallyBasedMaterial] = []
        var slice = ContinuousClock.now
        for m in materials {
            if ContinuousClock.now - slice > sliceBudget {
                try? await Task.sleep(for: .milliseconds(4))
                slice = ContinuousClock.now
            }
            // The engine says how each material is finished; only the
            // metal's colour is this side's.
            let finish = LusterCore.finish(role: m.role, lighting: appearance.lighting.core)
            var pbr: PhysicallyBasedMaterial
            if let index = m.texture, Int(index) < textures.count {
                // What the document paints on this face, where its colour is
                // not flat. Its tint is the prototype's own white: no change.
                pbr = finished(finish)
                pbr.baseColor = .init(tint: .white, texture: .init(textures[Int(index)]))
            } else {
                pbr = tinted(finish, finish.plated ? appearance.metal.core : m.color)
            }
            if m.role == .goldBack, let grit {
                pbr.normal = .init(texture: .init(grit, sampler: StudioResources.repeating))
            }
            built.append(pbr)
        }
        return built
    }

    /// Main-thread work allowed between frames.
    private static let sliceBudget = Duration.milliseconds(6)

    /// The first `PhysicallyBasedMaterial` costs about 20 ms; copies of it are
    /// cheap. `prewarm()` makes it before any badge needs it.
    private static let prototype = PhysicallyBasedMaterial()

    /// Each finish, set on a copy of the prototype.
    private static var finishes: [LusterCore.Finish: PhysicallyBasedMaterial] = [:]
    /// Each finish in each colour it has been asked for lately.
    private static var tints: [Tint: PhysicallyBasedMaterial] = [:]

    private struct Tint: Hashable {
        let finish: LusterCore.Finish
        let color: LusterCore.Rgba
    }

    private static func finished(_ finish: LusterCore.Finish) -> PhysicallyBasedMaterial {
        if let made = finishes[finish] { return made }
        var pbr = prototype
        pbr.metallic = .init(floatLiteral: finish.metallic)
        pbr.roughness = .init(floatLiteral: finish.roughness)
        // RealityKit's specular is half of glass's reflectance at 0.5, its
        // default; the engine's is a part of glass's.
        pbr.specular = .init(floatLiteral: 0.5 * finish.specular)
        finishes[finish] = pbr
        return pbr
    }

    private static func tinted(
        _ finish: LusterCore.Finish,
        _ color: LusterCore.Rgba
    ) -> PhysicallyBasedMaterial {
        let key = Tint(finish: finish, color: color)
        if let made = tints[key] { return made }
        var pbr = finished(finish)
        pbr.baseColor = .init(tint: PlatformColor(color))
        // Enough for the badges on screen at once; a long run of them starts
        // afresh rather than keep every colour it has shown.
        if tints.count >= 64 { tints.removeAll() }
        tints[key] = pbr
        return pbr
    }

    /// Makes the shared RealityKit resources ahead of the first badge.
    static func prewarm() async {
        _ = prototype
        async let environment = StudioResources.environment()
        async let grit = StudioResources.sandblast()
        async let mesh: Void = StudioResources.warmUp()
        _ = try? await (environment, grit, mesh)
    }

    /// Tilt about x, then spin about the badge's own vertical axis: the order
    /// the light rig is written in.
    static func orientation(_ angles: SIMD2<Float>) -> simd_quatf {
        simd_quatf(angle: angles.x, axis: [1, 0, 0]) * simd_quatf(angle: angles.y, axis: [0, 1, 0])
    }

    /// Unpacks the engine's interleaved vertices. Runs off the main actor.
    @concurrent
    nonisolated static func descriptors(for badge: LusterBadge) async -> [MeshDescriptor] {
        let layout = LusterCore.vertexLayout()
        let stride = Int(layout.stride)
        let (position, normal, uv) = (Int(layout.position), Int(layout.normal), Int(layout.uv))
        return badge.core.submeshes().map { submesh in
            let count = Int(submesh.vertexCount)
            var positions = [SIMD3<Float>]()
            var normals = [SIMD3<Float>]()
            var uvs = [SIMD2<Float>]()
            positions.reserveCapacity(count)
            normals.reserveCapacity(count)
            uvs.reserveCapacity(count)
            submesh.vertices.withUnsafeBytes { raw in
                func f(_ offset: Int) -> Float {
                    raw.loadUnaligned(fromByteOffset: offset, as: Float.self)
                }
                for i in 0..<count {
                    let o = i * stride
                    positions.append([f(o + position), f(o + position + 4), f(o + position + 8)])
                    normals.append([f(o + normal), f(o + normal + 4), f(o + normal + 8)])
                    // The engine's v runs down the image; RealityKit's runs up.
                    uvs.append([f(o + uv), 1 - f(o + uv + 4)])
                }
            }
            let indices = submesh.indices.withUnsafeBytes { raw in
                (0..<Int(submesh.indexCount)).map {
                    UInt32(littleEndian: raw.loadUnaligned(fromByteOffset: $0 * 4, as: UInt32.self))
                }
            }
            var descriptor = MeshDescriptor(name: submesh.name)
            descriptor.positions = MeshBuffers.Positions(positions)
            descriptor.normals = MeshBuffers.Normals(normals)
            descriptor.textureCoordinates = MeshBuffers.TextureCoordinates(uvs)
            descriptor.primitives = .triangles(indices)
            descriptor.materials = .allFaces(submesh.material)
            return descriptor
        }
    }
}

/// The studio's shared resources, made once.
@MainActor
enum StudioResources {
    private static let lighting = Kept(makeEnvironment)
    /// How many texels across each face of the cube the panorama is
    /// reflected from. The showcase's strips are crossed by glossy enamel,
    /// whose sheen has an edge only as sharp as this; at 256 it goes soft.
    private static let faceSize = 512

    /// The badge's own textures, uploaded once per badge.
    static func textures(_ images: [CGImage]) async throws -> [TextureResource] {
        var made: [TextureResource] = []
        for image in images {
            made.append(try await TextureResource(image: image, options: .init(semantic: .color)))
        }
        return made
    }

    /// The reverse repeats its grit across the badge, and sees it small
    /// enough to want the mipmaps.
    static let repeating: MaterialParameters.Texture.Sampler = {
        let descriptor = MTLSamplerDescriptor()
        descriptor.sAddressMode = .repeat
        descriptor.tAddressMode = .repeat
        descriptor.minFilter = .linear
        descriptor.magFilter = .linear
        descriptor.mipFilter = .linear
        return .init(descriptor)
    }()

    /// The sandblast on a badge's reverse.
    private static let grit = Kept(makeSandblast)
    /// RealityKit's first mesh, which takes it ten times as long as any after.
    private static let firstMesh = Kept(makeFirstMesh)

    /// None where the engine's pixels would not make an image; asked
    /// again, it tries again.
    static func sandblast() async throws -> TextureResource? {
        do {
            return try await grit.value()
        } catch is Unmade {
            return nil
        }
    }

    static func environment() async throws -> EnvironmentResource? {
        do {
            return try await lighting.value()
        } catch is Unmade {
            return nil
        }
    }

    /// The engine's pixels would not make an image.
    private struct Unmade: Error {}

    /// Builds a single triangle, which takes on the first mesh's cost.
    static func warmUp() async throws {
        _ = try await firstMesh.value()
    }

    private static func makeEnvironment() async throws -> EnvironmentResource {
        guard let image = await Studio.shared.environmentImage() else { throw Unmade() }
        // The async initializers keep the cube conversion and prefiltering
        // from blocking the main actor.
        let cube = try await TextureResource(
            cubeFromEquirectangular: image, named: nil,
            quality: .normal, faceSize: faceSize,
            options: .init(semantic: .color))
        return try await EnvironmentResource(cube: cube, options: .init(samplingQuality: .normal))
    }

    private static func makeSandblast() async throws -> TextureResource {
        guard let image = await Studio.shared.gritImage() else { throw Unmade() }
        // A normal map is data, not colour: it must not be read as sRGB.
        return try await TextureResource(image: image, options: .init(semantic: .normal))
    }

    @concurrent
    private nonisolated static func makeFirstMesh() async throws -> Bool {
        var triangle = MeshDescriptor(name: "warm-up")
        triangle.positions = MeshBuffers.Positions([[0, 0, 0], [1, 0, 0], [0, 1, 0]])
        triangle.normals = MeshBuffers.Normals([[0, 0, 1], [0, 0, 1], [0, 0, 1]])
        triangle.textureCoordinates = MeshBuffers.TextureCoordinates([[0, 0], [1, 0], [0, 1]])
        triangle.primitives = .triangles([0, 1, 2])
        _ = try await MeshResource(from: [triangle])
        return true
    }
}

/// Something made once for the process. Whoever asks while it is being made
/// waits for it rather than making another; a failure is tried again by the
/// next to ask.
@MainActor
final class Kept<Value: Sendable> {
    private let make: @MainActor () async throws -> Value
    private var making: Task<Value, any Error>?

    init(_ make: @escaping @MainActor () async throws -> Value) {
        self.make = make
    }

    func value() async throws -> Value {
        let task = making ?? Task { try await make() }
        making = task
        do {
            return try await task.value
        } catch {
            if making == task { making = nil }
            throw error
        }
    }
}

extension PlatformColor {
    convenience init(_ c: LusterCore.Rgba) {
        #if canImport(UIKit)
        self.init(
            red: CGFloat(c.r), green: CGFloat(c.g), blue: CGFloat(c.b), alpha: CGFloat(c.a))
        #else
        self.init(
            srgbRed: CGFloat(c.r), green: CGFloat(c.g), blue: CGFloat(c.b), alpha: CGFloat(c.a))
        #endif
    }
}
