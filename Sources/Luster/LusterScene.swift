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
        let line = String(format: "  %@%@%6.0f ms (%6.0f)\n", stage,
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

    private func frame() {
        camera.camera.fieldOfViewOrientation = .vertical
        camera.camera.fieldOfViewInDegrees = LusterCore.fieldOfView(width: size.x, height: size.y,
                                                                    aspect: aspectRatio)
    }

    public static func make(_ badge: LusterBadge, appearance: LusterAppearance) async throws -> LusterScene {
        let clock = Stopwatch()
        let descriptors = await descriptors(for: badge)
        clock.lap("descriptors")
        let mesh = try await MeshResource(from: descriptors)
        clock.lap("mesh")
        let materials = badge.core.materials()
        // The faces the document paints, and the reverse's sandblast.
        let images = await Self.images(for: badge)
        clock.lap("images \(images.count)")
        let textures = try await StudioResources.textures(images)
        clock.lap("textures")
        let grit = try await StudioResources.sandblast()
        clock.lap("grit")
        let built = await Self.materials(materials, appearance, textures: textures, grit: grit)
        clock.lap("materials \(materials.count)")
        let environment = try await StudioResources.environment()
        clock.lap("environment")
        return LusterScene(badge: badge, mesh: mesh, materials: materials, built: built,
                           environment: environment, appearance: appearance,
                           textures: textures, grit: grit)
    }

    /// The badge's textures as images, made off the main actor.
    @concurrent
    nonisolated static func images(for badge: LusterBadge) async -> [CGImage] {
        badge.core.textures().compactMap(CGImage.srgb)
    }

    private init(badge: LusterBadge, mesh: MeshResource, materials: [LusterCore.Material],
                 built: [PhysicallyBasedMaterial], environment: EnvironmentResource?,
                 appearance: LusterAppearance, textures: [TextureResource] = [],
                 grit: TextureResource? = nil) {
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
            light.light.intensity = LusterCore.lamp(role: role, lighting: appearance.lighting.core) * lux
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
    /// marked `@MainActor`. Each costs about 2 ms (a copy of the prototype and
    /// its first write), so the work is split into slices with a frame
    /// between them rather than done in one hitch.
    static func materials(_ materials: [LusterCore.Material],
                          _ appearance: LusterAppearance,
                          textures: [TextureResource] = [],
                          grit: TextureResource? = nil) async -> [PhysicallyBasedMaterial] {
        var built: [PhysicallyBasedMaterial] = []
        var slice = ContinuousClock.now
        for m in materials {
            if ContinuousClock.now - slice > sliceBudget {
                try? await Task.sleep(for: .milliseconds(4))
                slice = ContinuousClock.now
            }
            var pbr = material(m, appearance)
            // What the document paints on this face, where its colour is not
            // flat; the material's own colour stands for its sides.
            if let index = m.texture, Int(index) < textures.count {
                pbr.baseColor = .init(tint: .white, texture: .init(textures[Int(index)]))
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

    /// Makes the shared RealityKit resources ahead of the first badge.
    static func prewarm() async {
        _ = prototype
        _ = try? await StudioResources.environment()
    }

    /// The engine says how each material is finished; only the metal's
    /// colour is this side's.
    private static func material(_ m: LusterCore.Material,
                                 _ appearance: LusterAppearance) -> PhysicallyBasedMaterial {
        let finish = LusterCore.finish(role: m.role, lighting: appearance.lighting.core)
        var pbr = prototype
        pbr.baseColor = .init(tint: PlatformColor(finish.plated ? appearance.metal.core : m.color))
        pbr.metallic = .init(floatLiteral: finish.metallic)
        pbr.roughness = .init(floatLiteral: finish.roughness)
        // RealityKit's specular is half of glass's reflectance at 0.5, its
        // default; the engine's is a part of glass's.
        pbr.specular = .init(floatLiteral: 0.5 * finish.specular)
        return pbr
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
            var positions = [SIMD3<Float>](), normals = [SIMD3<Float>](), uvs = [SIMD2<Float>]()
            positions.reserveCapacity(count)
            normals.reserveCapacity(count)
            uvs.reserveCapacity(count)
            submesh.vertices.withUnsafeBytes { raw in
                func f(_ offset: Int) -> Float { raw.loadUnaligned(fromByteOffset: offset, as: Float.self) }
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
    private static var environment: EnvironmentResource?
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

    /// The sandblast on a badge's reverse, made once for the process.
    private static var grit: TextureResource?

    static func sandblast() async throws -> TextureResource? {
        if let grit { return grit }
        guard let image = CGImage.srgb(LusterCore.sandblast()) else { return nil }
        // A normal map is data, not colour: it must not be read as sRGB.
        let made = try await TextureResource(image: image, options: .init(semantic: .normal))
        grit = made
        return made
    }

    static func environment() async throws -> EnvironmentResource? {
        if let environment { return environment }
        guard let image = await Studio.shared.environmentImage() else { return nil }
        // The async initializers keep the cube conversion and prefiltering
        // from blocking the main actor.
        let cube = try await TextureResource(cubeFromEquirectangular: image, named: nil,
                                             quality: .normal, faceSize: faceSize,
                                             options: .init(semantic: .color))
        let made = try await EnvironmentResource(cube: cube, options: .init(samplingQuality: .normal))
        environment = made
        return made
    }
}

extension PlatformColor {
    convenience init(_ c: LusterCore.Rgba) {
        #if canImport(UIKit)
        self.init(red: CGFloat(c.r), green: CGFloat(c.g), blue: CGFloat(c.b), alpha: CGFloat(c.a))
        #else
        self.init(srgbRed: CGFloat(c.r), green: CGFloat(c.g), blue: CGFloat(c.b), alpha: CGFloat(c.a))
        #endif
    }
}
