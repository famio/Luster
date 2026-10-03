import AppKit
import CoreGraphics
import Foundation
import Luster
import Metal
import RealityKit

/// `LusterShot --tone-table out.bin [--size 17] [--range 4]`
///
/// Measures RealityKit's tone mapping as a 3D table, for flutter_scene to
/// show what RealityKit would (`style::REALITYKIT_TONE`). For each point of
/// a size³ grid over sRGB-encoded [0, 1], red fastest, then green, then
/// blue, the linear colour times `range` is drawn as an emissive patch, and
/// what comes out is written sRGB-encoded, a byte a channel.
///
/// Every patch of a blue slice is drawn in one frame, side by side; a patch
/// reads the same alone as among the others.
@MainActor
func writeToneTable(to path: String, size n: Int, range: Float) async throws {
    let cell = 8, side = n * cell
    let renderer = try RealityRenderer()
    let camera = PerspectiveCamera()
    // Narrow and far: as good as orthographic, so every patch is seen head on.
    camera.camera.fieldOfViewInDegrees = 2
    camera.camera.far = 10_000
    let width = Float(n)
    camera.position = [0, 0, width / 2 / tan(Float.pi / 180)]
    renderer.entities.append(camera)
    renderer.activeCamera = camera
    renderer.cameraSettings.colorBackground = .color(CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 1))

    guard let device = MTLCreateSystemDefaultDevice() else { throw LusterSnapshot.Failure.noRenderer }
    // Float, so what RealityKit writes is read before anything rounds it.
    let descriptor = MTLTextureDescriptor.texture2DDescriptor(
        pixelFormat: .rgba16Float, width: side, height: side, mipmapped: false)
    descriptor.usage = [.renderTarget, .shaderRead]
    descriptor.storageMode = .shared
    guard let texture = device.makeTexture(descriptor: descriptor) else {
        throw LusterSnapshot.Failure.noRenderer
    }

    let mesh = MeshResource.generatePlane(width: 0.75, height: 0.75)
    var patches: [ModelEntity] = []
    for g in 0..<n {
        for r in 0..<n {
            let patch = ModelEntity(mesh: mesh)
            patch.position = [Float(r) - width / 2 + 0.5, width / 2 - 0.5 - Float(g), 0]
            renderer.entities.append(patch)
            patches.append(patch)
        }
    }

    func decode(_ e: Float) -> Float { e <= 0.04045 ? e / 12.92 : pow((e + 0.055) / 1.055, 2.4) }
    func encode(_ c: Float) -> Float {
        let c = min(max(c, 0), 1)
        return c <= 0.0031308 ? c * 12.92 : 1.055 * pow(c, 1 / 2.4) - 0.055
    }
    let linear = CGColorSpace(name: CGColorSpace.linearSRGB)!
    var table = [UInt8](repeating: 0, count: n * n * n * 3)
    for b in 0..<n {
        for g in 0..<n {
            for r in 0..<n {
                let x = SIMD3<Float>(decode(Float(r) / Float(n - 1)), decode(Float(g) / Float(n - 1)),
                                     decode(Float(b) / Float(n - 1))) * range
                // Black, lit by nothing but itself: what shows is the
                // emission, tone mapped.
                var material = PhysicallyBasedMaterial()
                material.baseColor = .init(tint: .black)
                material.metallic = .init(floatLiteral: 0)
                material.roughness = .init(floatLiteral: 1)
                material.specular = .init(floatLiteral: 0)
                let peak = max(x.x, max(x.y, x.z))
                let hue = peak > 0 ? x / peak : .zero
                let color = CGColor(colorSpace: linear,
                                    components: [CGFloat(hue.x), CGFloat(hue.y), CGFloat(hue.z), 1])!
                material.emissiveColor = .init(color: NSColor(cgColor: color)!)
                material.emissiveIntensity = peak
                patches[g * n + r].model?.materials = [material]
            }
        }
        let output = try RealityRenderer.CameraOutput(.singleProjection(colorTexture: texture))
        // The first frame after the materials change can still show the old ones.
        for _ in 0..<2 {
            try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, any Error>) in
                do {
                    try renderer.updateAndRender(deltaTime: 1 / 60, cameraOutput: output,
                                                 onComplete: { _ in done.resume() })
                } catch {
                    done.resume(throwing: error)
                }
            }
        }
        var pixels = [UInt16](repeating: 0, count: side * side * 4)
        texture.getBytes(&pixels, bytesPerRow: side * 8,
                         from: MTLRegionMake2D(0, 0, side, side), mipmapLevel: 0)
        for g in 0..<n {
            for r in 0..<n {
                let at = ((g * cell + cell / 2) * side + r * cell + cell / 2) * 4
                for channel in 0..<3 {
                    let shown = encode(Float(Float16(bitPattern: pixels[at + channel])))
                    table[((b * n + g) * n + r) * 3 + channel] = UInt8((shown * 255).rounded())
                }
            }
        }
    }
    try Data(table).write(to: URL(fileURLWithPath: path), options: .atomic)
}
