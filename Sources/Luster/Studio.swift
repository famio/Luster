import CoreGraphics
import Foundation
internal import LusterCore

/// The studio's generated images, made off the main actor. What RealityKit
/// makes of them is kept, once, by `StudioResources`.
actor Studio {
    static let shared = Studio()

    /// The panorama the badge reflects, as an sRGB image.
    func environmentImage() -> CGImage? {
        CGImage.srgb(LusterCore.showcaseEnvironment())
    }

    /// The sandblast on a badge's reverse, as an image of its normals.
    func gritImage() -> CGImage? {
        CGImage.srgb(LusterCore.sandblast())
    }
}

extension CGImage {
    /// An image over the engine's RGBA8 sRGB bytes.
    static func srgb(_ texture: LusterCore.Texture) -> CGImage? {
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let provider = CGDataProvider(data: texture.rgba as CFData) else { return nil }
        return CGImage(width: Int(texture.width), height: Int(texture.height),
                       bitsPerComponent: 8, bitsPerPixel: 32,
                       bytesPerRow: Int(texture.width) * 4, space: space,
                       bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
                       provider: provider, decode: nil, shouldInterpolate: true,
                       intent: .defaultIntent)
    }
}
