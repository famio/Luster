import AppKit
import SwiftUI

/// The window's colors and metrics, in one place.
/// Each colour follows the system's light or dark appearance.
enum Palette {
    static let window = adaptive(light: 0xF2F1EE, dark: 0x0B0B0C)
    static let titleBar = adaptive(light: 0xE8E7E3, dark: 0x131316)
    static let sidebar = adaptive(light: 0xF8F7F5, dark: 0x0E0E11)
    static let hairline = adaptive(light: .black.withAlphaComponent(0.08),
                                   dark: .white.withAlphaComponent(0.06))
    static let sidebarEdge = adaptive(light: .black.withAlphaComponent(0.09),
                                      dark: .white.withAlphaComponent(0.07))
    static let swatchEdge = adaptive(light: .black.withAlphaComponent(0.12),
                                     dark: .white.withAlphaComponent(0.14))

    /// Selection rings and switches; deeper on light, where pale gold fades.
    static let accent = adaptive(light: 0xC08A2E, dark: 0xFFCC85)
    static let sectionLabel = adaptive(light: 0x85858D, dark: 0x6C6C75)
    static let titleText = adaptive(light: 0x707078, dark: 0x83838C)
    static let fileText = adaptive(light: 0x8E8E95, dark: 0x57575E)

    static let dropVeil = adaptive(light: NSColor(hex: 0xF2F1EE).withAlphaComponent(0.8),
                                   dark: NSColor(hex: 0x08080A).withAlphaComponent(0.74))
    static let dropText = adaptive(light: 0x7A5A22, dark: 0xE0CFAE)

    static let sidebarWidth: CGFloat = 286
    static let titleBarHeight: CGFloat = 44

    private static func adaptive(light: UInt32, dark: UInt32) -> Color {
        adaptive(light: NSColor(hex: light), dark: NSColor(hex: dark))
    }

    private static func adaptive(light: NSColor, dark: NSColor) -> Color {
        Color(nsColor: NSColor(name: nil) { appearance in
            appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? dark : light
        })
    }
}

private extension NSColor {
    convenience init(hex: UInt32) {
        self.init(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
                  green: CGFloat((hex >> 8) & 0xFF) / 255,
                  blue: CGFloat(hex & 0xFF) / 255,
                  alpha: 1)
    }
}
