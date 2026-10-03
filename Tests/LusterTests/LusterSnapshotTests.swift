import CoreGraphics
import Foundation
import Metal
import RealityKit
import Testing
@testable import Luster

/// A still carries the light the renderer works out encoded as sRGB, as a
/// view shows it: the same scene rendered into floats and encoded by hand
/// gives the same bytes.
@MainActor
@Test func aStillIsEncodedAsAViewIs() async throws {
    let scene = try await LusterScene.make(try await sampleBadge(), appearance: .init())
    let size = 160
    let still = try await LusterSnapshot.image(scene, pixelSize: size, background: backdrop)
    let light = try await render(scene, size: size, background: backdrop.cgColor)

    let bytes = try #require(pixels(still))
    var apart = 0.0
    for i in 0..<(size * size) {
        for c in 0..<3 {
            apart += abs(Double(bytes[i * 4 + c]) - Double(encoded(light[i * 4 + c]) * 255))
        }
    }
    let mean = apart / Double(size * size * 3)
    #expect(mean < 1.0, "a still's bytes are \(mean) levels from the encoded light")
}

/// A still is the background it is given round the badge, in that colour.
@MainActor
@Test func aStillShowsTheBackgroundItIsGiven() async throws {
    let badge = try await sampleBadge()
    for background in [backdrop, LusterColor(red: 0.95, green: 0.94, blue: 0.93)] {
        let still = try await LusterSnapshot.image(badge, pixelSize: 64, background: background)
        let bytes = try #require(pixels(still))
        let corner = [bytes[0], bytes[1], bytes[2], bytes[3]].map(Int.init)
        let wanted = [background.red, background.green, background.blue, 1].map { Int(($0 * 255).rounded()) }
        for (got, want) in zip(corner, wanted) {
            #expect(abs(got - want) <= 1, "\(corner) against \(wanted)")
        }
    }
}

/// Without a background a still is clear round the badge and solid over it,
/// its alpha the badge's coverage. Laid over a colour it is the still taken
/// over that colour, but for the anti-aliased edge, which the renderer
/// blends in linear light where a compositor blends in sRGB.
@MainActor
@Test func aStillWithoutABackgroundLaysOverAnyColour() async throws {
    let badge = try await sampleBadge()
    let size = 160
    let clear = try #require(pixels(try await LusterSnapshot.image(badge, pixelSize: size)))
    #expect(Array(clear[0..<4]) == [0, 0, 0, 0], "the corner is clear")
    let middle = ((size / 2) * size + size / 2) * 4
    #expect(clear[middle + 3] == 255, "the badge is solid")
    let edge = (0..<(size * size)).filter { clear[$0 * 4 + 3] > 0 && clear[$0 * 4 + 3] < 255 }
    // A thin edge round the rim, not a glow.
    #expect(!edge.isEmpty && edge.count < size * size / 20, "\(edge.count) pixels of edge")

    for background in [backdrop, LusterColor(red: 1, green: 1, blue: 1)] {
        let over = try #require(pixels(try await LusterSnapshot.image(badge, pixelSize: size,
                                                                       background: background)))
        let under = [background.red, background.green, background.blue].map { Int(($0 * 255).rounded()) }
        var badgeOff = 0, groundOff = 0
        for i in 0..<(size * size) {
            for c in 0..<3 {
                switch clear[i * 4 + 3] {
                case 255: badgeOff = max(badgeOff, abs(Int(clear[i * 4 + c]) - Int(over[i * 4 + c])))
                case 0: groundOff = max(groundOff, abs(under[c] - Int(over[i * 4 + c])))
                default: break
                }
            }
        }
        #expect(badgeOff <= 1, "over \(background) the badge is \(badgeOff) levels off")
        #expect(groundOff <= 1, "over \(background) the ground is \(groundOff) levels off")
    }
}

/// The dark grey the other stills are taken over.
private let backdrop = LusterColor(red: 0.043, green: 0.043, blue: 0.047)

private func sampleBadge() async throws -> LusterBadge {
    let here = URL(fileURLWithPath: #filePath)
    let url = here.deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appendingPathComponent("fixtures/svg/sample-badge.svg")
    return try await LusterEngine.mint(.url(url))
}

/// The scene rendered into half floats, unencoded: the light itself.
@MainActor
private func render(_ scene: LusterScene, size: Int, background: CGColor) async throws -> [Float] {
    let renderer = try RealityRenderer()
    renderer.entities.append(scene.root)
    renderer.activeCamera = scene.camera
    renderer.cameraSettings.colorBackground = .color(background)
    let device = try #require(MTLCreateSystemDefaultDevice())
    let descriptor = MTLTextureDescriptor.texture2DDescriptor(
        pixelFormat: .rgba16Float, width: size, height: size, mipmapped: false)
    descriptor.usage = [.renderTarget, .shaderRead]
    descriptor.storageMode = .shared
    let texture = try #require(device.makeTexture(descriptor: descriptor))
    let output = try RealityRenderer.CameraOutput(.singleProjection(colorTexture: texture))
    try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, any Error>) in
        do {
            try renderer.updateAndRender(deltaTime: 0, cameraOutput: output,
                                         onComplete: { _ in done.resume() })
        } catch {
            done.resume(throwing: error)
        }
    }
    withExtendedLifetime(renderer) {}
    renderer.entities.removeAll()
    var halves = [UInt16](repeating: 0, count: size * size * 4)
    halves.withUnsafeMutableBytes { raw in
        texture.getBytes(raw.baseAddress!, bytesPerRow: size * 8,
                         from: MTLRegionMake2D(0, 0, size, size), mipmapLevel: 0)
    }
    return halves.map(float)
}

/// An IEEE half, read without `Float16`, which Intel Macs lack.
private func float(_ half: UInt16) -> Float {
    let sign: Float = half & 0x8000 == 0 ? 1 : -1
    let exponent = Int((half >> 10) & 0x1F)
    let fraction = Float(half & 0x3FF)
    if exponent == 0 { return sign * fraction * powf(2, -24) }
    if exponent == 31 { return sign * .infinity }
    return sign * (1 + fraction / 1024) * powf(2, Float(exponent - 15))
}

private func encoded(_ light: Float) -> Float {
    let c = min(max(light, 0), 1)
    return c <= 0.0031308 ? c * 12.92 : 1.055 * powf(c, 1 / 2.4) - 0.055
}

/// A still's RGBA bytes, rows top to bottom.
private func pixels(_ image: CGImage) -> [UInt8]? {
    guard let data = image.dataProvider?.data as Data? else { return nil }
    return [UInt8](data)
}
