import AppKit
import Foundation
import Luster

/// `LusterShot out.png art.svg [--metal-lines] [--off] [--background 0b0b0c]
///            [--size 768] [--spin -20] [--tilt 7]`
///
/// Renders one badge with the Rust engine and writes a PNG, so that what it
/// builds can be looked at, and compared, without opening a window. The
/// background is an sRGB hex colour, dark grey by default, or `clear` for
/// none: the PNG's alpha is then the badge's coverage.
///
/// `LusterShot --tone-table out.bin` measures RealityKit's tone mapping
/// instead (`writeToneTable`).
@MainActor
func run() async {
    let arguments = CommandLine.arguments.dropFirst()
    if let at = arguments.firstIndex(of: "--tone-table"), at + 1 < arguments.endIndex {
        func number(_ flag: String, _ fallback: Double) -> Double {
            guard let index = arguments.firstIndex(of: flag), index + 1 < arguments.endIndex
            else { return fallback }
            return Double(arguments[index + 1]) ?? fallback
        }
        do {
            try await writeToneTable(to: arguments[at + 1], size: Int(number("--size", 17)),
                                     range: Float(number("--range", 4)))
            FileHandle.standardError.write(Data("wrote \(arguments[at + 1])\n".utf8))
            exit(0)
        } catch {
            FileHandle.standardError.write(Data("failed: \(error)\n".utf8))
            exit(1)
        }
    }
    let files = arguments.filter { !$0.hasPrefix("--") }
    guard files.count >= 2 else {
        FileHandle.standardError.write(Data("usage: LusterShot out.png art.svg [flags]\n".utf8))
        exit(64)
    }
    func string(_ flag: String) -> String? {
        guard let index = arguments.firstIndex(of: flag), index + 1 < arguments.endIndex
        else { return nil }
        return arguments[index + 1]
    }
    func value(_ flag: String, _ fallback: Double) -> Double {
        string(flag).flatMap(Double.init) ?? fallback
    }
    let options = LusterOptions(metalLines: arguments.contains("--metal-lines"))
    let lighting: LusterLighting = arguments.contains("--off") ? .off : .showcase
    let appearance = LusterAppearance(lighting: lighting)
    let background: LusterColor?
    switch string("--background") ?? "0b0b0c" {
    case "clear": background = nil
    case let hex:
        guard let color = LusterColor(hex: hex) else {
            FileHandle.standardError.write(Data("not a colour: \(hex)\n".utf8))
            exit(64)
        }
        background = color
    }
    // Degrees in, radians out.
    let pose = SIMD2<Float>(Float(value("--tilt", 7) * .pi / 180),
                            Float(value("--spin", -20) * .pi / 180))

    do {
        let image = try await LusterSnapshot.image(.url(URL(fileURLWithPath: files[1])),
                                                   options: options, appearance: appearance,
                                                   pose: pose,
                                                   pixelSize: Int(value("--size", 768)),
                                                   background: background)
        guard let data = NSBitmapImageRep(cgImage: image)
            .representation(using: .png, properties: [:]) else {
            throw LusterSnapshot.Failure.noRenderer
        }
        try data.write(to: URL(fileURLWithPath: files[0]), options: .atomic)
        FileHandle.standardError.write(Data("wrote \(files[0])\n".utf8))
        exit(0)
    } catch {
        FileHandle.standardError.write(Data("failed: \(error)\n".utf8))
        exit(1)
    }
}

extension LusterColor {
    /// `rrggbb`, with or without a `#`.
    init?(hex: String) {
        let digits = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        guard digits.count == 6, let value = UInt32(digits, radix: 16) else { return nil }
        self.init(red: Float((value >> 16) & 0xFF) / 255,
                  green: Float((value >> 8) & 0xFF) / 255,
                  blue: Float(value & 0xFF) / 255)
    }
}

// A plain entry point: the renderer needs the main thread, and nothing else
// about this is an app.
Task { await run() }
RunLoop.main.run()
