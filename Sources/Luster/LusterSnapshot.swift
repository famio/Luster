import CoreGraphics
import Foundation
import Metal
import RealityKit

/// Renders a badge to a still image, without a view.
///
/// For thumbnails in a list, where a live view of each badge would cost far
/// more than the picture is worth, and for checking by eye what the engine
/// builds.
@MainActor
public enum LusterSnapshot {

    /// A square still of the badge at `pose` (tilt, spin; radians), over
    /// `background`, or with nothing behind it when that is nil: the image's
    /// alpha is then the badge's coverage.
    public static func image(_ source: LusterSource,
                             options: LusterOptions = .init(),
                             appearance: LusterAppearance = .init(),
                             pose: SIMD2<Float> = LusterScene.restingPose,
                             pixelSize: Int = 768,
                             background: LusterColor? = nil) async throws -> CGImage {
        let badge = try await LusterEngine.mint(source, options: options)
        return try await image(badge, appearance: appearance, pose: pose, pixelSize: pixelSize,
                               background: background)
    }

    public static func image(_ badge: LusterBadge,
                             appearance: LusterAppearance = .init(),
                             pose: SIMD2<Float> = LusterScene.restingPose,
                             pixelSize: Int = 768,
                             background: LusterColor? = nil) async throws -> CGImage {
        let scene = try await LusterScene.make(badge, appearance: appearance)
        scene.pose = pose
        return try await image(scene, pixelSize: pixelSize, background: background)
    }

    /// Renders a scene that is already set up, lit as it is in a view: a
    /// still is the picture the view shows.
    public static func image(_ scene: LusterScene, pixelSize: Int = 768,
                             background: LusterColor? = nil) async throws -> CGImage {
        let renderer = try RealityRenderer()
        renderer.entities.append(scene.root)
        renderer.activeCamera = scene.camera
        renderer.cameraSettings.colorBackground = .color(
            background?.cgColor ?? CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 0))
        // Tone mapping stays on: a live RealityView tone maps too, and a
        // still should look like what the view shows.

        guard let device = MTLCreateSystemDefaultDevice() else { throw Failure.noRenderer }
        // The renderer writes light, which a view encodes as sRGB on its way
        // to the screen; so does an sRGB texture. Into a plain one it would
        // go unencoded, and a still would come out darker, harder and more
        // saturated than the view.
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: pixelFormat, width: pixelSize, height: pixelSize, mipmapped: false)
        descriptor.usage = [.renderTarget, .shaderRead]
        descriptor.storageMode = .shared
        guard let texture = device.makeTexture(descriptor: descriptor) else {
            throw Failure.noRenderer
        }

        let output = try RealityRenderer.CameraOutput(.singleProjection(colorTexture: texture))
        // Wait for the render to finish, not merely to be scheduled: the
        // texture is read back on the CPU. The main actor is free meanwhile.
        let done = Once()
        // The renderer has to outlive the render it is waiting on.
        defer { withExtendedLifetime(renderer) {} }
        try await withCheckedThrowingContinuation { (finished: CheckedContinuation<Void, any Error>) in
            do {
                try renderer.updateAndRender(deltaTime: 0, cameraOutput: output,
                                             onComplete: { _ in
                                                 if done.claim() { finished.resume() }
                                             })
            } catch {
                if done.claim() { finished.resume(throwing: error) }
                return
            }
            Task {
                try? await Task.sleep(for: .seconds(10))
                if done.claim() { finished.resume(throwing: Failure.noRenderer) }
            }
        }
        return try read(texture)
    }

    /// Lets exactly one of several racing callers through.
    private final class Once: @unchecked Sendable {
        private let lock = NSLock()
        private var claimed = false

        func claim() -> Bool {
            lock.withLock {
                defer { claimed = true }
                return !claimed
            }
        }
    }

    public enum Failure: Error {
        /// No Metal device, or the renderer could not be set up.
        case noRenderer
    }

    /// What a still is rendered into: 8-bit sRGB, as a view is shown.
    static let pixelFormat = MTLPixelFormat.rgba8Unorm_srgb

    /// The texture's pixels as an image.
    private static func read(_ texture: any MTLTexture) throws -> CGImage {
        let width = texture.width, height = texture.height
        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        bytes.withUnsafeMutableBytes { raw in
            texture.getBytes(raw.baseAddress!, bytesPerRow: width * 4,
                             from: MTLRegionMake2D(0, 0, width, height), mipmapLevel: 0)
        }
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let provider = CGDataProvider(data: Data(bytes) as CFData),
              let image = CGImage(width: width, height: height, bitsPerComponent: 8,
                                  bitsPerPixel: 32, bytesPerRow: width * 4, space: space,
                                  bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                                  provider: provider, decode: nil, shouldInterpolate: true,
                                  intent: .defaultIntent)
        else { throw Failure.noRenderer }
        return image
    }
}

extension LusterColor {
    /// The colour as CoreGraphics sees it.
    public var cgColor: CGColor {
        CGColor(colorSpace: CGColorSpace(name: CGColorSpace.sRGB)!,
                components: [CGFloat(red), CGFloat(green), CGFloat(blue), 1])
            ?? CGColor(gray: 0, alpha: 1)
    }
}
