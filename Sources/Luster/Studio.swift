import CoreGraphics
import Foundation
internal import LusterCore

/// The studio's shared, generated resources, made once per process off the
/// main actor.
actor Studio {
    static let shared = Studio()

    private var image: CGImage?

    /// The panorama the badge reflects, as an sRGB image.
    func environmentImage() -> CGImage? {
        if let image { return image }
        image = CGImage.srgb(LusterCore.showcaseEnvironment())
        return image
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
